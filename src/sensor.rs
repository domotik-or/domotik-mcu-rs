use defmt::info;
use embassy_executor::task;
// use embassy_time::Timer;
use embassy_stm32::{
    i2c::{I2c, mode::Master},
    mode::Async,
};
use embassy_bmp280::{Bmp280, Bmp280Address, Bmp280Config};

use crate::state::set_pressure;

#[task]
pub async fn task(i2c_device: I2c<'static, Async, Master>) {
    // Adresse par défaut
    let mut bmp = Bmp280::new(i2c_device, Bmp280Address::Default, Bmp280Config::default()).await.unwrap();

    loop {
        let data = bmp.read().await.unwrap();

        // Température en centidegrés (i32) : 2315 = 23.15 °C
        let temp_cdeg = data.temperature_cdeg as f32 / 100.0;

        // Pression en Pascals × 256 (format Q24.8)
        let press_raw = data.pressure_pa256;
        // let press_pa  = press_raw / 256;        // Pascals (entier)
        let press_hpa = press_raw as f32 / (256.0 * 100.0); // hPa (avec feature "float")

        info!("temperature: {}, pressure: {}", temp_cdeg, press_hpa);

        set_pressure(press_hpa).await;
    }
}
