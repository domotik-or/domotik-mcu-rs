#![no_std]
#![no_main]

mod crc;
mod config;
mod sd;

use defmt::*;
use defmt_rtt as _;
// Let panic_probe handle our panic routine
use panic_probe as _;

use chrono::{NaiveDate, NaiveDateTime};
use embassy_executor::{Spawner, task};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embassy_net::{self, Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_net_wiznet::{self, chip::W5500, Device, State};
use embassy_stm32::{
    bind_interrupts,
    Config,
    dma::{InterruptHandler as DmaInterruptHandler},
    exti::{ExtiInput, InterruptHandler as ExtiInterruptHandler},
    gpio::{Output, Level, Speed, Pull},
    interrupt,
    mode::Async,
    Peripherals,
    peripherals::{DMA2_CH2, DMA2_CH3},
    rcc::{AHBPrescaler, APBPrescaler, HseMode::Oscillator, Pll, PllPDiv, PllMul, PllPreDiv, Sysclk},
    rtc::{Rtc, RtcConfig, RtcTimeProvider},
    spi::{Config as SpiConfig, MODE_0, mode::Master as SpiMaster, Spi},
    time::Hertz,
    uid,
};
use embassy_time::Timer;
use heapless::Vec;
use static_cell::StaticCell;

use config::{ConfigData, parse_config};
use sd::{SdError, SdSpi, BlockReader};

pub type SpiPeripheral = Spi<'static, Async, SpiMaster>;
type EthernetSpiDevice = ExclusiveDevice<
    SpiPeripheral,
    Output<'static>,
    NoDelay,
>;
type EthernetRunner = embassy_net_wiznet::Runner<
    'static, W5500, &'static mut EthernetSpiDevice, ExtiInput<'static, Async>, Output<'static>
>;
type NetworkRunner = embassy_net::Runner<'static, Device<'static>>;

const ETH_SPI_FREQ: u32 = 1_000_000;
const SD_SPI_FREQ: u32 = 400_000;

static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static STATE: StaticCell<State<2, 2>> = StaticCell::new();
static W5500_SPI: StaticCell<EthernetSpiDevice> = StaticCell::new();

bind_interrupts!(struct Irqs {
    // Spi
    EXTI0 => ExtiInterruptHandler<interrupt::typelevel::EXTI0>;
    DMA2_STREAM2 => DmaInterruptHandler<DMA2_CH2>;
    DMA2_STREAM3 => DmaInterruptHandler<DMA2_CH3>;

    // Exti
    EXTI2 => ExtiInterruptHandler<interrupt::typelevel::EXTI2>;
});

pub struct Board {
    pub spi: SpiPeripheral,

    pub cs_w5500: Output<'static>,
    pub int_w5500: ExtiInput<'static, Async>,
    pub reset_w5500: Output<'static>,

    pub cs_sd: Output<'static>,

    pub rtc: Rtc,
    pub time_provider: RtcTimeProvider,
}

impl Board {
    pub fn init(p: Peripherals) -> Self {
        // Spi
        let spi = Spi::new(
            p.SPI1,
            p.PA5, // SCK
            p.PA7, // MOSI
            p.PA6, // MISO
            p.DMA2_CH3,
            p.DMA2_CH2,
            Irqs,
            SpiConfig::default(),
        );

        // W5500 pins
        let cs_w5500 = Output::new(p.PA3, Level::High, Speed::VeryHigh);
        let int_w5500 = ExtiInput::new(p.PB0, p.EXTI0, Pull::Up, Irqs);
        let reset_w5500 = Output::new(p.PB1, Level::High, Speed::VeryHigh);

        // SD CARD CS (boot phase only)
        let cs_sd = Output::new(p.PA4, Level::High, Speed::VeryHigh);

        let (rtc, time_provider) = Rtc::new(p.RTC, RtcConfig::default());

        Self {
            spi,
            cs_w5500,
            int_w5500,
            reset_w5500,
            cs_sd,
            rtc,
            time_provider,
        }
    }
}

pub fn configure_sd(spi: &mut SpiPeripheral) {
    let mut cfg = SpiConfig::default();
    cfg.frequency = Hertz(SD_SPI_FREQ);
    cfg.mode = MODE_0;
    spi.set_config(&cfg).unwrap();
}

pub fn configure_w5500(spi: &mut SpiPeripheral) {
    let mut cfg = SpiConfig::default();
    cfg.frequency = Hertz(ETH_SPI_FREQ);
    cfg.mode = MODE_0;
    spi.set_config(&cfg).unwrap();
}

pub fn load_sd_config(
    spi: SpiPeripheral, cs_sd: Output<'static>
) -> Result<(ConfigData, SpiPeripheral), SdError> {
    let mut sd_dev = SdSpi::new(spi, cs_sd)?;
    sd_dev.init()?;

    let mut sector = [0u8;512];
    sd_dev.read_block(0, &mut sector)?;

    let config = parse_config(&sector).unwrap();

    // return SPI ownership
    let (spi, _) = sd_dev.release();

    Ok((config, spi))
}

const FNV_OFFSET: u32 = 0x811C9DC5;
const FNV_PRIME: u32 = 0x01000193;

fn fnv1a(data: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET;

    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

pub fn generate_mac() -> [u8; 6] {
    let uid = uid::uid();
    let hash = fnv1a(&uid);

    [
        0x02,                           // Local administered, unicast
        (hash >> 24) as u8,
        (hash >> 16) as u8,
        (hash >> 8) as u8,
        hash as u8,
        uid[11],                        // dernier octet de l'UID
    ]
}

#[task]
async fn ethernet_task(runner: EthernetRunner) -> ! {
    runner.run().await
}

#[task]
async fn net_task(mut runner: NetworkRunner) -> ! {
    runner.run().await
}

pub async fn bring_up(
    spawner: &Spawner,
    spi: SpiPeripheral,
    cs: Output<'static>,
    int: ExtiInput<'static, Async>,
    mut reset: Output<'static>,
    ip: Ipv4Address,
    gateway: Ipv4Address,
    mask: u8,
) -> Stack<'static> {
    // Reset W5500
    reset.set_low();
    Timer::after_millis(10).await;
    reset.set_high();
    Timer::after_millis(100).await;

    let mac_addr = generate_mac();
    let state = STATE.init(State::new());
    let spi_dev = W5500_SPI.init(ExclusiveDevice::new_no_delay(spi, cs).unwrap());

    let (device, eth_runner) = embassy_net_wiznet::new::<2, 2, W5500, _, _, _>(
        mac_addr, state, spi_dev, int, reset
    ).await.unwrap();

    let seed = 0_u64;

    // Network stack
    // let config = embassy_net::Config::dhcpv4(Default::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
       address: Ipv4Cidr::new(ip, mask),
       dns_servers: Vec::new(),
       gateway: Some(gateway),
    });
    // let (stack, net_runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    let ressource = RESOURCES.init(StackResources::new());
    let (stack, net_runner) = embassy_net::new(device, config, ressource, seed);

    // Launch ethernet task
    spawner.spawn(unwrap!(ethernet_task(eth_runner)));

    // Launch network task
    spawner.spawn(unwrap!(net_task(net_runner)));

    // Ensure DHCP configuration is up before trying to connect
    stack.wait_config_up().await;

    stack
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Peripherals
    let mut config = Config::default();

    config.rcc.hse = Some(embassy_stm32::rcc::Hse { freq: Hertz(25_000_000), mode: Oscillator});

    config.rcc.pll_src = embassy_stm32::rcc::PllSource::HSE;

    config.rcc.pll = Some(Pll {
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

    info!("MCU initialized");

    let mut board = Board::init(p);

    info!("Board initialized");

    Timer::after_millis(100).await;

    // Rtc initialization (uncomment to set date and time)
    // let now = NaiveDate::from_ymd_opt(2026, 9, 13).unwrap().and_hms_opt(15, 30, 15).unwrap();
    // board.rtc.set_datetime(now.into()).unwrap();

    let mut spi = board.spi;

    configure_sd(&mut spi);
    let (config, mut spi) = load_sd_config(spi, board.cs_sd).unwrap();
    info!("Configuration loaded {:?}", config);

    configure_w5500(&mut spi);
    let _stack = bring_up(
        &spawner, spi, board.cs_w5500, board.int_w5500, board.reset_w5500,
        config.ip, config.gateway, config.mask,
    ).await;

    info!("Application ready");

    // Spawn tasks

    // default task
    loop {
        let now: NaiveDateTime = board.time_provider.now().unwrap().into();
        info!("{}", now);

        Timer::after_millis(500).await;
    }
}
