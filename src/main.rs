#![no_std]
#![no_main]

use defmt::*;
use embassy_stm32::{
    rcc::{Hse, HseMode, mux::Fdcansel},
};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::{Spawner, task};
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
    peripherals::{GPDMA1_CH0, GPDMA1_CH1, RNG},
    rng::{InterruptHandler as RngInterruptHandler, Rng },
    spi::{Spi, Config as SpiConfig, mode::Master as SpiMaster},
    time::Hertz,
    uid,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    mutex::Mutex
};
use heapless::Vec;
use static_cell::StaticCell;

pub type SpiPeripheral = Spi<'static, Async, SpiMaster>;
pub type SpiBus = Mutex<
    CriticalSectionRawMutex,
    SpiPeripheral
>;

type EthernetSpiDevice = SpiDevice<
    'static,
    CriticalSectionRawMutex,
    SpiPeripheral,
    Output<'static>,
>;
type EthernetRunner = embassy_net_wiznet::Runner<
    'static, W5500, EthernetSpiDevice, ExtiInput<'static, Async>, Output<'static>
>;
type NetworkRunner = embassy_net::Runner<'static, Device<'static>>;

static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static STATE: StaticCell<State<2, 2>> = StaticCell::new();
const SPI_FREQ: u32 = 9_000_000;

static SPI_BUS: StaticCell<SpiBus> = StaticCell::new();

bind_interrupts!(struct Irqs {
    // Spi
    EXTI0 => ExtiInterruptHandler<interrupt::typelevel::EXTI0>;
    GPDMA1_CHANNEL0 => DmaInterruptHandler<GPDMA1_CH0>;
    GPDMA1_CHANNEL1 => DmaInterruptHandler<GPDMA1_CH1>;

    // Exti
    EXTI2 => ExtiInterruptHandler<interrupt::typelevel::EXTI2>;

    // Random Number Generator
    RNG => RngInterruptHandler<RNG>;
});

pub struct Board {
    pub spi_bus: &'static SpiBus,

    pub cs_w5500: Output<'static>,
    pub int_w5500: ExtiInput<'static, Async>,
    pub reset_w5500: Output<'static>,

    pub cs_sd: Output<'static>,

    pub rng: Rng<'static, RNG>,
}

impl Board {
    pub fn init(p: Peripherals) -> Self {
        // Spi
        let mut spi_cfg = SpiConfig::default();
        spi_cfg.frequency = Hertz(SPI_FREQ);

        let spi = Spi::new(
            p.SPI1,
            p.PA5, // SCK
            p.PA7, // MOSI
            p.PA6, // MISO
            p.GPDMA1_CH0,
            p.GPDMA1_CH1,
            Irqs,
            spi_cfg,
        );

        let spi_bus = SPI_BUS.init(Mutex::new(spi));

        // W5500 pins
        let cs_w5500 = Output::new(p.PA4, Level::High, Speed::VeryHigh);
        let int_w5500 = ExtiInput::new(p.PB0, p.EXTI0, Pull::Up, Irqs);
        let reset_w5500 = Output::new(p.PB1, Level::High, Speed::VeryHigh);

        // SD CARD CS (boot phase only)
        let cs_sd = Output::new(p.PB12, Level::High, Speed::VeryHigh);

        // Misc
        let rng = Rng::new(p.RNG, Irqs);

        Self {
            spi_bus,
            cs_w5500,
            int_w5500,
            reset_w5500,
            cs_sd,
            rng,
        }
    }
}

// placeholder SD type (depends on your driver)
pub struct ConfigData {
    pub ip: Ipv4Address,
    pub gateway: Ipv4Address,
    pub mask: u8,
}

pub fn read_config(_spi_bus: &'static SpiBus, _cs_sd: Output<'static>) -> ConfigData {
    // let mut sd = fake_sd_driver(spi_bus, cs_sd);

    // pseudo-code
    let config = ConfigData {
        ip: Ipv4Address::new(192, 168, 1, 50),
        mask: 24,
        gateway: Ipv4Address::new(192, 168, 1, 1),
    };

    // return SPI ownership
    // let spi = sd.release_spi();

    config
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
    spi_bus: &'static SpiBus,
    cs: Output<'static>,
    int: ExtiInput<'static, Async>,
    reset: Output<'static>,
    mut rng: Rng<'static, RNG>,
    ip: Ipv4Address,
    gateway: Ipv4Address,
    mask: u8,
) -> Stack<'static> {
    let mac_addr = generate_mac();
    let state = STATE.init(State::new());
    let spi_dev = SpiDevice::new(spi_bus, cs);
    let (device, eth_runner) = embassy_net_wiznet::new::<2, 2, W5500, _, _, _>(
        mac_addr, state, spi_dev, int, reset
    ).await.unwrap();

    // Generate random seed
    let mut seed = [0; 8];
    unwrap!(rng.async_fill_bytes(&mut seed).await);
    let seed = u64::from_le_bytes(seed);

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
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.hsi48 = Some(Default::default()); // needed for RNG
    config.rcc.mux.fdcan12sel = Fdcansel::HSE;
    let p = embassy_stm32::init(config);

    let board = Board::init(p);

    let config = read_config(board.spi_bus, board.cs_sd);

    info!("Config loaded");

    // network
    let _stack = bring_up(
        &spawner, board.spi_bus, board.cs_w5500, board.int_w5500, board.reset_w5500, board.rng,
        config.ip, config.gateway, config.mask,
    ).await;

    info!("Network ready");

    // Spawn tasks

    // default task
    loop {
        Timer::after_millis(500).await;
    }
}
