#![allow(unused)]

use embassy_stm32::uid;

pub fn crc7(data: &[u8]) -> u8 {
    let mut crc = 0u8;

    for &byte in data {
        let mut d = byte;

        for _ in 0..8 {
            crc <<= 1;

            if ((d ^ crc) & 0x80) != 0 {
                crc ^= 0x09;
            }

            d <<= 1;
        }
    }

    crc
}

pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;

    for &byte in data {
        crc ^= (byte as u16) << 8;

        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

const FNV_OFFSET: u32 = 0x811C9DC5;
const FNV_PRIME: u32 = 0x01000193;

fn fnv1a(data: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET;

    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

pub fn generate_mac() -> [u8; 6] {
    let uid = uid::uid();
    let hash = fnv1a(&uid);

    [
        0x02,                           // Local administered, unicast
        (hash >> 24) as u8,
        (hash >> 16) as u8,
        (hash >> 8) as u8,
        hash as u8,
        uid[11],                        // dernier octet de l'UID
    ]
}

