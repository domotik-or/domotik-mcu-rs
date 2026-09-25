#[cfg(feature = "defmt")]
use defmt::*;
use embassy_executor::task;
use embassy_stm32::usart::BufferedUart;
use embedded_io_async::Read;

use crate::state::linky::{set_east, set_sinsts};
use my_libs::linky::Linky;

#[task]
pub async fn task(mut buf_usart: BufferedUart<'static>) {

    let mut linky = Linky::new();
    let mut buf = [0u8; 32];

    loop {
        if let Ok(n) = buf_usart.read(&mut buf).await {
            if n != 0 {
                for b in &mut buf[..n] { *b &= 0x7f; };
                // info!("{}", &buf[..n]);

                linky.decode_frame(&buf, n);

                if let Some(east) = linky.get_east() {
                    set_east(east).await;

                    #[cfg(feature = "defmt")]
                    info!("east: {}", east);
                };

                if let Some(sinsts) = linky.get_sinsts() {
                    set_sinsts(sinsts).await;

                    #[cfg(feature = "defmt")]
                    info!("sinsts: {}", sinsts);
                };
            };
        };
    };
}
