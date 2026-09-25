use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

#[derive(Clone, Copy)]
pub struct Outdoor {
    pub humidity: u16,
    pub pressure: u32,
    pub temperature: i16,
}

static OUTDOOR: Mutex<CriticalSectionRawMutex, Outdoor> = Mutex::new(Outdoor{humidity: 0, pressure: 0, temperature: 0});

pub async fn set(humidity: u16, temperature: i16, pressure: u32) {
    let mut o = OUTDOOR.lock().await;
    o.humidity = humidity;
    o.pressure = pressure;
    o.temperature = temperature;
}

pub async fn get() -> Outdoor {
    let o = OUTDOOR.lock().await;
    Outdoor {
        humidity: o.humidity,
        pressure: o.pressure,
        temperature: o.temperature,
    }
}
