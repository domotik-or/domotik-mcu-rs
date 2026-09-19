use defmt::info;
use embassy_executor::task;
use embassy_time::Timer;
use embassy_stm32::usart::BufferedUart;
use embedded_io_async::Read;

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
                    // We expect: DATA CHECKSUM CR LF
                    if len >= 3 && frame[len - 2] == b'\r' {
                        let received_checksum = frame[len - 3];

                        let checksum = (
                            frame[..len - 3].iter().fold(0u8, |sum, &b| sum.wrapping_add(b)) & 0x3f
                        ) + 0x20;

                        if checksum == received_checksum {
                            // info!("valid: {}", &frame[..len - 3]);

                            let temp = frame[0..4].iter().fold(0u32, |n, &b| n * 10 + (b - b'0') as u32);
                            let temp = temp as f32 / 100.0;
                            let humidity = frame[5..10].iter().fold(0u32, |n, &b| n * 10 + (b - b'0') as u32);
                            let humidity = humidity as f32 / 100.0;
                            let pressure = frame[11..17].iter().fold(0u32, |n, &b| n * 10 + (b - b'0') as u32);
                            let pressure = pressure as f32 / 100.0;

                            info!("temperature: {}, humidity: {}, pressure: {}", temp, humidity, pressure);
                        } else {
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
