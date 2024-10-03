#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod stts22h;
mod usb_interface;

use defmt::{debug, trace, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    gpio::{self, Output},
    i2c::{self, I2c},
    mode::{Async, Blocking},
    peripherals,
    rcc::WPAN_DEFAULT,
    time::khz,
    usb,
};
use embassy_sync::{
    blocking_mutex::{raw::NoopRawMutex, Mutex as SyncMutex},
    mutex::Mutex,
};
use embassy_time::Timer;
use static_cell::StaticCell;
use stts22h::STTS22H;
use usb_interface::pipe_data_to_usb;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(
    struct Irqs {
        I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
        I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
        USB_LP => usb::InterruptHandler<peripherals::USB>;
    }
);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    trace!("Starting");

    let p = embassy_stm32::init({
        let mut config = embassy_stm32::Config::default();
        config.rcc = WPAN_DEFAULT;
        config
    });
    trace!("RCC setup");

    Output::new(p.PC7, gpio::Level::High, gpio::Speed::VeryHigh);
    trace!("Power on external sensors");

    let i2c = I2c::new(
        p.I2C1,
        p.PB8,
        p.PB9,
        Irqs,
        p.DMA1_CH1,
        p.DMA1_CH2,
        khz(100),
        i2c::Config::default(),
    );

    // Attente entre i2c setup et utilisation
    // Peut-etre pour attendre une protagation de la config
    Timer::after_millis(100).await;

    let mut stts22h = STTS22H::new(i2c);
    unwrap!(stts22h.init().await);
    defmt::assert_eq!(unwrap!(stts22h.id().await), 0xA0);
    debug!("STTS22H init");

    debug!("Init done");
    unwrap!(spawner.spawn(pipe_data_to_usb(p.USB, p.PA12, p.PA11)));
    unwrap!(spawner.spawn(sensor_reading(stts22h)));

    loop {
        core::future::pending::<()>().await;
    }
}

#[embassy_executor::task]
async fn sensor_reading(mut stts22h: STTS22H) {
    loop {
        if let Ok(temp) = stts22h.temperature().await {
            debug!("stts22h {}", temp);
        }
        defmt::assert_eq!(unwrap!(stts22h.id().await), 0xA0);
    }
}
