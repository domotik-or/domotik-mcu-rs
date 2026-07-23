use defmt::info;
use embassy_executor::task;
use embassy_time::Timer;
use embassy_stm32::usart::BufferedUart;
use embedded_io_async::Read;

use my_libs::linky::Linky;
use crate::state::set_linky;

#[task]
pub async fn task(mut buf_usart: BufferedUart<'static>) {

    let mut linky = Linky::new();
    let mut buf = [0u8; 32];

    loop {
        let n = buf_usart.read(&mut buf).await.unwrap();
        info!("Received: {}", buf);

        linky.decode_frame(&buf, n);

        if let Some(east) = linky.east {
            if let Some(sinst) = linky.sinst {
                set_linky(east, sinst).await;
            };
        }

        Timer::after_secs(30).await;
    };
}
