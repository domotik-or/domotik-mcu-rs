use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

pub struct Outdoor {
    pub humidity: f32,
    pub pressure: f32,
    pub temperature: f32,
}

static OUTDOOR: Mutex<CriticalSectionRawMutex, Outdoor> = Mutex::new(Outdoor{humidity: 0.0, pressure: 0.0, temperature: 0.0});

pub async fn set_humidity_and_temperature(humidity: f32, temperature: f32) {
    let mut o = OUTDOOR.lock().await;
    o.humidity = humidity;
    o.temperature = temperature;
}

pub async fn set_pressure(pressure: f32) {
    let mut o = OUTDOOR.lock().await;
    o.pressure = pressure;
}

pub async fn get_outdoor() -> Outdoor {
    let o = OUTDOOR.lock().await;
    Outdoor {
        humidity: o.humidity,
        pressure: o.pressure,
        temperature: o.temperature,
    }
}
