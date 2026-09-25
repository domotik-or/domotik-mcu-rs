#[cfg(feature = "defmt")]
use defmt::info;
use embassy_executor::task;
use embassy_time::Timer;

use crate::state::{get_linky, get_outdoor};
use crate::http_requests::HttpRequests;

// async fn send_linky_east() {
// }

async fn send_linky_sinsts(http: &'static HttpRequests, sinsts: u16) -> Result<(), ()> {
    if let Ok(timestamp) = http.get("sinsts", format_args!("value={}", sinsts)).await {
        #[cfg(feature = "defmt")]
        info!("timestamp: {}", timestamp);
    };

    Ok(())
}

async fn send_outdoor(
    http: &'static HttpRequests,
    humidity: u16, pressure: u32, temperature: i16
) -> Result<(), ()>{
    if let Ok(timestamp) = http.get(
        "outdoor",
        format_args!(
            "temperature={}&humidity={}&pressure={}", temperature, humidity, pressure
        )
    ).await {
        #[cfg(feature = "defmt")]
        info!("timestamp: {}", timestamp);
    };

    Ok(())
}

#[task]
pub async fn task(http: &'static HttpRequests) {
    let mut sec_counter = 0u16;
    let mut sinsts_old = 0;

    let mut sinsts_sent: bool = false;
    loop {
        let linky = get_linky().await;

        sinsts_sent |= if linky.sinsts.abs_diff(sinsts_old) >= 20 {
            let _ = send_linky_sinsts(http, linky.sinsts).await;
            sinsts_old = linky.sinsts;
            true
        } else {
            false
        };

        sec_counter += 1;
        if sec_counter == 300 {
            sec_counter = 0;

            // send sinsts at least on a periodical basis
            if !sinsts_sent {
                let _ = send_linky_sinsts(http, linky.sinsts).await;
                sinsts_old = linky.sinsts;
                sinsts_sent = false;
            }
            let outdoor = get_outdoor().await;
            let _ = send_outdoor(http, outdoor.humidity, outdoor.pressure, outdoor.temperature).await;
        }

        Timer::after_secs(1).await;
    }
}
