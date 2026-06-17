#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

mod serial;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("Hello World!");

    let button = Input::new(p.PA0, Pull::Down);
    let mut led = Output::new(p.PC13, Level::High, Speed::Low);

    // Spawn serial task
    spawner.spawn(serial::task(p.USART1, p.PA9, p.PA10).unwrap());

    loop {
        if button.is_high() {
            led.toggle();
        }

        info!("high");
        led.set_high();
        Timer::after_millis(500).await;

        info!("low");
        led.set_low();
        Timer::after_millis(500).await;
    }
}
