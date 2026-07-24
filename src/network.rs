use defmt::*;

use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::{Spawner, task};
use embassy_net::{self, Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_net_wiznet::{self, chip::W5500, Device, State};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::Output,
    mode::Async,
    peripherals::RNG,
    rng::Rng,
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use heapless::Vec;
use static_cell::StaticCell;

use crate::utils::generate_mac;
use crate::board::{SpiBus, SpiPeripheral};

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
