use embassy_net::Ipv4Address;
use crc32fast::Hasher;
use defmt::*;

#[derive(Debug)]
pub enum ConfigError {
    BadMagic,
    BadCrc,
    InvalidMask,
}

// placeholder SD type (depends on your driver)
#[derive(Debug, Format)]
pub struct ConfigData {
    pub ip: Ipv4Address,
    pub gateway: Ipv4Address,
    pub mask: u8,
}

const CONFIG_MAGIC: u32 = 0x4346_4731;

pub fn parse_config(data: &[u8; 512]) -> Result<ConfigData, ConfigError> {
    // Magic
    let magic = u32::from_le_bytes([
        data[0],
        data[1],
        data[2],
        data[3],
    ]);

    if magic != CONFIG_MAGIC {
        return Err(ConfigError::BadMagic);
    }

    // CRC stored at bytes 13..17
    let stored_crc = u32::from_le_bytes([
        data[13],
        data[14],
        data[15],
        data[16],
    ]);

    // CRC covers magic + configuration, bytes 0..13.
    let mut hasher = Hasher::new();
    hasher.update(&data[0..13]);

    let calculated_crc = hasher.finalize();

    if calculated_crc != stored_crc {
        return Err(ConfigError::BadCrc);
    }

    let ip = Ipv4Address::new(
        data[4],
        data[5],
        data[6],
        data[7],
    );

    let gateway = Ipv4Address::new(
        data[8],
        data[9],
        data[10],
        data[11],
    );

    let mask = data[12];

    if mask > 32 {
        return Err(ConfigError::InvalidMask);
    }

    Ok(ConfigData {
        ip,
        gateway,
        mask,
    })
}
