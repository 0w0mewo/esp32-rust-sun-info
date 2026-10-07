use libm::{asin, cos, floor, fmod, round, sin};
use time::{Duration, OffsetDateTime, Time, UtcDateTime};

use crate::{
    HorizontalCoordinate, QuadraticInterpolator, QuadraticRoots, SECONDS_PER_DAY,
    datetime::{AstronDatetimeExt, DAY_PER_HOUR, J2000, sidereal_time},
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
}

impl SolarObject {
    /// ported from SunCalc: https://github.com/mourner/suncalc
    fn get_pos(&self, jd: f64, delta_t: f64, lat: f64, lon: f64) -> HorizontalCoordinate {
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

    /// find upcoming rise and set events,
    /// it searches forward 24 hours refers to the local midnight
    ///
    /// the event JD is in UT
    #[inline]
    fn get_rise_set(
        &self,
        now: &OffsetDateTime,
        lat: f64,
        lon: f64,
        refracted_horizon_rad: f64,
    ) -> (Option<EventInfo>, Option<EventInfo>) {
        self.get_rise_set_with_range(now, 0, 24, lat, lon, refracted_horizon_rad)
    }

    /// find upcoming rise and set events in Stellarium's planets visibility style, which is
    /// searching 12 hours forward and 12 hours backward, refers to the local midnight
    ///
    /// the event JD is in UT
    #[inline]
    fn get_rise_set_tonight(
        &self,
        now: &OffsetDateTime,
        lat: f64,
        lon: f64,
        refracted_horizon_rad: f64,
    ) -> (Option<EventInfo>, Option<EventInfo>) {
        // the datetime should shift 12 hours forward if it's local afternoon in order to obtain
        // the rise/set time at the center of midnight of next day rather than the midnight of today
        let start_at = if now.time() >= Time::from_hms(12, 0, 0).unwrap() {
            &now.saturating_add(Duration::hours(12))
        } else {
            now
        };

        self.get_rise_set_with_range(start_at, 12, 12, lat, lon, refracted_horizon_rad)
    }

    /// find upcoming rise and set events by brute forcing the crossing point,
    /// it searches `hr_forward` hours forward and `hr_backward` hours backward refers to the local midnight
    ///
    /// the event JD is in UT
    ///
    /// derive from 'Astronomy on the Personal Computer, ch 3'
    fn get_rise_set_with_range(
        &self,
        now: &OffsetDateTime,
        hr_backward: u8,
        hr_forward: u8,
        lat: f64,
        lon: f64,
        refracted_horizon_rad: f64,
    ) -> (Option<EventInfo>, Option<EventInfo>) {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();

        // local midnight in UTC
        let midnight = { now.replace_time(time::Time::MIDNIGHT).to_utc() };

        let jd0 = midnight.to_julian();
        let dt_days = midnight.delta_t(); // delta T

        // refraction
        let refracted_sine_horizon_altitude = sin(refracted_horizon_rad);

        let mut jd_rise = None;
        let mut jd_set = None;

        let mut quadratic = QuadraticInterpolator::default();

        // sine altitude of the object from given hour offset of JD0
        let sin_altitude = |hr: f64| {
            let jd = jd0 + hr * DAY_PER_HOUR;
            let jde = jd - J2000 + dt_days;
            let (ra_rad, dec_rad) = match self {
                SolarObject::Moon => {
                    let (ra, dec, _) = moon_coord(jde);
                    (ra, dec)
                }
                SolarObject::Sun => sun_coord(jde),
            };

            let hr_angle_rad = sidereal_time(jd, lon_rad) - ra_rad;

            sine_altitude(dec_rad, lat_rad, hr_angle_rad) - refracted_sine_horizon_altitude
        };

        // search for any rise/set in jd0 - hr_backward hours to jd0 + hr_forward hours interval
        let mut hour_offset = -(hr_backward as f64) + 1.0;
        let mut y_minus = sin_altitude(hour_offset - 1.0);
        while hour_offset <= hr_forward as f64 {
            if jd_rise.is_some() && jd_set.is_some() {
                break;
            }

            let y0 = sin_altitude(hour_offset);
            let y_plus = sin_altitude(hour_offset + 1.0);

            // searching altitude = 0 degree by searching y = 0 where y might between y_minus, y0 and y_plus
            quadratic.fit(y_minus, y0, y_plus);
            if let Some(roots) = quadratic.roots() {
                match roots {
                    QuadraticRoots::One { root } => {
                        let t = hour_offset + root;
                        let t = t * DAY_PER_HOUR + jd0; // decimal hour to JD

                        if y_minus < 0.0 {
                            jd_rise.replace(t);
                        } else {
                            jd_set.replace(t);
                        }
                    }
                    QuadraticRoots::Two { root1, root2 } => {
                        let t1 = hour_offset + root1;
                        let t2 = hour_offset + root2;
                        let mut t1 = t1 * DAY_PER_HOUR + jd0; // decimal hour to JD
                        let mut t2 = t2 * DAY_PER_HOUR + jd0;

                        if quadratic.y_extremum() < 0.0 {
                            core::mem::swap(&mut t1, &mut t2);
                        }

                        jd_rise.replace(t1);
                        jd_set.replace(t2);
                    }
                }
            }

            // advance the start point
            y_minus = y_plus; // the last is the first on the next round
            hour_offset += 2.0; // searching window size is 3
        }

        let to_event_info = |jd| {
            let HorizontalCoordinate { azimuth, .. } = self.get_pos(jd, dt_days, lat, lon);

            EventInfo { jd, azimuth }
        };
        let rise = jd_rise.map(to_event_info);
        let set = jd_set.map(to_event_info);

        (rise, set)
    }
}

pub trait PlanetUpdater {
    /// update horizontal position
    fn update_pos(&mut self, now: &UtcDateTime, lat: f64, lon: f64);
    /// update atronomical events, such as rise time, set time, etc
    fn update_astron(&mut self, now: &OffsetDateTime, lat: f64, lon: f64);
    /// get rise azimuth
    fn rise_azimuth(&self) -> f64;
    /// get set azimuth
    fn set_azimuth(&self) -> f64;
    /// rise datetime in UTC, return `None` if no rise event
    fn rise_at(&self) -> Option<f64>;
    /// set datetime in UTC, return `None` if no set event
    fn set_at(&self) -> Option<f64>;
    /// current altitude and azimuth in degrees
    fn pos(&self) -> HorizontalCoordinate;
    /// who are you?
    fn planet(&self) -> SolarObject;
}
