use crate::I2cDevice;
use embassy_embedded_hal::shared_bus::I2cDeviceError;
use embassy_stm32::i2c;
use embassy_time::Timer;
use embedded_hal_async::i2c::I2c as _;

#[derive(Clone, Copy)]
#[allow(non_camel_case_types, unused)]
enum Reg {
    WHOAMI = 0x01,
    TEMP_H_LIMIT = 0x02,
    TEMP_L_LIMIT = 0x03,
    CTRL = 0x04,
    STATUS = 0x05,
    TEMP_L_OUT = 0x06,
    TEMP_H_OUT = 0x07,
}

struct Ctrl;
#[allow(unused)]
impl Ctrl {
    /// Enables 1 Hz ODR operating mode (see Section 11 Operating modes).
    const LOW_ODR_START: u8 = 0x80;
    /// Default is set to 0 for BDU disabled; 1 for BDU enabled (if BDU is used, TEMP_L_OUT must be read first).
    /// Block Data Update
    const BDU: u8 = 0x40;
    /// These bits are used to set the number of averages configuration. When in freerun mode, these bits also set the ODR (see Table 13. Average configuration).
    const AVG: u8 = 0x30;
    /// If this bit is set to 1, the automatic address increment is enabled when multiple I²C read and write transactions are used.
    const IF_ADD_INC: u8 = 0x08;
    /// Enables freerun mode
    const FREERUN: u8 = 0x04;
    /// If this bit is set to 1, the timeout function of SMBus is disabled.
    const TIME_OUT_DIS: u8 = 0x02;
    /// If this bit is set to 1, a new one-shot temperature acquisition is executed.
    const ONE_SHOT: u8 = 0x01;
}

#[derive(Clone, Copy)]
pub struct Status(u8);
impl Status {
    /// 0: Low limit temperature not exceeded (or disabled).
    /// 1: Low limit temperature exceeded.
    /// The bit is automatically reset to 0 upon reading the STATUS register.
    const UNDER_THL: u8 = 0x04;

    /// 0: High limit temperature not exceeded (or disabled).
    /// 1: High limit temperature exceeded.
    /// The bit is automatically reset to 0 upon reading the STATUS register.
    const OVER_THH: u8 = 0x02;

    /// The BUSY bit is applicable to one-shot mode only:
    /// 0: The conversion is complete.
    /// 1: The conversion is in progress.
    const BUSY: u8 = 0x01;

    pub fn under_thl(self) -> bool {
        (self.0 & Status::UNDER_THL) != 0
    }

    pub fn over_thh(self) -> bool {
        (self.0 & Status::OVER_THH) != 0
    }
    //use only with one_shot or 1hz frequency
    pub fn busy(self) -> bool {
        (self.0 & Status::BUSY) != 0
    }
}
impl defmt::Format for Status {
    fn format(&self, fmt: defmt::Formatter) {
        let mut prev = false;
        defmt::write!(fmt, "Status(");

        if self.under_thl() {
            defmt::write!(fmt, "UNDER_THL");
            prev = true;
        }

        if self.over_thh() {
            if prev {
                defmt::write!(fmt, " | ");
            }
            defmt::write!(fmt, "OVER_THH");
            prev = true;
        }

        if self.busy() {
            if prev {
                defmt::write!(fmt, " | ");
            }
            defmt::write!(fmt, "BUSY");
        }

        defmt::write!(fmt, ")");
    }
}

/// Temperature sensor
pub struct STTS22H {
    i2c: I2cDevice,
}
impl STTS22H {
    const ADDRESS: u8 = 0x70 >> 1;
    pub fn new(i2c: I2cDevice) -> Self {
        STTS22H { i2c: i2c }
    }

    pub async fn init(&mut self) -> Result<(), I2cDeviceError<i2c::Error>> {
        self.i2c
            .write(Self::ADDRESS, &[Reg::CTRL as u8, Ctrl::IF_ADD_INC])
            .await
    }

    pub async fn id(&mut self) -> Result<u8, I2cDeviceError<i2c::Error>> {
        let mut res = [0; 1];
        self.i2c
            .write_read(Self::ADDRESS, &[Reg::WHOAMI as u8], &mut res)
            .await?;

        Ok(res[0])
    }

    pub async fn temperature(&mut self) -> Result<f32, I2cDeviceError<i2c::Error>> {
        let mut reg = [0; 1];
        self.i2c
            .write_read(Self::ADDRESS, &[Reg::CTRL as u8], &mut reg)
            .await?;

        self.i2c
            .write(Self::ADDRESS, &[Reg::CTRL as u8, reg[0] | Ctrl::ONE_SHOT])
            .await?;

        while self.status().await?.busy() {
            Timer::after_millis(100).await;
        }

        let mut temp = [0; 2];
        self.i2c
            .write_read(Self::ADDRESS, &[Reg::TEMP_L_OUT as u8], &mut temp)
            .await?;

        let temp = i16::from_le_bytes(temp) as f32 / 100.;

        Ok(temp)
    }

    pub async fn status(&mut self) -> Result<Status, I2cDeviceError<i2c::Error>> {
        let mut res = [0; 1];
        self.i2c
            .write_read(Self::ADDRESS, &[Reg::STATUS as u8], &mut res)
            .await?;

        Ok(Status(res[0]))
    }
}
