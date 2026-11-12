use core::f64::consts::TAU;

use libm::{cos, floor, fmod, round, sincos, trunc};
use time::{PlainDateTime, UtcDateTime};

use crate::{
    SECONDS_PER_DAY,
    datetime::{AstronDatetimeExt, DAYS_PER_JULIAN_CENTURY, J2000, UtOffsetExt},
    math::{RADS_PER_ARCSECOND, wrap_2pi},
    solar::{coord::HorizontalCoordinate, nutation::nutation_iau2006, planet::SolarObject},
};

pub mod coord;
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

fn sine_altitude(dec_rad: f64, lat_rad: f64, ha_rad: f64) -> f64 {
    let (dec_sin, dec_cos) = sincos(dec_rad);
    let (lat_sin, lat_cos) = sincos(lat_rad);

    lat_sin * dec_sin + lat_cos * dec_cos * cos(ha_rad)
}

/// ported from SunCalc: https://github.com/mourner/suncalc
fn astro_refraction(h: f64) -> f64 {
    if h < -1.0 {
        return 0.0;
    }

    // Meeus 16.4: 1.02 / tan(h + 10.26 / (h + 5.10)), h in degrees, arcmin result
    let refrac_arcm = 1.02 / libm::tan((h + 10.3 / (h + 5.11)).to_radians());

    // to degrees
    refrac_arcm / 60.0
}

/// local apparent sidereal time in radians
pub fn apparent_sidereal_time(jd_ut: f64, dt_days: f64, lon_rad: f64) -> f64 {
    let jde = jd_ut + dt_days;
    let t = (jde - J2000) / DAYS_PER_JULIAN_CENTURY;

    let gmst_rad = gmst(jd_ut, dt_days);

    let (dpsi, deps) = nutation_iau2006(t);
    let eps = mean_obliq(t) + deps; // true obliq
    let eqeq = cos(eps) * dpsi; // equation of equinox

    wrap_2pi(gmst_rad + lon_rad + eqeq)
}

/// local mean sidereal time in radians
pub fn mean_sidereal_time(jd_ut: f64, dt_days: f64, lon_rad: f64) -> f64 {
    let gmst_rad = gmst(jd_ut, dt_days);

    wrap_2pi(gmst_rad + lon_rad)
}

/// Greenwich mean sidereal time in radians
fn gmst(jd_ut: f64, dt_days: f64) -> f64 {
    let jd_tt = jd_ut + dt_days;

    let t = (jd_tt - J2000) / DAYS_PER_JULIAN_CENTURY;
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;

    let era_rad = earth_rotation_angle(jd_ut);

    era_rad
        + RADS_PER_ARCSECOND
            * (0.014506 + 4612.156534 * t + 1.3915817 * t2
                - 0.00000044 * t3
                - 0.000029956 * t4
                - 0.0000000368 * t5)
}

/// Earth rotation angle in radians
fn earth_rotation_angle(jd_ut: f64) -> f64 {
    let t = jd_ut - J2000;
    let f = jd_ut - trunc(jd_ut);

    TAU * (f + 0.7790572732640 + 0.00273781191135448 * t)
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
