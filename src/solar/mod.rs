use libm::{floor, fmod, round};
use time::{PlainDateTime, UtcDateTime};

use crate::{
    HorizontalCoordinate, RADS_PER_ARCSECOND, SECONDS_PER_DAY,
    datetime::{AstronDatetimeExt, UtOffsetExt},
    solar::planet::SolarObject,
};

pub mod moon;
pub mod npb;
pub mod nutation;
pub mod planet;
pub mod sun;

/// obliquity at J2000 in radians
pub const MEAN_OBLIQUITY_J2000_RAD: f64 = mean_obliq(0.0);
// astronomical unit in km
pub const ASTRONOMICAL_UNIT_KM: f64 = 149597870700_f64 / 1000.0;

#[derive(Clone, Default)]
pub struct EventInfo {
    /// rise/set event JD in UT
    pub jd: f64,
    /// rise/set azimuth, in degrees
    pub azimuth: f64,
}

impl EventInfo {
    /// event time in seconds since midnight in local time
    pub fn seconds_since_midnight_local(&self, ut_offset_days: f64) -> u32 {
        let jd_local = self.jd + ut_offset_days;
        let day_frac = jd_local - floor(jd_local) + 0.5; // fraction of a day, 0.0 is midnight

        round(fmod(day_frac * SECONDS_PER_DAY, SECONDS_PER_DAY)) as u32
    }

    /// event time in seconds since midnight in UTC
    #[inline]
    pub fn seconds_since_midnight(&self) -> u32 {
        self.seconds_since_midnight_local(0.0)
    }

    /// event datetime in UTC
    #[inline]
    pub fn event_datetime_utc(&self) -> UtcDateTime {
        UtcDateTime::from_julian(self.jd)
    }

    /// convert event JD to `PlainDatetime` in local timezone
    #[inline]
    pub fn event_datetime_local(&self) -> PlainDateTime {
        self.event_datetime_utc().to_local_with_dst()
    }
}

/// Mean obliquity of the ecliptic, IAU 2006 precession model,
/// `t` is julian centuries since JD2000
/// return obliquity of ecliptic in radians
const fn mean_obliq(t: f64) -> f64 {
    let eps_arcsec = 84381.406 - 46.836769 * t - 0.0001831 * t * t + 0.00200340 * t * t * t
        - 0.000000576 * t * t * t * t
        - 0.0000000434 * t * t * t * t * t;

    eps_arcsec * RADS_PER_ARCSECOND
}

pub trait PlanetUpdater {
    /// update horizontal position
    fn update_pos(&mut self, now: &UtcDateTime, lat: f64, lon: f64);
    /// update rise/set events, `local_midnight` should be
    /// the local midnight in UTC
    fn update_riseset(&mut self, local_midnight: &UtcDateTime, lat: f64, lon: f64);
    /// get rise event
    fn rise(&self) -> Option<&EventInfo>;
    /// get set event
    fn set(&self) -> Option<&EventInfo>;
    /// current altitude and azimuth in degrees
    fn pos(&self) -> HorizontalCoordinate;
    /// who are you?
    fn planet(&self) -> SolarObject;
}
