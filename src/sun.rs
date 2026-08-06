use core::f64::consts::TAU;
use fasttime::{DateTime, OffsetDateTime, Time};
use libm::{acos, asin, atan2, cos, sin, sincos, tan};
// use smart_leds_trait::RGB;

use crate::{AstronDatetimeExt, DateExt, HorizontalCoordinate, delta_t_2000};

#[derive(Default, Clone, Copy)]
pub enum DayProgress {
    /// between 0.0 to 1.0
    Day(f64),
    #[default]
    Night,
}

impl core::fmt::Display for DayProgress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Day(day_progress) => write!(f, "{:.3} %", day_progress * 100.0),
            Self::Night => write!(f, "Night"),
        }
    }
}

impl DayProgress {
    /// convert to PWM duty cycle, between 0 to 100% during `Self::Day`,
    /// full 100% at `Self::Night`
    pub fn to_pwm_duty_cycle_percent(&self) -> u8 {
        if let Self::Day(day_prog) = self {
            (day_prog * 100.0) as u8
        } else {
            100
        }
    }
}

#[derive(Clone, Default)]
pub struct Sun {
    /// rise time, seconds since midnight
    rise_at: u32,
    /// set time, seconds since midnight
    set_at: u32,
    pos: HorizontalCoordinate,
    daytime_length: f64,
}

impl Sun {
    /// sun rise at local time
    #[inline(always)]
    pub fn rise_at(&self) -> Time {
        Time::from_seconds_nanos(self.rise_at, 0).unwrap()
    }

    /// sun set at local time
    #[inline(always)]
    pub fn set_at(&self) -> Time {
        Time::from_seconds_nanos(self.set_at, 0).unwrap()
    }

    /// sun current azimuth and altitude are in degrees
    #[inline]
    pub fn pos(&self) -> HorizontalCoordinate {
        self.pos
    }

    pub fn update(&mut self, now: &OffsetDateTime, lat: f64, lon: f64) {
        let (sunrise, sunset) = sunrise_sunset_utc_seconds(&now.utc, lat, lon);

        // convert to seconds since midnight in local time
        let (sunrise, sunset) = (
            sunrise + now.offset.as_seconds() as f64,
            sunset + now.offset.as_seconds() as f64,
        );

        // update state
        self.rise_at = sunrise as u32;
        self.set_at = sunset as u32;
        self.daytime_length = sunset - sunrise;
        self.pos = get_pos(&now.utc, lat, lon).apparent_altitude();
    }

    /// daytime progress, `None` if it's after sunset
    pub fn day_progress(&self, now_local: &Time) -> DayProgress {
        let now = now_local.seconds_since_midnight();

        // invalid rise/set time or after sunset or before sunrise
        if self.set_at < self.rise_at || self.set_at < now || self.rise_at > now {
            return DayProgress::Night;
        }

        // sunrise < now < sunset, so it should be safe to subtract two unsigned integers
        DayProgress::Day(
            (now.saturating_sub(self.rise_at) as f64 / self.daytime_length).clamp(0.0, 1.0),
        )
    }

    // pub fn color_at(&self, t: &Time) -> RGB<u8> {
    //     const NOON_COLOR: RGB<f32> = RGB::new(255.0, 254.0, 250.0);
    //     const END_OF_DAY_COLOR: RGB<f32> = RGB::new(255.0, 166.0, 87.0);

    //     if let DayProgress::Day(day_progress) = self.day_progress(t) {
    //         // blend
    //         let t = ((2.0 * day_progress - 1.0) * (2.0 * day_progress - 1.0)).clamp(0.0, 1.0); // smoother curve and clamp it between 0.0 and 1.0
    //         let sun_color = NOON_COLOR * (1.0 - t) + END_OF_DAY_COLOR * t;

    //         RGB::new(sun_color.r as u8, sun_color.g as u8, sun_color.b as u8)
    //     } else {
    //         RGB::new(0, 80, 255) // Moon color
    //     }
    // }
}

/// UTC time of sunrise and sunset in seconds since midnight
/// return in (`sunrise`, `sunset`)
/// derived from https://gml.noaa.gov/grad/solcalc/solareqns.PDF
fn sunrise_sunset_utc_seconds(now_utc: &DateTime, lat: f64, lon: f64) -> (f64, f64) {
    // fractional year in radians
    let frac_year = {
        let day_of_year = now_utc.date.ordinal() as f64;
        let days_per_year = now_utc.days_per_year() as f64;
        let frac_day = (now_utc.time.hour as f64 - 12.0) / 24.0;

        TAU * (day_of_year - 1.0 + frac_day) / days_per_year
    };
    let (frac_year_sin, frac_year_cos) = sincos(frac_year);
    let (double_frac_year_sin, double_frac_year_cos) = sincos(2.0 * frac_year);
    let (triple_frac_year_sin, triple_frac_year_cos) = sincos(3.0 * frac_year);

    // equation of time in minutes
    let eqtime = 229.18
        * (0.000075 + 0.001868 * frac_year_cos
            - 0.032077 * frac_year_sin
            - 0.014615 * double_frac_year_cos
            - 0.040849 * double_frac_year_sin);

    // solar declination angle in radians
    let dec = 0.006918 - 0.399912 * frac_year_cos + 0.070257 * frac_year_sin
        - 0.006758 * double_frac_year_cos
        + 0.000907 * double_frac_year_sin
        - 0.002697 * triple_frac_year_cos
        + 0.00148 * triple_frac_year_sin;

    let lat_rad = lat.to_radians();
    let zenith_angle = 90.8333_f64.to_radians(); // zenith angle in radians
    let ha_cos = cos(zenith_angle) / (cos(lat_rad) * cos(dec)) - tan(lat_rad) * tan(dec);
    let ha = acos(ha_cos).to_degrees(); // hour angle

    // UTC time of sunrise and sunset in minutes since midnight
    let sunrise = 720.0 - 4.0 * (lon + ha) - eqtime;
    let sunset = 720.0 - 4.0 * (lon - ha) - eqtime;

    (sunrise * 60.0, sunset * 60.0)
}

/// Sun's apparent equatorial coordinates, Meeus ch. 25. d = days since J2000 (TT);
/// return right asc and declination
/// ported from SunCalc: https://github.com/mourner/suncalc
fn sun_coord(d: f64) -> (f64, f64) {
    let t = d / 36525.0; // Julian centuries
    let l0 = (280.46646 + t * (36000.76983 + t * 0.0003032)).to_radians(); // 25.2 geometric mean longitude
    let m = (357.52911 + t * (35999.05029 - t * 0.0001537)).to_radians(); // 25.3 mean anomaly
    let (sin_m, cos_m) = sincos(m);
    let c = ((1.914602 - t * (0.004817 + t * 0.000014)) * sin_m + // equation of center
        (0.019993 - 0.000101 * t) * 2.0 * sin_m * cos_m + 0.000289 * sin_m * (3.0 - 4.0 * sin_m * sin_m)).to_radians();
    let o_m = (125.04 - 1934.136 * t).to_radians(); // longitude of the ascending node
    let lon_apparent = l0 + c - (0.00569 + 0.00478 * sin(o_m)).to_radians(); // apparent longitude (nutation + aberration)
    // 22.2 mean obliquity + 25.8 correction for apparent position
    let e = (23.439291 - t * (0.0130042 + t * (0.00000016 - t * 0.000000504))).to_radians()
        + (0.00256 * cos(o_m)).to_radians();

    let ra = atan2(cos(e) * sin(lon_apparent), cos(lon_apparent)); // 25.6
    let dec = asin(sin(e) * sin(lon_apparent)); // 25.7

    (ra, dec)
}

/// ported from SunCalc: https://github.com/mourner/suncalc
fn get_pos(now_utc: &DateTime, lat: f64, lon: f64) -> HorizontalCoordinate {
    let phi = lat.to_radians();
    let local_sidereal_time = now_utc.to_sidereal_time(lon).to_radians();
    let dt = delta_t_2000(now_utc.decimal_year()); // delta T is in days

    let jde = now_utc.to_julian() - 2451545.0; // Julian days epoch since J2000
    let jde = jde + dt;
    let (ra, dec) = sun_coord(jde);
    let hour_angle = local_sidereal_time - ra;

    HorizontalCoordinate::from_equatorial(hour_angle, phi, dec)
}
