use embassy_stm32::gpio::Output;

use crate::board::SpiPeripheral;
use crate::config::{ConfigData, parse_config};
use crate::sd::{SdError, SdSpi, BlockReader};

pub fn load_sd_config(
    spi: SpiPeripheral, cs_sd: Output<'static>
) -> Result<(ConfigData, SpiPeripheral), SdError> {
    let mut sd_dev = SdSpi::new(spi, cs_sd)?;
    sd_dev.init()?;

    let mut sector = [0u8;512];
    sd_dev.read_block(0, &mut sector)?;

    let config = parse_config(&sector).unwrap();

    // return SPI ownership
    let (spi, _) = sd_dev.release();

    Ok((config, spi))
}
