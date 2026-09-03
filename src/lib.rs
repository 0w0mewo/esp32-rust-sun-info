#![no_std]
pub mod board;
pub mod config;
pub mod datetime;
pub mod events;
pub mod ntp;
pub mod solar;
pub mod ui;

pub type SSD1306<DI> = ssd1306::Ssd1306Async<
    DI,
    ssd1306::size::DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsModeAsync<ssd1306::size::DisplaySize128x64>,
>;

use core::f64::consts::PI;

use esp_hal::rng;
use libm::{asin, atan2, cos, fmod, sin, tan};

use crate::datetime::DAYS_PER_JULIAN_CENTURY;

pub const MICROSECS_PER_SEC: u64 = 1_000_000;
const SECONDS_PER_DAY: f64 = 24.0 * 3600.0;

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
pub fn astro_refraction(h: f64) -> f64 {
    let h = h.to_radians().max(0.0); // formula valid for positive altitudes only

    // Meeus 16.4: 1.02 / tan(h + 10.26 / (h + 5.10)), h in degrees, arcmin result — folded into degree
    0.0002967 / libm::tan(h + 0.00312536 / (h + 0.08901179)).to_degrees()
}
