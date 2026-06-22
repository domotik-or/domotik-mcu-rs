#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_net::{Ipv4Address, Ipv4Cidr, StackResources};
use embassy_net_wiznet::{chip::W5500, Device, Runner, State};
use embassy_stm32::{
    bind_interrupts,
    can::{
        CanConfigurator, IT0InterruptHandler, IT1InterruptHandler,
    },
    Config,
    dma::{InterruptHandler as DmaInterruptHandler},
    exti::{self, ExtiInput},
    gpio::{Level, Output, Pull, Speed},
    i2c::{self, I2c},
    interrupt,
    mode::Async,
    peripherals::{FDCAN1, GPDMA1_CH0, GPDMA1_CH6, GPDMA2_CH0, GPDMA2_CH3, I2C1, USART1},
    rcc,
    spi::{Config as SpiConfig, mode::Master, Spi},
    time::Hertz,
    usart::{self, BufferedUart, Config as UsartConfig, DataBits, Parity, StopBits}
};
use embassy_time::{Delay, Timer};
use {defmt_rtt as _, panic_probe as _};
use heapless::Vec;
use static_cell::StaticCell;

mod can;
mod ethernet;
mod ring;
mod serial;
mod sensor;

bind_interrupts!(struct Irqs {
    // Can
    FDCAN1_IT0 => IT0InterruptHandler<FDCAN1>;
    FDCAN1_IT1 => IT1InterruptHandler<FDCAN1>;

    // Ethernet
    EXTI0 => exti::InterruptHandler<interrupt::typelevel::EXTI0>;
    GPDMA2_CHANNEL0 => DmaInterruptHandler<GPDMA2_CH0>;
    GPDMA2_CHANNEL3 => DmaInterruptHandler<GPDMA2_CH3>;

    // I2c
    I2C1_EV => i2c::EventInterruptHandler<I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<I2C1>;
    GPDMA1_CHANNEL0 => DmaInterruptHandler<GPDMA1_CH0>;
    GPDMA1_CHANNEL6 => DmaInterruptHandler<GPDMA1_CH6>;

    // Serial
    USART1 => usart::BufferedInterruptHandler<USART1>;
});

type EthernetSPI = ExclusiveDevice<Spi<'static, Async, Master>, Output<'static>, Delay>;
#[embassy_executor::task]
async fn ethernet_task(runner: Runner<'static, W5500, EthernetSPI, ExtiInput<'static, Async>, Output<'static>>) -> ! {
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
    config.rcc.hse = Some(rcc::Hse {
        freq: embassy_stm32::time::Hertz(8_000_000),
        mode: rcc::HseMode::Oscillator,
    });
    config.rcc.mux.fdcan12sel = rcc::mux::Fdcansel::HSE;
    let p = embassy_stm32::init(config);

    info!("Hello World!");

    // Generate random seed (stm32f411 has no random generator)
    // let mut rng = Rng::new(p.RNG, Irqs);
    // let mut seed = [0; 8];
    // unwrap!(rng.async_fill_bytes(&mut seed).await);
    // let seed = u64::from_le_bytes(seed);
    let seed = 0x0123_4567_89AB_CDEFu64;

    // Spi
    let mut spi_cfg = SpiConfig::default();
    spi_cfg.frequency = Hertz(50_000_000); // up to 50m works
    let (miso, mosi, clk) = (p.PA6, p.PA7, p.PA5);
    let spi = Spi::new(p.SPI1, clk, mosi, miso, p.GPDMA2_CH3, p.GPDMA2_CH0, Irqs, spi_cfg);
    let cs = Output::new(p.PA4, Level::High, Speed::VeryHigh);
    let spi = unwrap!(ExclusiveDevice::new(spi, cs, Delay));

    // Ethernet
    let w5500_int = ExtiInput::new(p.PB0, p.EXTI0, Pull::Up, Irqs);
    let w5500_reset = Output::new(p.PB1, Level::High, Speed::VeryHigh);

    let mac_addr = [0x02, 234, 3, 4, 82, 231];
    static STATE: StaticCell<State<2, 2>> = StaticCell::new();
    let state = STATE.init(State::<2, 2>::new());
    let (device, runner) = embassy_net_wiznet::new(mac_addr, state, spi, w5500_int, w5500_reset)
        .await
        .unwrap();
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
    let i2c = I2c::new(p.I2C1, p.PB8, p.PB7, p.GPDMA1_CH6, p.GPDMA1_CH0, Irqs, Default::default());

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
    let buf_usart = BufferedUart::new(
        p.USART1, p.PA10, p.PA9,
        tx_buff, rx_buff,
        Irqs,
        config
    ).unwrap();

    // Can
    let mut can = CanConfigurator::new(p.FDCAN1, p.PA11, p.PA12, Irqs);
    can.set_bitrate(125_000);
    // let mut can = can.into_internal_loopback_mode();
    let can = can.into_normal_mode();

    // Spawn tasks
    spawner.spawn(can::task(can).unwrap());
    spawner.spawn(ethernet::task(stack).unwrap());
    spawner.spawn(ring::task(p.PB2, p.PB3, p.EXTI2).unwrap());
    spawner.spawn(serial::task(buf_usart).unwrap());
    spawner.spawn(sensor::task(i2c).unwrap());

    // default task
    let mut led = Output::new(p.PC13, Level::High, Speed::Low);

    loop {
        led.toggle();
        Timer::after_millis(500).await;
    }
}
