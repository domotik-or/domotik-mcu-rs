#![no_std]
#![no_main]

// use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
// use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_embedded_hal;
use embassy_net::{Ipv4Address, Ipv4Cidr, StackResources};
use embassy_net_wiznet::{chip::W5500, Device, Runner, State};
use embassy_stm32::{
    bind_interrupts,
    can::{CanConfigurator, IT0InterruptHandler, IT1InterruptHandler},
    Config,
    dma::{InterruptHandler as DmaInterruptHandler},
    exti::{ExtiInput, InterruptHandler as ExtiInterruptHandler},
    gpio::{Level, Output, Pull, Speed},
    i2c::{
        I2c,
        ErrorInterruptHandler as I2cErrorInterruptHandler,
        EventInterruptHandler as I2cEventInterruptHandler
    },
    interrupt,
    mode::Async,
    peripherals::{FDCAN1, GPDMA1_CH0, GPDMA1_CH1, GPDMA1_CH2, GPDMA1_CH3, I2C1, RNG, USART1},
    rcc::{
        Hse, HseMode, /* mux::Fdcansel, */
    },
    rng::{InterruptHandler as RngInterruptHandler, Rng},
    rtc::{DateTime, DayOfWeek, Rtc, RtcConfig},
    spi::{Config as SpiConfig, mode::Master, Spi},
    time::Hertz,
    usart::{self, BufferedUart, Config as UsartConfig, DataBits, Parity, StopBits}
};
use embassy_time::{Delay, Timer};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use {defmt_rtt as _, panic_probe as _};
use heapless::Vec;
use static_cell::StaticCell;

mod can;
mod http_requests;
mod planner;
mod ring;
mod serial;
mod sensor;
mod state;
mod utils;

bind_interrupts!(struct Irqs {
    // Can
    FDCAN1_IT0 => IT0InterruptHandler<FDCAN1>;
    FDCAN1_IT1 => IT1InterruptHandler<FDCAN1>;

    // Spi
    EXTI0 => ExtiInterruptHandler<interrupt::typelevel::EXTI0>;
    GPDMA1_CHANNEL0 => DmaInterruptHandler<GPDMA1_CH0>;
    GPDMA1_CHANNEL1 => DmaInterruptHandler<GPDMA1_CH1>;

    // Exti
    EXTI2 => ExtiInterruptHandler<interrupt::typelevel::EXTI2>;

    // I2c
    I2C1_EV => I2cEventInterruptHandler<I2C1>;
    I2C1_ER => I2cErrorInterruptHandler<I2C1>;
    GPDMA1_CHANNEL2 => DmaInterruptHandler<GPDMA1_CH2>;
    GPDMA1_CHANNEL3 => DmaInterruptHandler<GPDMA1_CH3>;

    // Random Number Generator
    RNG => RngInterruptHandler<RNG>;

    // Serial
    USART1 => usart::BufferedInterruptHandler<USART1>;
});

static SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, Spi<'static, Async, Master>>> = StaticCell::new();

#[embassy_executor::task]
async fn ethernet_task(runner: Runner<'static,
        W5500,
        embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice<
            'static,
            CriticalSectionRawMutex,
            Spi<'static, Async, Master>,
            Output<'static>,
            // Master,
        >,
        ExtiInput<'static, Async>,
        Output<'static>
    >
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Peripherals
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.hsi48 = Some(Default::default()); // needed for RNG
    // config.rcc.mux.fdcan12sel = Fdcansel::HSE;
    let p = embassy_stm32::init(config);

    info!("Hello World!");

    // Rtc
    let mut rtc = Rtc::new(p.RTC, RtcConfig::default());
    rtc.0.set_datetime(
        DateTime::from(2026, 06, 28, DayOfWeek::Sunday, 18, 46, 0, 0).unwrap()
    ).unwrap();

    info!("Rtc programmed");

    // Generate random seed
    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    unwrap!(rng.async_fill_bytes(&mut seed).await);
    let seed = u64::from_le_bytes(seed);

    // Spi
    let mut spi_cfg = SpiConfig::default();
    spi_cfg.frequency = Hertz(9_000_000);  // a sd card is 10MHz max, the spi bus is shared
    let (miso, mosi, clk) = (p.PA6, p.PA7, p.PA5);
    let spi = Spi::new(p.SPI1, clk, mosi, miso, p.GPDMA1_CH0, p.GPDMA1_CH1, Irqs, spi_cfg);

    let spi_bus: &'static _ = SPI_BUS.init(Mutex::new(spi));

    // Ethernet
    let cs_w5500 = Output::new(p.PA4, Level::High, Speed::VeryHigh);
    let spi_w5500 = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(&spi_bus, cs_w5500);

    let int_w5500 = ExtiInput::new(p.PB0, p.EXTI0, Pull::Up, Irqs);
    let reset_w5500 = Output::new(p.PB1, Level::High, Speed::VeryHigh);

    let mac_addr = utils::generate_mac();
    static STATE: StaticCell<State<2, 2>> = StaticCell::new();
    let state = STATE.init(State::new());
    let (device, runner) = embassy_net_wiznet::new::<2, 2, W5500, _, _, _>(mac_addr, state, spi_w5500, int_w5500, reset_w5500).await.unwrap();
    spawner.spawn(unwrap!(ethernet_task(runner)));

    // let config = embassy_net::Config::dhcpv4(Default::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
       address: Ipv4Cidr::new(Ipv4Address::new(192,168, 1, 50), 24),
       dns_servers: Vec::new(),
       gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
    });

    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);

    // Launch network task
    spawner.spawn(unwrap!(net_task(runner)));

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    info!("Network task initialized");

    // I2c
    let i2c = I2c::new(p.I2C1, p.PB8, p.PB7, p.GPDMA1_CH2, p.GPDMA1_CH3, Irqs, Default::default());

    // Usart
    let mut config = UsartConfig::default();
    // set Linky serial line configuration
    config.baudrate = 9600;
    config.parity = Parity::ParityEven;
    config.data_bits = DataBits::DataBits7;
    config.stop_bits =  StopBits::STOP1;

    static RX_BUF: StaticCell<[u8; 32]> = StaticCell::new();
    static TX_BUF: StaticCell<[u8; 32]> = StaticCell::new();
    let rx_buff = RX_BUF.init([0u8; 32]);
    let tx_buff = TX_BUF.init([0u8; 32]);
    let buf_usart = BufferedUart::new(p.USART1, p.PA10, p.PA9, tx_buff, rx_buff, Irqs, config).unwrap();

    // Can
    let mut can = CanConfigurator::new(p.FDCAN1, p.PA11, p.PA12, Irqs);
    can.set_bitrate(125_000);
    // let mut can = can.into_internal_loopback_mode();
    let can = can.into_normal_mode();

    // Gpio
    let button = ExtiInput::new(p.PB2, p.EXTI2, Pull::Up, Irqs);
    let bell = Output::new(p.PB3, Level::High, Speed::Low);

    // Spawn tasks
    spawner.spawn(can::task(can).unwrap());
    spawner.spawn(planner::task(stack).unwrap());
    spawner.spawn(ring::task(button, bell).unwrap());
    spawner.spawn(serial::task(buf_usart).unwrap());
    spawner.spawn(sensor::task(i2c).unwrap());

    // default task
    let mut led = Output::new(p.PC13, Level::High, Speed::Low);

    loop {
        led.toggle();
        Timer::after_millis(500).await;
    }
}
