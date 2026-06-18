use defmt::info;
use embassy_executor::task;
// use embassy_time::Timer;
use embassy_stm32::{
    bind_interrupts,
    Peri, peripherals::{PA9, PA10, USART1},
    usart::{self, BufferedUart, Config, DataBits, Parity, StopBits}
};
use embedded_io_async::Read;

use my_libs::linky::Linky;

bind_interrupts!(struct Irqs {
    USART1 => usart::BufferedInterruptHandler<USART1>;
});

#[task]
pub async fn task(uart: Peri<'static, USART1>, tx: Peri<'static, PA9>, rx: Peri<'static, PA10>) {
    let mut config = Config::default();
    // set Linky serial line configuration
    config.baudrate = 9600;
    config.parity = Parity::ParityEven;
    config.data_bits = DataBits::DataBits7;
    config.stop_bits =  StopBits::STOP1;

    let mut tx_buf  = [0u8; 32];
    let mut rx_buf = [0u8; 32];
    let mut buf = [0u8; 32];
    let mut buf_usart = BufferedUart::new(uart, rx, tx, &mut tx_buf, &mut rx_buf, Irqs, config).unwrap();

    let mut linky = Linky::new();

    loop {
        let n = buf_usart.read(&mut buf).await.unwrap();
        info!("Received: {}", buf);

        linky.decode_frame(&buf, n);

        // Timer::after_millis(500).await;
    };
}
