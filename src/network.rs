// use defmt::*;
use embassy_executor::{Spawner, task};
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_net::{
    self,
    dns::DnsSocket,
    Ipv4Address,
    Ipv4Cidr,
    Stack,
    StackResources,
    tcp::client::{TcpClient, TcpClientState}
};
use embassy_net_wiznet::{self, chip::W5500, Device, State};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::Output,
    mode::Async,
};
use embassy_time::Timer;
use heapless::Vec;
use static_cell::StaticCell;

use crate::board::{EthernetSpiDevice, SpiPeripheral};
use crate::utils::generate_mac;

type EthernetRunner = embassy_net_wiznet::Runner<
    'static, W5500, &'static mut EthernetSpiDevice, ExtiInput<'static, Async>, Output<'static>
>;
type NetworkRunner = embassy_net::Runner<'static, Device<'static>>;

static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static STATE: StaticCell<State<2, 2>> = StaticCell::new();
static W5500_SPI: StaticCell<EthernetSpiDevice> = StaticCell::new();

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
    let mut dns_servers = Vec::new();
    dns_servers.push(gateway).unwrap();
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
       address: Ipv4Cidr::new(ip, mask),
       dns_servers,
       gateway: Some(gateway),
    });
    // let (stack, net_runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    let ressource = RESOURCES.init(StackResources::new());
    let (stack, net_runner) = embassy_net::new(device, config, ressource, seed);

    // Launch ethernet task
    spawner.spawn(ethernet_task(eth_runner).unwrap());

    // Launch network task
    spawner.spawn(net_task(net_runner).unwrap());

    // Ensure DHCP configuration is up before trying to connect
    stack.wait_config_up().await;

    stack
}

static CLIENT_STATE: StaticCell<TcpClientState<1, 1024, 1024>> = StaticCell::new();

pub fn create_tcp_clients(stack: Stack<'static>) -> (TcpClient<'static, 1, 1024, 1024>, DnsSocket<'static>) {
    let client_state = CLIENT_STATE.init(TcpClientState::new());
    (TcpClient::new(stack, client_state), DnsSocket::new(stack))
}
