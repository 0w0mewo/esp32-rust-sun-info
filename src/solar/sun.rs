use fasttime::{DateTime, OffsetDateTime};
use libm::{asin, atan2, cos, floor, sin, sincos};
use smart_leds::{RGB, RGB8};

use crate::{
    DAYS_PER_JULIAN_CENTURY, HorizontalCoordinate,
    datetime::{AstronDatetimeExt, J2000, delta_t_2000},
    solar::{EventInfo, PlanetUpdater, SolarObject},
};

const TWILIGHT_REFRACTION_RAD: f64 = -6.0f64.to_radians();
const SOLAR_EDGE_REFRACTION_RAD: f64 = (-50.0f64 / 60.0).to_radians();

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
    /// full 100% at `Self::Night`,
    /// 0% -> 100% before noon, 100% -> 0% after noon
    pub fn to_pwm_duty_cycle_percent(&self) -> u8 {
        if let Self::Day(day_prog) = self {
            // ramping up before noon, ramping down after noon
            ((1.0 - 2.0 * (day_prog - 0.5).abs()).max(0.0) * 100.0) as u8
        } else {
            100
        }
    }
}

#[derive(Clone, Default)]
pub struct Sun {
    /// rise info
    rise: Option<EventInfo>,
    /// set info
    set: Option<EventInfo>,
    /// dawn info
    dawn: Option<EventInfo>,
    /// dusk info
    dusk: Option<EventInfo>,
    /// current position
    pos: HorizontalCoordinate,
    /// day length in fractional day between [0, 1]
    daytime_length: f64,
}

impl PlanetUpdater for Sun {
    fn update_pos(&mut self, utc_now: &DateTime, lat: f64, lon: f64) {
        self.pos = SolarObject::Sun.get_pos(utc_now.to_julian(), utc_now.delta_t(), lat, lon);
    }

    fn update_astron(&mut self, now: &OffsetDateTime, lat: f64, lon: f64) {
        // start searching at local midnight
        let (rise, set) = SolarObject::Sun.get_rise_set(now, lat, lon, SOLAR_EDGE_REFRACTION_RAD);
        let (dawn, dusk) = SolarObject::Sun.get_rise_set(now, lat, lon, TWILIGHT_REFRACTION_RAD);

        // daytime length
        if let (Some(rise), Some(set)) = (rise.as_ref(), set.as_ref()) {
            self.daytime_length = (set.jd - rise.jd).abs();
        }

        self.rise = rise;
        self.set = set;
        self.dawn = dawn;
        self.dusk = dusk;
    }

    #[inline]
    fn rise_azimuth(&self) -> f64 {
        self.rise
            .as_ref()
            .map(|event_info| event_info.azimuth)
            .unwrap_or_default()
    }

    #[inline]
    fn set_azimuth(&self) -> f64 {
        self.set
            .as_ref()
            .map(|event_info| event_info.azimuth)
            .unwrap_or_default()
    }

    #[inline(always)]
    fn set_at(&self) -> Option<f64> {
        self.set.as_ref().map(|ev_info| ev_info.jd)
    }

    #[inline]
    fn rise_at(&self) -> Option<f64> {
        self.rise.as_ref().map(|ev_info| ev_info.jd)
    }

    #[inline]
    fn pos(&self) -> HorizontalCoordinate {
        self.pos
    }

    #[inline]
    fn planet(&self) -> SolarObject {
        SolarObject::Sun
    }
}

impl Sun {
    /// sun dawn at UTC
    #[inline]
    pub fn dawn_at(&self) -> Option<DateTime> {
        self.dawn
            .as_ref()
            .map(|event_info| DateTime::from_julian(event_info.jd))
    }

    /// sun dusk at UTC
    #[inline]
    pub fn dusk_at(&self) -> Option<DateTime> {
        self.dusk
            .as_ref()
            .map(|event_info| DateTime::from_julian(event_info.jd))
    }

    /// daytime progress, `None` if it's after sunset
    pub fn day_progress(&self, now: &OffsetDateTime) -> DayProgress {
        let now = now.utc.to_julian();

        // convert rise/set time in seconds since midnight local time, the `EventInfo` assume the event time is in UTC
        let rise = self.rise.as_ref().map(|ev| ev.jd).unwrap_or_default();
        let set = self.set.as_ref().map(|ev| ev.jd).unwrap_or_default();

        // invalid rise/set time or after sunset or before sunrise
        if set < rise || set < now || rise > now {
            return DayProgress::Night;
        }

        // sunrise < now < sunset, so it should be safe to subtract two unsigned integers
        let day_prog = (now - rise) / self.daytime_length;
        DayProgress::Day(day_prog.clamp(0.0, 1.0))
    }

    pub fn color_at(&self, now: DayProgress) -> RGB8 {
        const NOON_COLOR: RGB<f64> = RGB::new(255.0, 254.0, 250.0);
        const END_OF_DAY_COLOR: RGB<f64> = RGB::new(255.0, 166.0, 87.0);

        if let DayProgress::Day(t) = now {
            // blend
            let sun_color = if t < 0.5 {
                // before noon
                END_OF_DAY_COLOR * (1.0 - t) + NOON_COLOR * t
            } else {
                // after noon
                NOON_COLOR * (1.0 - t) + END_OF_DAY_COLOR * t
            };

            RGB::new(sun_color.r as u8, sun_color.g as u8, sun_color.b as u8)
        } else {
            RGB::new(0, 80, 255) // Moon color
        }
    }
}

/// Sun's apparent equatorial coordinates, Meeus ch. 25. d = days since J2000 (TT);
/// return right asc and declination
/// ported from SunCalc: https://github.com/mourner/suncalc
pub(crate) fn sun_coord(d: f64) -> (f64, f64) {
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

/// 24 periodic terms, table 27.c,
/// (A, B, C) in degrees
const EQX_SOL_PERIODIC_TERMS: [(f64, f64, f64); 24] = [
    (485.00, 324.96, 1934.14),
    (203.00, 337.23, 32964.47),
    (199.00, 342.08, 20.19),
    (182.00, 27.85, 445267.11),
    (156.00, 73.14, 45036.89),
    (136.00, 171.52, 22518.44),
    (77.00, 222.54, 65928.93),
    (74.00, 296.72, 3034.91),
    (70.00, 243.58, 9037.51),
    (58.00, 119.81, 33718.15),
    (52.00, 297.17, 150.68),
    (50.00, 21.02, 2281.23),
    (45.00, 247.54, 29929.56),
    (44.00, 325.15, 31555.96),
    (29.00, 60.93, 4443.42),
    (18.00, 155.12, 67555.33),
    (17.00, 288.79, 4562.45),
    (16.00, 198.04, 62894.03),
    (14.00, 199.76, 31436.92),
    (12.00, 95.39, 14577.85),
    (12.00, 287.11, 31931.76),
    (12.00, 320.81, 34777.26),
    (9.00, 227.73, 1222.11),
    (8.00, 15.45, 16859.07),
];

/// astronomical season transiting pattern in north hemisphere
pub const NORTH_HEMISPHERE_ASTRON_SEASON_TRANSIT: [AstronomicalSeason; 4] = [
    AstronomicalSeason::Spring,
    AstronomicalSeason::Summer,
    AstronomicalSeason::Autumn,
    AstronomicalSeason::Winter,
];

/// astronomical season transiting pattern in south hemisphere
pub const SOUTH_HEMISPHERE_ASTRON_SEASON_TRANSIT: [AstronomicalSeason; 4] = [
    AstronomicalSeason::Autumn,
    AstronomicalSeason::Winter,
    AstronomicalSeason::Spring,
    AstronomicalSeason::Summer,
];

#[derive(Clone, Copy)]
pub enum AstronomicalSeason {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl AstronomicalSeason {
    /// return JD of the given astronomical season of the year
    pub fn equinox_solstice_jd(&self, year: f64) -> f64 {
        let y = (floor(year) - 2000.0) / 1000.0;
        let y2 = y * y;
        let y3 = y2 * y;
        let y4 = y3 * y;
        // table 27.b, validate between 1000 to 3000 year
        let jde0 = match self {
            AstronomicalSeason::Spring => {
                2451623.80984 + 365242.37404 * y + 0.05169 * y2 - 0.00411 * y3 - 0.00057 * y4
            }
            AstronomicalSeason::Summer => {
                2451716.56767 + 365241.62603 * y + 0.00325 * y2 + 0.00888 * y3 - 0.00030 * y4
            }
            AstronomicalSeason::Autumn => {
                2451810.21715 + 365242.01767 * y - 0.11575 * y2 + 0.00337 * y3 + 0.00078 * y4
            }
            AstronomicalSeason::Winter => {
                2451900.05952 + 365242.74049 * y - 0.06223 * y2 - 0.00823 * y3 + 0.00032 * y4
            }
        };

        let t = (jde0 - J2000) / DAYS_PER_JULIAN_CENTURY;
        let w_rad = (35999.373 * t - 2.47).to_radians();
        let delta_lambda = 1.0 + 0.0334 * cos(w_rad) + 0.0007 * cos(2.0 * w_rad);

        let s = EQX_SOL_PERIODIC_TERMS.iter().fold(0.0, |s, term| {
            s + term.0 * cos((term.1 + term.2 * t).to_radians())
        });

        let jde = jde0 + ((0.00001 * s) / delta_lambda);

        jde - delta_t_2000(year)
    }
}
