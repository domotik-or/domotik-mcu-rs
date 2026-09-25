use core::fmt::{Arguments, Write};

#[cfg(feature = "defmt")]
use defmt::{error, info, Debug2Format};

use embassy_net::{dns::DnsSocket, tcp::client::TcpClient};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use heapless::String;
use reqwless::{client::HttpClient, request::Method};

use crate::rtc::SharedRtc;

const API_URL: &str = env!("API_URL");

static HTTP_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

type Tcp = TcpClient<'static, 1, 1024, 1024>;

pub struct HttpRequests {
    dns: &'static DnsSocket<'static>,
    tcp: &'static Tcp,
    rtc: &'static SharedRtc,
}

impl HttpRequests {
    pub const fn new(
        tcp: &'static Tcp,
        dns: &'static DnsSocket<'static>,
        rtc: &'static SharedRtc
    ) -> Self {
        Self{dns, rtc, tcp}
    }

    pub async fn get(&self, route: &str, query: Arguments<'_>) -> Result<u64, ()> {
        let mut url: String<256> = String::new();

        write!(&mut url, "{}/{}?", API_URL, route,).map_err(|_| ())?;

        url.write_fmt(query).map_err(|_| ())?;

        let _guard = HTTP_LOCK.lock().await;

        let mut buffer = [0u8; 1024];

        let mut http_client = HttpClient::new(self.tcp, self.dns);

        let mut req = match http_client.request(Method::GET, &url).await {
            Ok(req) => req,

            Err(e) => {
                #[cfg(feature = "defmt")]
                error!("request error: {:?}", Debug2Format(&e));

                return Err(());
            }
        };

        let resp = match req.send(&mut buffer).await {
            Ok(resp) => resp,

            Err(e) => {
                #[cfg(feature = "defmt")]
                error!("send error: {:?}", Debug2Format(&e));

                return Err(());
            }
        };

        #[cfg(feature = "defmt")]
        info!("HTTP status: {}", resp.status.0);

        if resp.status.0 != 200 {
            return Err(());
        }

        // Consume resp and read the complete response body.
        let body = resp.body().read_to_end().await.map_err(|_| ())?;

        // Convert the response bytes to text.
        let text = core::str::from_utf8(body).map_err(|_| ())?;

        // Convert the text to a Unix timestamp.
        let timestamp = text.trim().parse::<u64>().map_err(|_| ())?;

        {
            let mut rtc = self.rtc.lock().await;
            rtc.set_datetime(timestamp)?;
        }

        Ok(timestamp)
    }
}
