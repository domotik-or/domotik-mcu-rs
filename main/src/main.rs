#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_net::{Ipv4Address, Ipv4Cidr, StackResources};
use embassy_net_wiznet::{chip::W5500, Device, Runner, State};
use embassy_stm32::{
    bind_interrupts,
    Config,
    dma,
    exti::{self, ExtiInput},
    gpio::{Level, Output, Pull, Speed},
    interrupt,
    mode::Async,
    peripherals::{self, USART1},
    spi::{Config as SpiConfig, mode::Master, Spi},
    time::Hertz,
    usart::{self, BufferedUart, Config as UsartConfig, DataBits, Parity, StopBits}
};
use embassy_time::{Delay, Timer};
use {defmt_rtt as _, panic_probe as _};
use heapless::Vec;
use static_cell::StaticCell;

mod ring;
mod serial;
mod ethernet;

bind_interrupts!(struct Irqs {
    // Ethernet
    EXTI0 => exti::InterruptHandler<interrupt::typelevel::EXTI0>;
    DMA2_STREAM0 => dma::InterruptHandler<peripherals::DMA2_CH0>;
    DMA2_STREAM3 => dma::InterruptHandler<peripherals::DMA2_CH3>;

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
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Bypass,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL180,
            divp: Some(PllPDiv::DIV2), // 8mhz / 4 * 180 / 2 = 180Mhz.
            divq: None,
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.sys = Sysclk::PLL1_P;
    }
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
    let spi = Spi::new(p.SPI1, clk, mosi, miso, p.DMA2_CH3, p.DMA2_CH0, Irqs, spi_cfg);
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

    // Spawn tasks
    spawner.spawn(ethernet::task(stack).unwrap());
    spawner.spawn(serial::task(buf_usart).unwrap());
    spawner.spawn(ring::task(p.PB2, p.PB3, p.EXTI2).unwrap());

    loop {
        Timer::after_millis(500).await;
    }
}
