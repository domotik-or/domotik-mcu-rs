#[cfg(feature = "defmt")]
use defmt::{debug, error, info};
use embassy_executor::task;
use embassy_time::Timer;

use crate::http_requests::HttpRequests;
use crate::state::{get_linky, get_outdoor};
use crate::rtc::SharedRtc;

async fn send_linky_east(
    http: &'static HttpRequests, month: u8, year: u16, east: u32
) -> Result<(), ()> {
    match http.get(
        "set_east",
        format_args!("month={}&year={}&value={}", month, year, east)
    ).await {
        Ok(timestamp) => {
            #[cfg(feature = "defmt")]
            debug!("timestamp: {}", timestamp);
        },
        Err(e) => {
            #[cfg(feature = "defmt")]
            error!("error: {}", e);
        }
    };

    Ok(())
}

async fn send_linky_sinsts(http: &'static HttpRequests, sinsts: u16) -> Result<(), ()> {
    if let Ok(_timestamp) = http.get_path(format_args!("set_sinsts/{}", sinsts)).await {
        #[cfg(feature = "defmt")]
        debug!("timestamp: {}", _timestamp);
    };

    Ok(())
}

async fn send_outdoor(
    http: &'static HttpRequests,
    humidity: u16, pressure: u32, temperature: i16
) -> Result<(), ()>{
    match http.get(
        "set_outdoor",
        format_args!(
            "temperature={}&humidity={}&pressure={}", temperature, humidity, pressure
        )
    ).await {
        Ok(timestamp) => {
            #[cfg(feature = "defmt")]
            debug!("timestamp: {}", timestamp);
        },
        Err(e) => {
            #[cfg(feature = "defmt")]
            error!("error: {}", e);
        }
    };

    Ok(())
}

#[task]
pub async fn task(http: &'static HttpRequests, rtc: &'static SharedRtc) {
    let mut sec_counter = 0u16;
    let mut sinsts_old = 0;

    let mut last_month = 0;
    let mut sinsts_sent: bool = false;
    loop {
        let linky = get_linky().await;

        // send east at the beginning of each month
        let now = {
            let rtc = rtc.lock().await;
            rtc.now()
        };
        if let Ok(now) = now {
            let month = now.month();
            if last_month != month {
                let year = now.year();

                #[cfg(feature = "defmt")]
                info!("sending east {}/{}", month, year);

                if let Ok(_) = send_linky_east(http, month, year, linky.east).await {
                    last_month = month;
                };
            };
        };

        sinsts_sent |= if linky.sinsts.abs_diff(sinsts_old) >= 20 {
            #[cfg(feature = "defmt")]
            info!("sending sinsts (difference): {}", linky.sinsts);

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
            if sinsts_sent {
                sinsts_sent = false;
            } else {
                #[cfg(feature = "defmt")]
                info!("sending sinsts (periodical): {}", linky.sinsts);

                let _ = send_linky_sinsts(http, linky.sinsts).await;
                sinsts_old = linky.sinsts;
            }
            let outdoor = get_outdoor().await;
            let _ = send_outdoor(http, outdoor.humidity, outdoor.pressure, outdoor.temperature).await;
        }

        Timer::after_secs(1).await;
    }
}
