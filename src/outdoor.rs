#[cfg(feature = "defmt")]
use defmt::*;
use embassy_executor::task;
use embassy_stm32::usart::BufferedUart;
use embedded_io_async::Read;

use crate::state::set_outdoor;

#[task]
pub async fn task(mut buf_usart: BufferedUart<'static>) {

    let mut buf = [0u8; 32];
    let mut frame = [0u8; 64];
    let mut len = 0;

    loop {
        if let Ok(n) = buf_usart.read(&mut buf).await {
            for &raw in &buf[..n] {
                let b = raw & 0x7f;

                if len < frame.len() {
                    frame[len] = b;
                    len += 1;
                } else {
                    // Overflow: discard current frame
                    len = 0;
                    continue;
                }

                if b == b'\n' {
                    // Frame format
                    // temp  hum   press
                    // -1111,22222,333333C\r\n
                    // 01111,22222,333333C\r\n
                    // 0     6     12    18
                    //                   -3
                    // <---> <---> <---->
                    //   5     5     6

                    if len >= 3 && frame[len - 2] == b'\r' {
                        let received_checksum = frame[len - 3];

                        let checksum = (
                            frame[..len - 3].iter().fold(0u8, |sum, &b| sum.wrapping_add(b)) & 0x3f
                        ) + 0x20;

                        if checksum == received_checksum {
                            // info!("valid: {}", &frame[..len - 3]);

                            let temperature = if frame[0] == b'-' {
                                let temperature = -frame[1..5].iter().fold(0i16, |n, &b| n * 10 + (b - b'0') as i16);
                                temperature
                            } else {
                                let temperature = frame[0..5].iter().fold(0i16, |n, &b| n * 10 + (b - b'0') as i16);
                                temperature
                            };

                            let humidity = frame[6..11].iter().fold(0u16, |n, &b| n * 10 + (b - b'0') as u16);

                            let pressure = frame[12..18].iter().fold(0u32, |n, &b| n * 10 + (b - b'0') as u32);

                            set_outdoor(humidity, temperature, pressure).await;

                            #[cfg(feature = "defmt")]
                            info!("temperature: {}, humidity: {}, pressure: {}", temperature, humidity, pressure);
                        } else {
                            #[cfg(feature = "defmt")]
                            info!(
                                "bad checksum: received={} calculated={}",
                                received_checksum,
                                checksum
                            );
                        }
                    }

                    len = 0;
                }
            }
        }
    }
}
