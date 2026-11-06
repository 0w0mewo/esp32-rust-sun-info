use core::f64::consts::PI;

use libm::{asin, atan2, cos, fmod, sin, tan};

use crate::solar::{astro_refraction, sine_altitude};

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
