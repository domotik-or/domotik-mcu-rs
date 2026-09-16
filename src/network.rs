use defmt::*;
use embassy_executor::{Spawner, task};
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_net::{self, Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_net_wiznet::{self, chip::W5500, Device, State};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::Output,
    mode::Async,
    uid,
};
use embassy_time::Timer;
use heapless::Vec;
use static_cell::StaticCell;

use crate::board::{EthernetSpiDevice, SpiPeripheral};

type EthernetRunner = embassy_net_wiznet::Runner<
    'static, W5500, &'static mut EthernetSpiDevice, ExtiInput<'static, Async>, Output<'static>
>;
type NetworkRunner = embassy_net::Runner<'static, Device<'static>>;

const FNV_OFFSET: u32 = 0x811C9DC5;
const FNV_PRIME: u32 = 0x01000193;

static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static STATE: StaticCell<State<2, 2>> = StaticCell::new();
static W5500_SPI: StaticCell<EthernetSpiDevice> = StaticCell::new();

fn fnv1a(data: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET;

    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

fn generate_mac() -> [u8; 6] {
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
