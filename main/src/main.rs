#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

mod ring;
mod serial;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("Hello World!");

    // Spawn tasks
    spawner.spawn(serial::task(p.USART1, p.PA9, p.PA10).unwrap());
    spawner.spawn(ring::task(p.PB2, p.PB3, p.EXTI2).unwrap());

    loop {
        Timer::after_millis(500).await;
    }
}
