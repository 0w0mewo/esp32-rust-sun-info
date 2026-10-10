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
use libm::{asin, atan2, cos, fmod, sin, sincos, sqrt, tan};

use crate::datetime::DAYS_PER_JULIAN_CENTURY;

pub const MICROSECS_PER_SEC: u64 = 1_000_000;
const SECONDS_PER_DAY: f64 = 24.0 * 3600.0;
pub const HOUR_PER_RAD: f64 = (1.0_f64 / 15.0).to_radians();

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

        let altitude = asin(sine_altitude(dec, lat, ha)).to_degrees();

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

pub fn sine_altitude(dec_rad: f64, lat_rad: f64, ha_rad: f64) -> f64 {
    let (dec_sin, dec_cos) = sincos(dec_rad);
    let (lat_sin, lat_cos) = sincos(lat_rad);

    lat_sin * dec_sin + lat_cos * dec_cos * cos(ha_rad)
}

/// ported from SunCalc: https://github.com/mourner/suncalc
pub fn astro_refraction(h: f64) -> f64 {
    let h = h.to_radians().max(0.0); // formula valid for positive altitudes only

    // Meeus 16.4: 1.02 / tan(h + 10.26 / (h + 5.10)), h in degrees, arcmin result — folded into degree
    0.0002967 / libm::tan(h + 0.00312536 / (h + 0.08901179)).to_degrees()
}

#[derive(Clone)]
pub enum QuadraticRoots {
    /// one root
    One { root: f64 },
    /// two roots
    Two { root1: f64, root2: f64 },
}

/// fit to y = a*x^2 + b*x + c
#[derive(Default)]
pub struct QuadraticInterpolator {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    xe: f64,
    ye: f64,
}

impl QuadraticInterpolator {
    /// x extremum of the fitted quadratic
    pub fn x_extremum(&self) -> f64 {
        self.xe
    }

    /// y extremum of the fitted quadratic
    pub fn y_extremum(&self) -> f64 {
        self.ye
    }

    /// compute roots of the fitted quadratic, i.e, the x value of the quadratic when y = 0
    ///
    /// return None if no roots are found
    pub fn roots(&self) -> Option<QuadraticRoots> {
        let dis = self.b * self.b - 4.0 * self.a * self.c;
        if dis < 0.0 {
            return None;
        }

        let dx = 0.5 * sqrt(dis) / self.a.abs();
        let mut root1 = self.xe - dx;
        let root2 = self.xe + dx;

        let mut nroots = 0;
        if root1.abs() <= 1.0 {
            nroots += 1;
        }

        if root2.abs() <= 1.0 {
            nroots += 1;
        }

        if root1 < -1.0 {
            root1 = root2;
        }

        match nroots {
            0 => None,
            1 => Some(QuadraticRoots::One { root: root1 }),
            2 => Some(QuadraticRoots::Two { root1, root2 }),
            _ => unreachable!(),
        }
    }

    /// quadratic interpolation from given y values
    pub fn fit(&mut self, y_start: f64, y_mid: f64, y_end: f64) {
        self.a = 0.5 * (y_end + y_start) - y_mid;
        self.b = 0.5 * (y_end - y_start);
        self.c = y_mid;

        self.xe = -self.b / (2.0 * self.a);
        self.ye = (self.a * self.xe + self.b) * self.xe + self.c;
    }
}
