#[cfg(feature = "defmt")]
use defmt::*;

use chrono::{DateTime, NaiveDateTime, Utc};
use embassy_stm32::rtc::{Rtc, RtcTimeProvider};

pub struct RtcClock {
    rtc: Rtc,
    time_provider: RtcTimeProvider,
}

impl RtcClock {
    pub fn new(
        rtc: Rtc,
        time_provider: RtcTimeProvider,
    ) -> Self {
        Self {
            rtc,
            time_provider,
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
        #[cfg(feature = "defmt")]
        info!("setting RTC timestamp: {}", timestamp);

        // Unix timestamp -> chrono UTC DateTime
        let datetime: DateTime<Utc> = DateTime::from_timestamp(timestamp as i64, 0).ok_or(())?;

        // chrono NaiveDateTime -> embassy DateTime
        let datetime = datetime.naive_utc().into();

        self.rtc.set_datetime(datetime).map_err(|_| ())?;

        Ok(())
    }
}
//
// let now = NaiveDate::from_ymd_opt(2026, 9, 14).unwrap().and_hms_opt(17, 00, 15).unwrap();
