#[cfg(feature = "defmt")]
use defmt::*;
use embassy_executor::task;
use embassy_time::{Duration, Timer};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::Output,
    mode::Async,
};

use crate::http_requests::HttpRequests;

async fn send_ring(http: &'static HttpRequests) -> Result<(), ()>{
    if let Ok(timestamp) = http.get("ring", format_args!("")).await {
        #[cfg(feature = "defmt")]
        info!("timestamp: {}", timestamp);
    };

    Ok(())
}

async fn wait_button_pressed(button: &mut ExtiInput<'static, Async>) {
    loop {
        // Premier front : potentiellement un appui
        button.wait_for_rising_edge().await;

        // Laisser passer les rebonds
        Timer::after(Duration::from_millis(20)).await;

        // Vérifier que le bouton est réellement toujours appuyé
        if button.is_high() {
            return;
        }
    }
}

#[task]
pub async fn task(
    http: &'static HttpRequests, mut button: ExtiInput<'static, Async>, mut bell: Output<'static>
) {
    loop {
        wait_button_pressed(&mut button).await;
        #[cfg(feature = "defmt")]
        info!("Pulled!");

        let _ = send_ring(http).await;

        // ring the bell five times
        for _ in 1..=5 {
            bell.set_high();
            Timer::after_millis(500).await;
            bell.set_low();
            Timer::after_millis(1500).await;
        }
    }
}
