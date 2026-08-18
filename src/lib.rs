#![no_std]
pub mod board;
pub mod events;
pub mod ntp;
pub mod solar;
pub mod ui;

pub const MICROSECS_PER_SEC: u64 = 1_000_000;
const SECONDS_PER_DAY: f64 = 24.0 * 3600.0;
pub const J2000: f64 = 2451545.0;
pub const DAYS_PER_JULIAN_CENTURY: f64 = 36525.0;

pub const D2000: Date = Date::from_ymd_unchecked(2000, 1, 1);
pub const MIDNIGHT: Time = Time {
    hour: 0,
    minute: 0,
    second: 0,
    nanosecond: 0,
};

pub type SSD1306<DI> = ssd1306::Ssd1306Async<
    DI,
    ssd1306::size::DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsModeAsync<ssd1306::size::DisplaySize128x64>,
>;

use core::f64::consts::PI;

use esp_hal::rng;
use fasttime::{Date, DateTime, Time};
use libm::{asin, atan2, cos, floor, fmod, sin, tan};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("fail to connect to wifi")]
    WifiLinkTimeout,
    #[error("fail to obtain IP address")]
    IpAddrTimeout,
    #[error("draw error")]
    DrawError,
    #[error("network error")]
    NetworkError,
}

pub fn rand_u64() -> u64 {
    let rand = rng::Rng::new();
    rand.random() as u64 | ((rand.random() as u64) << 32)
}

pub trait AstronDatetimeExt: DateExt {
    /// convert from julian days to civil datetime
    /// Note: f64 used here because f32 was not precise enough
    fn from_julian(jd: f64) -> Self;

    /// convert to julian days
    fn to_julian(&self) -> f64;

    /// convert to julian days epoch since J2000
    fn to_julian_epoch_2000(&self) -> f64 {
        self.to_julian() - J2000
    }

    /// convert to julian centuries
    fn to_julian_centuries(&self) -> f64 {
        self.to_julian() / DAYS_PER_JULIAN_CENTURY
    }

    /// local sidereal time in degrees, assume the current datetime is in UT
    #[inline]
    fn to_sidereal_time(&self, lon: f64) -> f64 {
        let jd = self.to_julian_epoch_2000();
        sidereal_time(jd, lon)
    }

    /// local sidereal time in HMS, assume the current datetime is in UT
    fn to_sidereal_time_hms(&self, lon: f64) -> (u8, u8, u8) {
        let hr = self.to_sidereal_time(lon) / 15.0;
        let h = floor(hr);
        let m_decimal = (hr - h) * 60.0;
        let m = floor(m_decimal);
        let s = (m_decimal - m) * 60.0;

        (h as u8, m as u8, s as u8)
    }
}

pub trait DateExt {
    /// is the current year leap year
    fn is_leap_year(&self) -> bool;

    /// decimal year
    fn decimal_year(&self) -> f64;

    /// how many days in the current year, 366 days if it's leap year, 365 days otherwise
    fn days_per_year(&self) -> u16 {
        if self.is_leap_year() { 366 } else { 365 }
    }
}

impl AstronDatetimeExt for DateTime {
    fn from_julian(jd: f64) -> Self {
        let unix_secs = ((jd - 2440587.5) * SECONDS_PER_DAY) as i64;
        DateTime::from_unix_timestamp(unix_secs, 0).unwrap()
    }

    fn to_julian(&self) -> f64 {
        self.unix_timestamp() as f64 / SECONDS_PER_DAY + 2440587.5
    }
}

impl AstronDatetimeExt for Date {
    fn from_julian(jd: f64) -> Self {
        Date::from_days_since_unix_epoch((jd - 2440587.5) as i64).unwrap()
    }

    fn to_julian(&self) -> f64 {
        self.days_since_unix_epoch() as f64 + 2440587.5
    }
}

impl DateExt for DateTime {
    fn is_leap_year(&self) -> bool {
        self.date.is_leap_year()
    }

    fn decimal_year(&self) -> f64 {
        self.date.decimal_year()
    }
}

impl DateExt for Date {
    fn is_leap_year(&self) -> bool {
        let year = self.year;
        let century_candidate = year % 25 == 0;
        (year & if century_candidate { 15 } else { 3 }) == 0
    }

    fn decimal_year(&self) -> f64 {
        self.ordinal() as f64 / self.days_per_year() as f64 + self.year as f64
    }
}

/// `azimuth` and `altitude` are in degrees
#[derive(Debug, Default, Clone, Copy)]
pub struct HorizontalCoordinate {
    /// north-based clockwise azimuth in degrees (0 = N, 90 = E, 180 = S, 270 = W)
    pub azimuth: f64,
    /// altitude in degrees
    pub altitude: f64,
}

impl HorizontalCoordinate {
    /// convert horizontal coordinate from equatorial coordinate
    /// `ha`: local hour angle in radians
    /// `lat`: latitude of observer on Earth in radians
    /// `dec`: declination in radians
    pub fn from_equatorial(ha: f64, lat: f64, dec: f64) -> Self {
        let az_rad = atan2(sin(ha), cos(ha) * sin(lat) - tan(dec) * cos(lat));
        let az_rad = PI + az_rad;

        let az_deg = fmod(az_rad.to_degrees(), 360.0);
        let az_deg = if az_deg < 0.0 { az_deg + 360.0 } else { az_deg };

        let altitude = altitude(dec, lat, ha).to_degrees();

        Self {
            altitude,
            azimuth: az_deg,
        }
    }

    /// encounter atomsphere refraction
    pub fn apparent_altitude(mut self) -> Self {
        self.altitude = self.altitude + astro_refraction(self.altitude);

        self
    }
}

impl core::fmt::Display for HorizontalCoordinate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "az: {:.4}, alt: {:.4}", self.azimuth, self.altitude)
    }
}

pub fn altitude(dec_rad: f64, lat_rad: f64, ha_rad: f64) -> f64 {
    asin(sin(lat_rad) * sin(dec_rad) + cos(lat_rad) * cos(dec_rad) * cos(ha_rad))
}

/// ported from SunCalc: https://github.com/mourner/suncalc
fn astro_refraction(h: f64) -> f64 {
    let h = h.to_radians().max(0.0); // formula valid for positive altitudes only

    // Meeus 16.4: 1.02 / tan(h + 10.26 / (h + 5.10)), h in degrees, arcmin result — folded into degree
    0.0002967 / libm::tan(h + 0.00312536 / (h + 0.08901179)).to_degrees()
}

/// Espenak & Meeus polynomial of delta T for 2005 to 2050,
/// return delta T in days.
/// `y`: decimal year between 2005.0 to 2050.0
pub fn delta_t_2000(y: f64) -> f64 {
    let t = y - 2000.0;

    (62.92 + 0.32217 * t + 0.005589 * t * t) / SECONDS_PER_DAY
}

pub fn sidereal_time(jd2000: f64, lon: f64) -> f64 {
    let t = jd2000 / DAYS_PER_JULIAN_CENTURY;
    let gmst =
        280.46061837 + 360.98564736629 * jd2000 + 0.000387933 * t * t - (t * t * t) / 38710000.0;

    let lst = (gmst + lon) % 360.0;
    if lst < 0.0 { lst + 360.0 } else { lst }
}
