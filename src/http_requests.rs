use core::fmt::Write;
use defmt::info;
use embassy_net::{dns::DnsSocket, Stack, tcp::client::{TcpClient, TcpClientState}};
// use embassy_time::Timer;
// use embedded_io_async::Write;
use heapless::{String};
use reqwless::{client::HttpClient, request::Method};
use static_cell::StaticCell;

async fn send(stack: Stack<'static>, domain: &str, parameters: &str) {
    const HOST: &str = env!("API_HOST");
    const PORT: &str = env!("API_PORT");

    let mut rx_buffer = [0; 1024];

    static CLIENT_STATE: StaticCell<TcpClientState<1, 1024, 1024>> = StaticCell::new();
    let client_state = CLIENT_STATE.init(TcpClientState::<1, 1024, 1024>::new());

    let tcp_client = TcpClient::new(stack, &client_state);
    let dns_client = DnsSocket::new(stack);
    let mut http_client = HttpClient::new(&tcp_client, &dns_client);

    let mut url: String<128> = String::new();
    if write!(&mut url, "{}:{}/api/{}?{}", HOST, PORT, domain, parameters).is_ok() {
        if let Ok(mut req) = http_client.request(Method::GET, &url).await {
            if let Ok(resp) = req.send(&mut rx_buffer).await {
                if resp.status.0 == 200 {
                    info!("request sent successfully");
                } else {
                    info!("error while sending the request");
                };
            };
        } else {
            info!("error while making the request");
        };
    } else {
        info!("request while formatting the url");
    };
}


pub async fn send_outdoor(stack: Stack<'static>, humidity: f32, pressure: f32, temperature: f32) {
    let mut parameters: String<64> = String::new();
    if write!(parameters, "humidity={:0.2}&pressure={}&temperature={:0.2}", humidity, pressure, temperature).is_ok() {
        send(stack, "outdoor", &parameters).await;
    } else {
        info!("error while formatting parameters");
    };
}


pub async fn send_linky(stack: Stack<'static>, east: u32, sinst: u32) {
    let mut parameters: String<128> = String::new();
    if write!(parameters, "east={}&sinst={}", east, sinst).is_ok() {
        send(stack, "outdoor", &parameters).await;
    } else {
        info!("error while formatting parameters");
    };
}
