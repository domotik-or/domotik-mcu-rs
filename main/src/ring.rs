use defmt::info;
use embassy_executor::task;
use embassy_time::Timer;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::Output,
    mode::Async,
};


#[task]
pub async fn task(mut button: ExtiInput<'static, Async>, mut bell: Output<'static>) {
    // configure gpio

    loop {
        button.wait_for_falling_edge().await;
        info!("Pulled!");

        // ring the bell five times
        for _ in 1..=5 {
            bell.set_high();
            Timer::after_millis(500).await;
            bell.set_low();
            Timer::after_millis(1500).await;
        }
    }
}
