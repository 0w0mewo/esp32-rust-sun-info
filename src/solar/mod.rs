use fasttime::{OffsetDateTime, Time};
use libm::{asin, cos, floor, fmod, round, sin};

use crate::{
    HorizontalCoordinate, QuadraticInterpolator, QuadraticRoots, SECONDS_PER_DAY,
    datetime::{DAY_PER_HOUR, J2000, MIDNIGHT, sidereal_time},
    sine_altitude,
    solar::{moon::moon_coord, sun::sun_coord},
};

pub mod moon;
pub mod sun;

#[derive(Clone, Copy)]
pub enum SolarObject {
    Sun,
    Moon,
}

#[derive(Clone, Default)]
pub struct EventInfo {
    /// rise/set event info
    pub jd: f64,
    /// rise/set azimuth, in degrees
    pub azimuth: f64,
}

impl EventInfo {
    /// time of the event
    pub fn time(&self) -> Time {
        let day_frac = self.jd - floor(self.jd) + 0.5; // fraction of a day, 0.0 is midnight
        let secs_since_midnight = fmod(day_frac * SECONDS_PER_DAY, SECONDS_PER_DAY);

        Time::from_seconds_nanos(round(secs_since_midnight) as u32, 0).unwrap_or(MIDNIGHT)
    }
}

impl SolarObject {
    /// ported from SunCalc: https://github.com/mourner/suncalc
    pub fn get_pos(&self, jd: f64, delta_t: f64, lat: f64, lon: f64) -> HorizontalCoordinate {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();
        let lst = sidereal_time(jd, lon_rad);

        let jde = jd - J2000 + delta_t;
        let (ra, dec, dist) = match self {
            SolarObject::Moon => moon_coord(jde),
            SolarObject::Sun => {
                let (ra, dec) = sun_coord(jde);
                (ra, dec, 0.0)
            }
        };
        let hour_angle = lst - ra;

        let mut pos = HorizontalCoordinate::from_equatorial(hour_angle, lat_rad, dec);
        if let SolarObject::Moon = self {
            let altitude_geocentric_rad = pos.altitude.to_radians();
            pos.altitude = (altitude_geocentric_rad
                - asin(6378.14 / dist * cos(altitude_geocentric_rad)))
            .to_degrees();
        }

        pos.apparent_altitude()
    }

    /// find rise and set JD by brute forcing the crossing point
    /// derive from 'Astronomy on the Personal Computer, ch 3'
    pub fn get_rise_set(
        &self,
        jd: f64,
        delta_t: f64,
        lat: f64,
        lon: f64,
        refraction_rad: f64,
    ) -> (Option<EventInfo>, Option<EventInfo>) {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();
        let jd0 = floor(jd) + 0.5;

        // refraction
        let refracted_sine_altitude = sin(refraction_rad);

        let mut jd_rise = None;
        let mut jd_set = None;

        let mut quadratic = QuadraticInterpolator::default();

        // sine altitude of the object from given hour offset of JD0
        let sin_altitude = |hr: f64| {
            let jd = jd0 + hr * DAY_PER_HOUR;
            let jde = jd - J2000 + delta_t;
            let (ra_rad, dec_rad) = match self {
                SolarObject::Moon => {
                    let (ra, dec, _) = moon_coord(jde);
                    (ra, dec)
                }
                SolarObject::Sun => sun_coord(jde),
            };

            let hr_angle_rad = sidereal_time(jd, lon_rad) - ra_rad;

            sine_altitude(dec_rad, lat_rad, hr_angle_rad) - refracted_sine_altitude
        };

        // search for rise/set in 24 hours interval
        let mut hour_offset = 1.0;
        let mut y_minus = sin_altitude(0.0);
        while hour_offset < 25.0 || (jd_rise.is_none() && jd_set.is_none()) {
            let y0 = sin_altitude(hour_offset);
            let y_plus = sin_altitude(hour_offset + 1.0);

            // searching altitude = 0 degree by searching y = 0 where y might between y_minus, y0 and y_plus
            quadratic.fit(y_minus, y0, y_plus);
            if let Some(roots) = quadratic.roots() {
                match roots {
                    QuadraticRoots::One { root } => {
                        let t = hour_offset + root;
                        let t = t * DAY_PER_HOUR + jd0;
                        if y_minus < 0.0 {
                            jd_rise.replace(t);
                        } else {
                            jd_set.replace(t);
                        }
                    }
                    QuadraticRoots::Two { root1, root2 } => {
                        let t1 = hour_offset + root1;
                        let t1 = t1 * DAY_PER_HOUR + jd0;
                        let t2 = hour_offset + root2;
                        let t2 = t2 * DAY_PER_HOUR + jd0;
                        if quadratic.y_extremum() < 0.0 {
                            jd_rise.replace(t2);
                            jd_set.replace(t1);
                        } else {
                            jd_rise.replace(t1);
                            jd_set.replace(t2);
                        }
                    }
                }
            }

            // advance the start point
            y_minus = y_plus;
            hour_offset += 2.0;
        }

        let to_event_info = |jd| {
            let HorizontalCoordinate { azimuth, .. } = self.get_pos(jd, delta_t, lat, lon);

            EventInfo { jd, azimuth }
        };
        let rise = jd_rise.map(to_event_info);
        let set = jd_set.map(to_event_info);

        (rise, set)
    }
}

pub trait PlanetUpdater {
    /// update horizontal position
    fn update_pos(&mut self, now: &OffsetDateTime, lat: f64, lon: f64);
    /// update atronomical events, such as rise time, set time, etc
    fn update_astron(&mut self, now: &OffsetDateTime, lat: f64, lon: f64);
}
