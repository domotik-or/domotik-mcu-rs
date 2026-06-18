use defmt::info;
use embassy_executor::task;
use embassy_time::Timer;
use embassy_stm32::{
    bind_interrupts,
    exti::{self, ExtiInput},
    gpio::{Level, Output, Pull, Speed},
    interrupt,
    Peri, peripherals::{EXTI2, PB2, PB3},
};

bind_interrupts!(
    pub struct Irqs{
        EXTI2 => exti::InterruptHandler<interrupt::typelevel::EXTI2>;
});

#[task]
pub async fn task(input: Peri<'static, PB2>, output: Peri<'static, PB3>, exti: Peri<'static, EXTI2>) {
    // configure gpio
    let mut button = ExtiInput::new(input, exti, Pull::Up, Irqs);
    let mut bell = Output::new(output, Level::High, Speed::Low);

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
