use embassy_net::Ipv4Address;
use embassy_stm32::gpio::Output;

use crate::board::SpiBus;

// placeholder SD type (depends on your driver)
pub struct ConfigData {
    pub ip: Ipv4Address,
    pub gateway: Ipv4Address,
    pub mask: u8,
}

pub fn read_config(_spi_bus: &'static SpiBus, _cs_sd: Output<'static>) -> ConfigData {
    // let mut sd = fake_sd_driver(spi_bus, cs_sd);

    // pseudo-code
    let config = ConfigData {
        ip: Ipv4Address::new(192, 168, 1, 50),
        mask: 24,
        gateway: Ipv4Address::new(192, 168, 1, 1),
    };

    // return SPI ownership
    // let spi = sd.release_spi();

    config
}
