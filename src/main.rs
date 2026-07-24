#![no_std]
#![no_main]

mod board;
mod boot;
mod can;
mod http_requests;
mod network;
mod planner;
mod ring;
mod serial;
mod sensor;
mod state;
mod utils;

// use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    Config,
    rcc::{Hse, HseMode, mux::Fdcansel},
    rtc::{DateTime, DayOfWeek},
};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use board::Board;


#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Peripherals
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.hsi48 = Some(Default::default()); // needed for RNG
    config.rcc.mux.fdcan12sel = Fdcansel::HSE;
    let p = embassy_stm32::init(config);

    let mut board = Board::init(p);

    let config = boot::read_config(board.spi_bus, board.cs_sd);

    info!("Config loaded");

    // Rtc
    board.rtc.0.set_datetime(
        DateTime::from(2026, 06, 28, DayOfWeek::Sunday, 18, 46, 0, 0).unwrap()
    ).unwrap();

    info!("Rtc programmed");

    // network
    let stack = network::bring_up(
        &spawner, board.spi_bus, board.cs_w5500, board.int_w5500, board.reset_w5500, board.rng,
        config.ip, config.gateway, config.mask,
    ).await;

    info!("Network ready");

    // Spawn tasks
    spawner.spawn(can::task(board.can_bus).unwrap());
    spawner.spawn(planner::task(stack).unwrap());
    spawner.spawn(ring::task(board.button, board.bell).unwrap());
    spawner.spawn(serial::task(board.buf_usart).unwrap());
    spawner.spawn(sensor::task(board.i2c_bus).unwrap());

    // default task
    loop {
        board.led.toggle();
        Timer::after_millis(500).await;
    }
}
