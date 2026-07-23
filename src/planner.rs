// use defmt::info;
use embassy_executor::task;
use embassy_net::Stack;
use embassy_time::Timer;

use crate::state::{get_linky, get_outdoor};
use crate::http_requests::{send_linky, send_outdoor};

#[task]
pub async fn task(stack: Stack<'static>) {
    loop {
        let linky = get_linky().await;
        send_linky(stack, linky.east, linky.sinst).await;
        let outdoor = get_outdoor().await;
        send_outdoor(stack, outdoor.humidity, outdoor.pressure, outdoor.temperature).await;

        Timer::after_secs(30).await;
    }
}
