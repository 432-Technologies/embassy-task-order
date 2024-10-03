use crate::Irqs;
use defmt::debug;
use embassy_futures::select::select;
use embassy_stm32::peripherals::{PA11, PA12, USB};
use embassy_stm32::usb::Driver as UsbDriver;
use embassy_usb::driver::{Driver, Endpoint, EndpointIn, EndpointOut};
use embassy_usb::msos::{self, windows_version};
use embassy_usb::Builder;
use embassy_usb::{
    class::web_usb::{Config as WebUsbConfig, State, WebUsb},
    Config,
};

const DEVICE_INTERFACE_GUIDS: &[&str] = &["{AFB9A6FB-30BA-44BC-9232-806CFC875321}"];

#[embassy_executor::task]
pub async fn pipe_datas_to_usb(usb: USB, pa12: PA12, pa11: PA11) {
    let driver = UsbDriver::new(usb, Irqs, pa12, pa11);
    // Create embassy-usb Config
    let mut config = Config::new(0xDEAD, 0xC0DE);
    config.manufacturer = Some("Manufacturer");
    config.product = Some("Product");
    config.serial_number = Some("12345678");
    // config.self_powered = true;
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xff;
    config.device_sub_class = 0x00;
    config.device_protocol = 0x00;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut msos_descriptor = [0; 256];

    let webusb_config = WebUsbConfig {
        max_packet_size: 64,
        vendor_code: 1,
        // If defined, shows a landing page which the device manufacturer would like the user to visit in order to control their device. Suggest the user to navigate to this URL when the device is connected.
        landing_url: None,
    };

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    builder.msos_descriptor(windows_version::WIN8_1, 0);
    builder.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
    builder.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
        "DeviceInterfaceGUIDs",
        msos::PropertyData::RegMultiSz(DEVICE_INTERFACE_GUIDS),
    ));

    WebUsb::configure(&mut builder, &mut state, &webusb_config);

    let mut endpoints = WebEndpoints::new(&mut builder, &webusb_config);
    let mut usb = builder.build();

    loop {
        let usb_fut = usb.run();
        let webusb_fut = async {
            debug!("Waiting for an interface to connect");
            endpoints.wait_connected().await;
            debug!("Connected to webusb");
            endpoints.run().await;
        };

        select(usb_fut, webusb_fut).await;
        debug!("pipe cut, attemting to disable USB bus");

        usb.disable().await;
    }
}

struct WebEndpoints<'d, D: Driver<'d>> {
    write_ep: D::EndpointIn,
    read_ep: D::EndpointOut,
}

impl<'d, D: Driver<'d>> WebEndpoints<'d, D> {
    fn new(builder: &mut Builder<'d, D>, config: &'d WebUsbConfig<'d>) -> Self {
        let mut func = builder.function(0xff, 0x00, 0x00);
        let mut iface = func.interface();
        let mut alt = iface.alt_setting(0xff, 0x00, 0x00, None);

        let write_ep = alt.endpoint_bulk_in(config.max_packet_size);
        let read_ep = alt.endpoint_bulk_out(config.max_packet_size);

        WebEndpoints { write_ep, read_ep }
    }

    // Wait until the device's endpoints are enabled.
    async fn wait_connected(&mut self) {
        self.read_ep.wait_enabled().await
    }

    async fn run(&mut self) {
        let mut buff = [0; 64];
        loop {
            let read = self.read_ep.read(&mut buff).await.unwrap();
            self.write_ep.write(&buff[..read]).await.unwrap()
        }
    }
}
