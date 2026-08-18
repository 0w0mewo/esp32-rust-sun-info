use fasttime::{DateTime, OffsetDateTime};
use libm::{asin, cos};

use crate::{
    AstronDatetimeExt, DateExt, HorizontalCoordinate, delta_t_2000,
    solar::{moon::moon_coord, sun::sun_coord},
};

pub mod moon;
pub mod sun;

#[derive(Clone, Copy)]
pub enum SolarObject {
    Sun,
    Moon,
}

/// ported from SunCalc: https://github.com/mourner/suncalc
fn get_pos(now_utc: &DateTime, lat: f64, lon: f64, obj: SolarObject) -> HorizontalCoordinate {
    let phi = lat.to_radians();
    let local_sidereal_time = now_utc.to_sidereal_time(lon).to_radians();
    let dt = delta_t_2000(now_utc.decimal_year()); // delta T is in days

    let jde = now_utc.to_julian_epoch_2000();
    let jde = jde + dt;
    let (ra, dec, dist) = match obj {
        SolarObject::Moon => moon_coord(jde),
        SolarObject::Sun => {
            let (ra, dec) = sun_coord(jde);
            (ra, dec, 0.0)
        }
    };
    let hour_angle = local_sidereal_time - ra;

    let mut pos = HorizontalCoordinate::from_equatorial(hour_angle, phi, dec);
    if let SolarObject::Moon = obj {
        let altitude_geocentric_rad = pos.altitude.to_radians();
        pos.altitude = (altitude_geocentric_rad
            - asin(6378.14 / dist * cos(altitude_geocentric_rad)))
        .to_degrees();
    }

    pos.apparent_altitude()
}

pub trait PlanetUpdater {
    /// update horizontal position
    fn update_pos(&mut self, now: &OffsetDateTime, lat: f64, lon: f64);
    /// update atronomical events, such as rise time, set time, etc
    fn update_astron(&mut self, now: &OffsetDateTime, lat: f64, lon: f64);
}
