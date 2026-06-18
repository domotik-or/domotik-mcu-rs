use defmt::info;
use embassy_executor::task;
use embassy_net::{/* dns::DnsQueryType,*/ Ipv4Address, Stack, tcp::TcpSocket};
use embassy_time::Timer;
use embedded_io_async::Write;

#[task]
pub async fn task(stack: Stack<'static>) {
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];

    // let ip = if let Ok(ip) = stack.dns_query("www.tf1.fr", DnsQueryType::A).await {
    //     ip[0]
    // } else {
    //     return;
    // };

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        // let remote_endpoint = (ip, 8000);
        let remote_endpoint = (Ipv4Address::new(10, 42, 0, 1), 8000);
        info!("connecting...");
        let r = socket.connect(remote_endpoint).await;
        if let Err(e) = r {
            info!("connect error: {:?}", e);
            Timer::after_secs(1).await;
            continue;
        }
        info!("connected!");
        let buf = [0; 1024];
        loop {
            let r = socket.write_all(&buf).await;
            if let Err(e) = r {
                info!("write error: {:?}", e);
                break;
            }
            Timer::after_secs(1).await;
        }
    }
}
