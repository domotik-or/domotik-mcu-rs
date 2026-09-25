#![no_std]
#![no_main]

mod board;
mod boot;
mod config;
mod http_requests;
mod network;
mod outdoor;
mod ring;
mod sd;
mod state;
mod tic;
mod utils;

#[cfg(feature = "defmt")]
use defmt::*;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
// Let panic_probe handle our panic routine
use panic_probe as _;

use chrono::{NaiveDate, NaiveDateTime};
use embassy_executor::Spawner;
use embassy_stm32::{
    Config,
    rcc::{
        AHBPrescaler, APBPrescaler, Hse, HseMode, LsConfig,
        Pll, PllPDiv, PllSource, PllMul, PllPreDiv, Sysclk
    },
    time::Hertz,
};
use embassy_time::Timer;

use board::{Board, configure_sd, configure_w5500};
use boot::load_sd_config;
use network::{bring_up, create_tcp_clients};


#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Peripherals
    let mut config = Config::default();

    config.rcc.hse = Some(Hse{freq: Hertz(25_000_000), mode: HseMode::Oscillator});

    config.rcc.ls = LsConfig::default_lse();

    config.rcc.pll_src = PllSource::HSE;

    config.rcc.pll = Some(Pll{
        prediv: PllPreDiv::DIV25,
        mul: PllMul::MUL336,
        divp: Some(PllPDiv::DIV4),
        divq: None,
        divr: None,
    });

    config.rcc.sys = Sysclk::PLL1_P;

    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV1;

    let p = embassy_stm32::init(config);

    #[cfg(feature = "defmt")]
    info!("MCU initialized");

    let mut board = Board::init(p);

    #[cfg(feature = "defmt")]
    info!("Board initialized");

    Timer::after_millis(100).await;

    // Rtc initialization (uncomment to set date and time)
    let now = NaiveDate::from_ymd_opt(2026, 9, 14).unwrap().and_hms_opt(17, 00, 15).unwrap();
    board.rtc.set_datetime(now.into()).unwrap();

    let mut spi_dev = board.spi_dev;

    #[cfg(feature = "defmt")]
    info!("before 2s delay");
    Timer::after_secs(2).await;
    #[cfg(feature = "defmt")]
    info!("after 2s delay");

    configure_sd(&mut spi_dev);

    let (sd_config, mut spi_dev) = match load_sd_config(spi_dev, board.cs_sd) {
        Ok(result) => {
            board.led.set_low(); // ON = SD SUCCESS
            result
        }
        Err(_) => {
            // LED remains OFF = SD FAILED
            loop {
                Timer::after_secs(1).await;
            }
        }
    };

    // let (sd_config, mut spi) = load_sd_config(spi, board.cs_sd).unwrap();
    #[cfg(feature = "defmt")]
    info!("Configuration loaded {:?}", sd_config);

    configure_w5500(&mut spi_dev);
    let stack = bring_up(
        &spawner, spi_dev, board.cs_w5500, board.int_w5500, board.reset_w5500,
        sd_config.ip, sd_config.gateway, sd_config.mask,
    ).await;

    let (tcp_client, dns_client) = create_tcp_clients(stack);

    #[cfg(feature = "defmt")]
    info!("Application ready");

    // Spawn tasks
    spawner.spawn(outdoor::task(board.buf_usart2).unwrap());
    spawner.spawn(ring::task(board.button, board.bell).unwrap());
    spawner.spawn(tic::task(board.buf_usart1).unwrap());

    // default task
    loop {
        let _now: NaiveDateTime = board.time_provider.now().unwrap().into();
        // debug!("{}", now);

        http_requests::send(&dns_client, &tcp_client).await;

        Timer::after_secs(10).await;
    }
}
