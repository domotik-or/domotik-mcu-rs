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
