use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

#[derive(Clone, Copy)]
pub struct Linky {
    pub east: u32,
    pub sinsts: u32,
}

static LINKY: Mutex<CriticalSectionRawMutex, Linky> = Mutex::new(Linky{east: 0u32, sinsts: 0u32});

pub async fn set_east(east: u32) {
    let mut l = LINKY.lock().await;
    l.east = east;
}

pub async fn set_sinsts(sinsts: u32) {
    let mut l = LINKY.lock().await;
    l.sinsts = sinsts;
}

pub async fn get() -> Linky {
    let l = LINKY.lock().await;
    Linky {
        east: l.east,
        sinsts: l.sinsts,
    }
}
