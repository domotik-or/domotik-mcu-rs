#[cfg(feature = "defmt")]
use defmt::*;

use chrono::{DateTime, NaiveDateTime};
use embassy_stm32::rtc::{Rtc, RtcTimeProvider};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

const RTC_MAGIC: u32 = 0x5254_4321;
const RTC_MAGIC_REGISTER: usize = 0;

pub type SharedRtc = Mutex<CriticalSectionRawMutex, RtcClock>;

pub struct RtcClock {
    rtc: Rtc,
    time_provider: RtcTimeProvider,
    needs_initialization: bool,
}

impl RtcClock {
    pub fn new(
        rtc: Rtc,
        time_provider: RtcTimeProvider,
    ) -> Self {
        let needs_initialization = rtc.read_backup_register(RTC_MAGIC_REGISTER) != Some(RTC_MAGIC);

        Self {
            rtc,
            time_provider,
            needs_initialization
        }
    }

    pub fn get_datetime(&self) -> Result<u64, ()> {
        // RTC -> embassy DateTime
        let datetime = self.time_provider.now().map_err(|_| ())?;

        // embassy DateTime -> chrono NaiveDateTime
        let datetime: NaiveDateTime = datetime.into();

        // chrono -> Unix timestamp
        Ok(datetime.and_utc().timestamp() as u64)
    }

    pub fn set_datetime(&mut self, timestamp: u64) -> Result<(), ()> {
        if !self.needs_initialization {
            return Ok(());
        }

        // Unix timestamp -> chrono UTC DateTime
        let datetime = DateTime::from_timestamp(timestamp as i64, 0,).ok_or(())?;

        // chrono NaiveDateTime -> embassy DateTime
        let datetime: NaiveDateTime = datetime.naive_utc().into();

        self.rtc.set_datetime(datetime.into()).map_err(|_| ())?;

        self.rtc.write_backup_register(RTC_MAGIC_REGISTER, RTC_MAGIC);

        self.needs_initialization = false;

        #[cfg(feature = "defmt")]
        info!("RTC initialized: {}", timestamp);

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.rtc.read_backup_register(RTC_MAGIC_REGISTER) == Some(RTC_MAGIC)
    }

    pub fn invalidate(&self) {
        self.rtc.write_backup_register( RTC_MAGIC_REGISTER, 0);
    }
}
