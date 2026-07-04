use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

pub struct Linky {
    pub east: u32,
    pub sinst: u32,
}

static LINKY: Mutex<CriticalSectionRawMutex, Linky> = Mutex::new(Linky{east: 0u32, sinst: 0u32});

pub async fn set(east: u32, sinst: u32) {
    let mut l = LINKY.lock().await;
    l.east = east;
    l.sinst = sinst;
}

pub async fn get() -> Linky {
    let l = LINKY.lock().await;
    Linky {
        east: l.east,
        sinst: l.sinst,
    }
}
