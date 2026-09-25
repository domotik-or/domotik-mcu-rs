#[cfg(feature = "defmt")]
use defmt::info;
use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use reqwless::{client::HttpClient, request::Method};


pub async fn send(dns_client: &DnsSocket<'static>, tcp_client: &TcpClient<'static, 1, 1024, 1024>) {
    let mut rx_buffer = [0; 1024];

    let mut http_client = HttpClient::new(&tcp_client, &dns_client);

    let url = "http://http-stat.us/200";
    match http_client.request(Method::GET, url).await {
        Ok(mut req) => {
            #[cfg(feature = "defmt")]
            info!("request created");

            match req.send(&mut rx_buffer).await {
                Ok(resp) => {
                    #[cfg(feature = "defmt")]
                    info!("HTTP status: {}", resp.status.0);
                }
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    info!("send error: {:?}", defmt::Debug2Format(&e));
                }
            }
        }

        Err(e) => {
            #[cfg(feature = "defmt")]
            info!("request error: {:?}", defmt::Debug2Format(&e));
        }
    }
}
