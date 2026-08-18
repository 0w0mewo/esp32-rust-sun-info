use core::f64::consts::TAU;
use fasttime::{DateTime, OffsetDateTime, Time};
use libm::{acos, asin, atan2, cos, floor, fmod, round, sin, sincos, tan};

use crate::{
    AstronDatetimeExt, DateExt, HorizontalCoordinate, MIDNIGHT, SECONDS_PER_DAY, altitude,
    delta_t_2000,
    solar::{PlanetUpdater, SolarObject, get_pos},
};

const LUNAR_ORBIT_PERIOD_AVG: f64 = 29.530588861;

#[derive(Default, Clone, Copy)]
pub enum Phase {
    #[default]
    New,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    Full,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl Phase {
    fn from_age(age: f64) -> Self {
        match age {
            0.92..6.46 => Self::WaxingCrescent,
            6.46..8.31 => Self::FirstQuarter,
            8.31..13.84 => Self::WaxingGibbous,
            13.84..15.69 => Self::Full,
            15.69..21.22 => Self::WaningGibbous,
            21.22..23.07 => Self::LastQuarter,
            23.07..28.61 => Self::WaningCrescent,
            _ => Self::New,
        }
    }
}

impl core::fmt::Display for Phase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            Self::New => "New",
            Self::WaxingCrescent => "WXC",
            Self::FirstQuarter => "FQ",
            Self::WaxingGibbous => "WXG",
            Self::Full => "Full",
            Self::WaningGibbous => "WNG",
            Self::LastQuarter => "LQ",
            Self::WaningCrescent => "WNC",
        };

        write!(f, "{}", s)
    }
}

#[derive(Clone, Default)]
pub struct Moon {
    /// moon phase
    phase: Phase,
    /// illumination in percentage
    illumination: f64,
    /// local JD of upcoming new moon
    new_moon: f64,
    /// local JD of upcoming full moon
    full_moon: f64,
    /// moonrise in local time, seconds since midnight
    moonrise: u32,
    /// moonset in localtime, seconds since midnight
    moonset: u32,
    pos: HorizontalCoordinate,
}

/// get decimal year by today with `offset` days
#[inline]
fn decimal_year(now: &DateTime, offset: f64) -> f64 {
    (now.date.ordinal() as f64 + offset) / now.days_per_year() as f64 + now.date.year as f64
}

impl PlanetUpdater for Moon {
    fn update_pos(&mut self, now: &OffsetDateTime, lat: f64, lon: f64) {
        self.pos = get_pos(&now.utc, lat, lon, SolarObject::Moon);
    }

    fn update_astron(&mut self, now: &OffsetDateTime, lat: f64, lon: f64) {
        let now_utc = &now.utc;
        let jd_now_utc = now_utc.to_julian();
        let tz_offset_sec = now.offset.as_seconds() as f64;
        let tz_offset_days = tz_offset_sec / SECONDS_PER_DAY;

        // upcoming moon events
        let next_new_moon_jd = upcoming_moon_phase_jd(now_utc, Phase::New); // in UTC
        self.new_moon = next_new_moon_jd + tz_offset_days;
        self.full_moon = upcoming_moon_phase_jd(now_utc, Phase::Full) + tz_offset_days;

        // find the Julian days of last new moon,
        // push back one lunar period and re-calculate it if the day is in the future.
        let mut jd_last_new_moon = moon_phase_jd(now_utc.decimal_year(), Phase::New);
        if jd_last_new_moon > jd_now_utc {
            jd_last_new_moon =
                moon_phase_jd(decimal_year(now_utc, -LUNAR_ORBIT_PERIOD_AVG), Phase::New);
        }

        // moonrise and moonset
        let (rise_utc_secs, set_utc_secs) = moon_rise_set(now_utc, lat, lon).unwrap_or_default();
        let (rise_local_secs, set_local_secs) =
            (rise_utc_secs + tz_offset_sec, set_utc_secs + tz_offset_sec);
        self.moonrise = round(rise_local_secs % SECONDS_PER_DAY) as u32;
        self.moonset = round(set_local_secs % SECONDS_PER_DAY) as u32;

        // other stuffs
        let (age, illumination) =
            Self::approx_phase(jd_now_utc, jd_last_new_moon, next_new_moon_jd);
        self.illumination = illumination * 100.0;
        self.phase = Phase::from_age(age);
    }
}

impl Moon {
    #[inline]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    #[inline]
    pub fn illumination(&self) -> f64 {
        self.illumination
    }

    #[inline]
    /// upcoming new moon in local time
    pub fn upcoming_new_moon(&self) -> DateTime {
        DateTime::from_julian(self.new_moon)
    }

    #[inline]
    /// upcoming full moon in local time
    pub fn upcoming_full_moon(&self) -> DateTime {
        DateTime::from_julian(self.full_moon)
    }

    #[inline]
    /// moon current azimuth and altitude are in degrees
    pub fn pos(&self) -> HorizontalCoordinate {
        self.pos
    }

    #[inline]
    /// moon rise in local time
    pub fn rise_at(&self) -> Time {
        Time::from_seconds_nanos(self.moonrise, 0).unwrap_or(MIDNIGHT)
    }

    #[inline]
    /// moon set in local time
    pub fn set_at(&self) -> Time {
        Time::from_seconds_nanos(self.moonset, 0).unwrap_or(MIDNIGHT)
    }

    // lunar age and illumination
    // TODO: implements with Meeus ch.48
    fn approx_phase(jd: f64, last_new_moon_jd: f64, next_new_moon_jd: f64) -> (f64, f64) {
        let lunar_orbit_period = (next_new_moon_jd - last_new_moon_jd).abs();
        let age = fmod(jd - last_new_moon_jd, lunar_orbit_period);
        let illumination = 0.5 * (1.0 - cos(age * TAU / lunar_orbit_period));

        (age, illumination)
    }
}

const NEW_MOON_COFFS: [f64; 25] = [
    -0.40720, 0.17241, 0.01608, 0.01039, 0.00739, -0.00514, 0.00208, -0.00111, -0.00057, 0.00056,
    -0.00042, 0.00042, 0.00038, -0.00024, -0.00017, -0.00007, 0.00004, 0.00004, 0.00003, 0.00003,
    -0.00003, 0.00003, -0.00002, -0.00002, 0.00002,
];

const FULL_MOON_COFFS: [f64; 25] = [
    -0.40614, 0.17302, 0.01614, 0.01043, 0.00734, -0.00515, 0.00209, -0.00111, -0.00057, 0.00056,
    -0.00042, 0.00042, 0.00038, -0.00024, -0.00017, -0.00007, 0.00004, 0.00004, 0.00003, 0.00003,
    -0.00003, 0.00003, -0.00002, -0.00002, 0.00002,
];

const QUARTER_COFFS: [f64; 25] = [
    -0.62801, 0.17172, -0.01183, 0.00862, 0.00804, 0.00454, 0.00204, -0.00180, -0.00070, -0.00040,
    -0.00034, 0.00032, 0.00032, -0.00028, 0.00027, -0.00017, -0.00005, 0.00004, -0.00004, 0.00004,
    0.00003, 0.00003, 0.00002, 0.00002, -0.00002,
];

/// MEEUS ch.49 - calculate the new/full moon from given decimal year in the current lunation
fn moon_phase_jd(decimal_year: f64, phase: Phase) -> f64 {
    // ch. 49.2
    let k_offset = match phase {
        Phase::New => 0.0,
        Phase::FirstQuarter => 0.25,
        Phase::Full => 0.50,
        Phase::LastQuarter => 0.75,
        _ => unreachable!(),
    };
    let k = floor((decimal_year - 2000.0) * 12.3685) + k_offset;

    // ch. 49.3, Julian centuries since epoch 2000
    let t = k / 1236.85;
    let tt = t * t; // t^2
    let ttt = tt * t; // t^3
    let tttt = ttt * t; // t^4

    // ch. 49.1
    let jde = 2451550.09766 + 29.530588861 * k + 0.00015437 * tt - 0.000000150 * ttt
        + 0.00000000073 * tttt;

    // ch. 49.4, Sun's mean anomaly
    let sun_m = 2.5534 + 29.10535670 * k - 0.0000014 * tt - 0.00000011 * ttt;

    // ch. 49.5, Moon's mean anomaly
    let moon_m =
        201.5643 + 385.81693528 * k + 0.0107582 * tt + 0.00001238 * ttt - 0.000000058 * tttt;

    // ch. 49.6
    let f = 160.7108 + 390.67050284 * k - 0.0016118 * tt - 0.00000227 * ttt + 0.000000011 * tttt;

    // ch. 49.7
    let omega = 124.7746 - 1.56375588 * k + 0.0020672 * tt + 0.00000215 * ttt;

    let m_rad = sun_m.to_radians();
    let m_prime_rad = moon_m.to_radians();
    let f_rad = f.to_radians();
    let omega_rad = omega.to_radians();

    // planetary arguments in radians
    let a1 = (299.77 + 0.107408 * k - 0.009173 * tt).to_radians();
    let a2 = (251.88 + 0.016321 * k).to_radians();
    let a3 = (251.83 + 26.651886 * k).to_radians();
    let a4 = (349.42 + 36.412478 * k).to_radians();
    let a5 = (84.66 + 18.206239 * k).to_radians();
    let a6 = (141.74 + 53.303771 * k).to_radians();
    let a7 = (207.14 + 2.453732 * k).to_radians();
    let a8 = (154.84 + 7.306860 * k).to_radians();
    let a9 = (34.52 + 27.261239 * k).to_radians();
    let a10 = (207.19 + 0.121824 * k).to_radians();
    let a11 = (291.34 + 1.844379 * k).to_radians();
    let a12 = (161.72 + 24.198154 * k).to_radians();
    let a13 = (239.56 + 25.513099 * k).to_radians();
    let a14 = (331.55 + 3.592518 * k).to_radians();

    // E
    let e = 1.0 - 0.002516 * t - 0.0000074 * tt;
    let ee = e * e; // e^2

    // correction terms for new moon
    let coff = match phase {
        Phase::Full => &FULL_MOON_COFFS,
        Phase::New => &NEW_MOON_COFFS,
        Phase::FirstQuarter | Phase::LastQuarter => &QUARTER_COFFS,
        _ => unreachable!(),
    };
    let c = match phase {
        Phase::Full | Phase::New => {
            coff[0] * sin(m_prime_rad)
                + coff[1] * e * sin(m_rad)
                + coff[2] * sin(2.0 * m_prime_rad)
                + coff[3] * sin(2.0 * f_rad)
                + coff[4] * e * sin(m_prime_rad - m_rad)
                + coff[5] * e * sin(m_prime_rad + m_rad)
                + coff[6] * ee * sin(2.0 * m_rad)
                + coff[7] * sin(m_prime_rad - 2.0 * f_rad)
                + coff[8] * sin(m_prime_rad + 2.0 * f_rad)
                + coff[9] * e * sin(2.0 * m_prime_rad + m_rad)
                + coff[10] * sin(3.0 * m_prime_rad)
                + coff[11] * e * sin(m_rad + 2.0 * f_rad)
                + coff[12] * e * sin(m_rad - 2.0 * f_rad)
                + coff[13] * e * sin(2.0 * m_prime_rad - m_rad)
                + coff[14] * sin(omega_rad)
                + coff[15] * sin(m_prime_rad + 2.0 * m_rad)
                + coff[16] * sin(2.0 * m_prime_rad - 2.0 * f_rad)
                + coff[17] * sin(3.0 * m_rad)
                + coff[18] * sin(m_prime_rad + m_rad - 2.0 * f_rad)
                + coff[19] * sin(2.0 * m_prime_rad + 2.0 * f_rad)
                + coff[20] * sin(m_prime_rad + m_rad + 2.0 * f_rad)
                + coff[21] * sin(m_prime_rad - m_rad + 2.0 * f_rad)
                + coff[22] * sin(m_prime_rad - m_rad - 2.0 * f_rad)
                + coff[23] * sin(3.0 * m_prime_rad + m_rad)
                + coff[24] * sin(4.0 * m_prime_rad)
        }

        Phase::FirstQuarter | Phase::LastQuarter => {
            coff[0] * sin(m_prime_rad)
                + coff[1] * e * sin(m_rad)
                + coff[2] * e * sin(m_rad + m_prime_rad)
                + coff[3] * sin(2.0 * m_prime_rad)
                + coff[4] * sin(2.0 * f_rad)
                + coff[5] * e * sin(m_prime_rad - m_rad)
                + coff[6] * ee * sin(2.0 * m_rad)
                + coff[7] * sin(m_prime_rad - 2.0 * f_rad)
                + coff[8] * sin(m_prime_rad + 2.0 * f_rad)
                + coff[9] * sin(3.0 * m_prime_rad)
                + coff[10] * e * sin(2.0 * m_prime_rad - m_rad)
                + coff[11] * e * sin(m_rad + 2.0 * f_rad)
                + coff[12] * e * sin(m_rad - 2.0 * f_rad)
                + coff[13] * ee * sin(m_prime_rad + 2.0 * m_rad)
                + coff[14] * e * sin(2.0 * m_prime_rad + m_rad)
                + coff[15] * sin(omega_rad)
                + coff[16] * sin(m_prime_rad - m_rad - 2.0 * f_rad)
                + coff[17] * sin(2.0 * m_prime_rad + 2.0 * f_rad)
                + coff[18] * sin(m_prime_rad + m_rad + 2.0 * f_rad)
                + coff[19] * sin(m_prime_rad - 2.0 * m_rad)
                + coff[20] * sin(m_prime_rad + m_rad - 2.0 * f_rad)
                + coff[21] * sin(3.0 * m_rad)
                + coff[22] * sin(2.0 * m_prime_rad - 2.0 * f_rad)
                + coff[23] * sin(m_prime_rad - m_rad + 2.0 * f_rad)
                + coff[24] * sin(3.0 * m_prime_rad + m_rad)
        }

        _ => unreachable!(),
    };

    // additional correction terms for quarter phase only
    let w = 0.00306 - 0.00038 * e * cos(m_rad) + 0.00026 * cos(m_prime_rad)
        - 0.00002 * cos(m_prime_rad - m_rad)
        + 0.00002 * cos(m_prime_rad + m_rad)
        + 0.00002 * cos(2.0 * f_rad);
    let w = match phase {
        Phase::LastQuarter => -w,
        Phase::FirstQuarter => 1.0 * w,
        Phase::New | Phase::Full => 0.0,
        _ => unreachable!(),
    };

    // additional correction terms for all phases
    let c_additional = 0.000325 * sin(a1)
        + 0.000165 * sin(a2)
        + 0.000164 * sin(a3)
        + 0.000126 * sin(a4)
        + 0.000110 * sin(a5)
        + 0.000062 * sin(a6)
        + 0.000060 * sin(a7)
        + 0.000056 * sin(a8)
        + 0.000047 * sin(a9)
        + 0.000042 * sin(a10)
        + 0.000040 * sin(a11)
        + 0.000037 * sin(a12)
        + 0.000035 * sin(a13)
        + 0.000023 * sin(a14);

    jde + c + w + c_additional - delta_t_2000(decimal_year)
}

fn upcoming_moon_phase_jd(now: &DateTime, phase: Phase) -> f64 {
    let jd_now_utc = now.to_julian();
    let mut jd_phase_utc = moon_phase_jd(decimal_year(now, 0.0), phase);
    if jd_now_utc > jd_phase_utc {
        jd_phase_utc = moon_phase_jd(decimal_year(now, LUNAR_ORBIT_PERIOD_AVG), phase);
    }

    jd_phase_utc
}

/// Meeus table 47.A
/// Periodic terms for the Moon's longitude (Σl, ×1e-6 deg) and distance (Σr, ×1e-3 km).
/// `(D, M, M', F, sum_l, sum_r)`
const MOON_LON_LUT: [(f64, f64, f64, f64, f64, f64); 60] = [
    (0.00, 0.00, 1.00, 0.00, 6288774.00, -20905355.00),
    (2.00, 0.00, -1.00, 0.00, 1274027.00, -3699111.00),
    (2.00, 0.00, 0.00, 0.00, 658314.00, -2955968.00),
    (0.00, 0.00, 2.00, 0.00, 213618.00, -569925.00),
    (0.00, 1.00, 0.00, 0.00, -185116.00, 48888.00),
    (0.00, 0.00, 0.00, 2.00, -114332.00, -3149.00),
    (2.00, 0.00, -2.00, 0.00, 58793.00, 246158.00),
    (2.00, -1.00, -1.00, 0.00, 57066.00, -152138.00),
    (2.00, 0.00, 1.00, 0.00, 53322.00, -170733.00),
    (2.00, -1.00, 0.00, 0.00, 45758.00, -204586.00),
    (0.00, 1.00, -1.00, 0.00, -40923.00, -129620.00),
    (1.00, 0.00, 0.00, 0.00, -34720.00, 108743.00),
    (0.00, 1.00, 1.00, 0.00, -30383.00, 104755.00),
    (2.00, 0.00, 0.00, -2.00, 15327.00, 10321.00),
    (0.00, 0.00, 1.00, 2.00, -12528.00, 0.00),
    (0.00, 0.00, 1.00, -2.00, 10980.00, 79661.00),
    (4.00, 0.00, -1.00, 0.00, 10675.00, -34782.00),
    (0.00, 0.00, 3.00, 0.00, 10034.00, -23210.00),
    (4.00, 0.00, -2.00, 0.00, 8548.00, -21636.00),
    (2.00, 1.00, -1.00, 0.00, -7888.00, 24208.00),
    (2.00, 1.00, 0.00, 0.00, -6766.00, 30824.00),
    (1.00, 0.00, -1.00, 0.00, -5163.00, -8379.00),
    (1.00, 1.00, 0.00, 0.00, 4987.00, -16675.00),
    (2.00, -1.00, 1.00, 0.00, 4036.00, -12831.00),
    (2.00, 0.00, 2.00, 0.00, 3994.00, -10445.00),
    (4.00, 0.00, 0.00, 0.00, 3861.00, -11650.00),
    (2.00, 0.00, -3.00, 0.00, 3665.00, 14403.00),
    (0.00, 1.00, -2.00, 0.00, -2689.00, -7003.00),
    (2.00, 0.00, -1.00, 2.00, -2602.00, 0.00),
    (2.00, -1.00, -2.00, 0.00, 2390.00, 10056.00),
    (1.00, 0.00, 1.00, 0.00, -2348.00, 6322.00),
    (2.00, -2.00, 0.00, 0.00, 2236.00, -9884.00),
    (0.00, 1.00, 2.00, 0.00, -2120.00, 5751.00),
    (0.00, 2.00, 0.00, 0.00, -2069.00, 0.00),
    (2.00, -2.00, -1.00, 0.00, 2048.00, -4950.00),
    (2.00, 0.00, 1.00, -2.00, -1773.00, 4130.00),
    (2.00, 0.00, 0.00, 2.00, -1595.00, 0.00),
    (4.00, -1.00, -1.00, 0.00, 1215.00, -3958.00),
    (0.00, 0.00, 2.00, 2.00, -1110.00, 0.00),
    (3.00, 0.00, -1.00, 0.00, -892.00, 3258.00),
    (2.00, 1.00, 1.00, 0.00, -810.00, 2616.00),
    (4.00, -1.00, -2.00, 0.00, 759.00, -1897.00),
    (0.00, 2.00, -1.00, 0.00, -713.00, -2117.00),
    (2.00, 2.00, -1.00, 0.00, -700.00, 2354.00),
    (2.00, 1.00, -2.00, 0.00, 691.00, 0.00),
    (2.00, -1.00, 0.00, -2.00, 596.00, 0.00),
    (4.00, 0.00, 1.00, 0.00, 549.00, -1423.00),
    (0.00, 0.00, 4.00, 0.00, 537.00, -1117.00),
    (4.00, -1.00, 0.00, 0.00, 520.00, -1571.00),
    (1.00, 0.00, -2.00, 0.00, -487.00, -1739.00),
    (2.00, 1.00, 0.00, -2.00, -399.00, 0.00),
    (0.00, 0.00, 2.00, -2.00, -381.00, -4421.00),
    (1.00, 1.00, 1.00, 0.00, 351.00, 0.00),
    (3.00, 0.00, -2.00, 0.00, -340.00, 0.00),
    (4.00, 0.00, -3.00, 0.00, 330.00, 0.00),
    (2.00, -1.00, 2.00, 0.00, 327.00, 0.00),
    (0.00, 2.00, 1.00, 0.00, -323.00, 1165.00),
    (1.00, 1.00, -1.00, 0.00, 299.00, 0.00),
    (2.00, 0.00, 3.00, 0.00, 294.00, 0.00),
    (2.00, 0.00, -1.00, -2.00, 0.00, 8752.00),
];

/// Meeus table 47.B
/// Periodic terms for the Moon's latitude (Σb, ×1e-6 deg).
/// `(D, M, M', F, sum_b)`
const MOON_LAT_LUT: [(f64, f64, f64, f64, f64); 60] = [
    (0.00, 0.00, 0.00, 1.00, 5128122.00),
    (0.00, 0.00, 1.00, 1.00, 280602.00),
    (0.00, 0.00, 1.00, -1.00, 277693.00),
    (2.00, 0.00, 0.00, -1.00, 173237.00),
    (2.00, 0.00, -1.00, 1.00, 55413.00),
    (2.00, 0.00, -1.00, -1.00, 46271.00),
    (2.00, 0.00, 0.00, 1.00, 32573.00),
    (0.00, 0.00, 2.00, 1.00, 17198.00),
    (2.00, 0.00, 1.00, -1.00, 9266.00),
    (0.00, 0.00, 2.00, -1.00, 8822.00),
    (2.00, -1.00, 0.00, -1.00, 8216.00),
    (2.00, 0.00, -2.00, -1.00, 4324.00),
    (2.00, 0.00, 1.00, 1.00, 4200.00),
    (2.00, 1.00, 0.00, -1.00, -3359.00),
    (2.00, -1.00, -1.00, 1.00, 2463.00),
    (2.00, -1.00, 0.00, 1.00, 2211.00),
    (2.00, -1.00, -1.00, -1.00, 2065.00),
    (0.00, 1.00, -1.00, -1.00, -1870.00),
    (4.00, 0.00, -1.00, -1.00, 1828.00),
    (0.00, 1.00, 0.00, 1.00, -1794.00),
    (0.00, 0.00, 0.00, 3.00, -1749.00),
    (0.00, 1.00, -1.00, 1.00, -1565.00),
    (1.00, 0.00, 0.00, 1.00, -1491.00),
    (0.00, 1.00, 1.00, 1.00, -1475.00),
    (0.00, 1.00, 1.00, -1.00, -1410.00),
    (0.00, 1.00, 0.00, -1.00, -1344.00),
    (1.00, 0.00, 0.00, -1.00, -1335.00),
    (0.00, 0.00, 3.00, 1.00, 1107.00),
    (4.00, 0.00, 0.00, -1.00, 1021.00),
    (4.00, 0.00, -1.00, 1.00, 833.00),
    (0.00, 0.00, 1.00, -3.00, 777.00),
    (4.00, 0.00, -2.00, 1.00, 671.00),
    (2.00, 0.00, 0.00, -3.00, 607.00),
    (2.00, 0.00, 2.00, -1.00, 596.00),
    (2.00, -1.00, 1.00, -1.00, 491.00),
    (2.00, 0.00, -2.00, 1.00, -451.00),
    (0.00, 0.00, 3.00, -1.00, 439.00),
    (2.00, 0.00, 2.00, 1.00, 422.00),
    (2.00, 0.00, -3.00, -1.00, 421.00),
    (2.00, 1.00, -1.00, 1.00, -366.00),
    (2.00, 1.00, 0.00, 1.00, -351.00),
    (4.00, 0.00, 0.00, 1.00, 331.00),
    (2.00, -1.00, 1.00, 1.00, 315.00),
    (2.00, -2.00, 0.00, -1.00, 302.00),
    (0.00, 0.00, 1.00, 3.00, -283.00),
    (2.00, 1.00, 1.00, -1.00, -229.00),
    (1.00, 1.00, 0.00, -1.00, 223.00),
    (1.00, 1.00, 0.00, 1.00, 223.00),
    (0.00, 1.00, -2.00, -1.00, -220.00),
    (2.00, 1.00, -1.00, -1.00, -220.00),
    (1.00, 0.00, 1.00, 1.00, -185.00),
    (2.00, -1.00, -2.00, -1.00, 181.00),
    (0.00, 1.00, 2.00, 1.00, -177.00),
    (4.00, 0.00, -2.00, -1.00, 176.00),
    (4.00, -1.00, -1.00, -1.00, 166.00),
    (1.00, 0.00, 1.00, -1.00, -164.00),
    (4.00, 0.00, 1.00, -1.00, 132.00),
    (1.00, 0.00, -1.00, -1.00, -119.00),
    (4.00, -1.00, 0.00, -1.00, 115.00),
    (2.00, -2.00, 0.00, 1.00, 107.00),
];

/// geocentric apparent equatorial coordinates of the Moon, Meeus ch. 47. jde = days since J2000 (TT).
/// return right asc and declination in radians, distance in km
/// ported from SunCalc: https://github.com/mourner/suncalc
pub(crate) fn moon_coord(jde: f64) -> (f64, f64, f64) {
    let t = jde / 36525.0;
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;

    // fundamental arguments (degrees), 47.1–47.6
    let l_prime =
        218.3164477 + 481267.88123421 * t - 0.0015786 * t2 + t3 / 538841.0 - t4 / 65194000.0;
    let d = 297.8501921 + 445267.1114034 * t - 0.0018819 * t2 + t3 / 545868.0 - t4 / 113065000.0;
    let m = 357.5291092 + 35999.0502909 * t - 0.0001536 * t2 + t3 / 24490000.0;
    let m_prime =
        134.9633964 + 477198.8675055 * t + 0.0087414 * t2 + t3 / 69699.0 - t4 / 14712000.0;
    let f = 93.2720950 + 483202.0175233 * t - 0.0036539 * t2 - t3 / 3526000.0 + t4 / 863310000.0;
    let a1 = 119.75 + 131.849 * t;
    let a2 = 53.09 + 479264.290 * t;
    let a3 = 313.45 + 481266.484 * t;
    let e = 1.0 - t * 0.002516 + t2 * 0.0000074; // eccentricity factor for solar-anomaly terms

    let l_prime_rad = l_prime.to_radians();
    let d_rad = d.to_radians();
    let m_rad = m.to_radians();
    let m_prime_rad = m_prime.to_radians();
    let f_rad = f.to_radians();
    let a1_rad = a1.to_radians();
    let a2_rad = a2.to_radians();
    let a3_rad = a3.to_radians();

    let (sum_l, sum_r) = MOON_LON_LUT
        .iter()
        .map(|param| {
            let arg = param.0 * d_rad + param.1 * m_rad + param.2 * m_prime_rad + param.3 * f_rad;
            let extra = match param.1 {
                // the coefficient of M
                -1.0 | 1.0 => e,
                -2.0 | 2.0 => e * e,
                _ => 1.0,
            };
            let sl = param.4 * extra * sin(arg);
            let sr = param.5 * extra * cos(arg); // we will not calculate distance here
            (sl, sr)
        })
        .reduce(|acc, s| (acc.0 + s.0, acc.1 + s.1))
        .unwrap_or((0.0, 0.0));

    let sum_b = MOON_LAT_LUT
        .iter()
        .map(|param| {
            let arg = param.0 * d_rad + param.1 * m_rad + param.2 * m_prime_rad + param.3 * f_rad;
            let extra = match param.1 {
                // the coefficient of M
                -1.0 | 1.0 => e,
                -2.0 | 2.0 => e * e,
                _ => 1.0,
            };

            param.4 * extra * sin(arg)
        })
        .sum::<f64>();

    let sum_l =
        sum_l + 3958.0 * sin(a1_rad) + 1962.0 * sin(l_prime_rad - f_rad) + 318.0 * sin(a2_rad);
    let sum_b = sum_b - 2235.0 * sin(l_prime_rad)
        + 382.0 * sin(a3_rad)
        + 175.0 * sin(a1_rad - f_rad)
        + 175.0 * sin(a1_rad + f_rad)
        + 127.0 * sin(l_prime_rad - m_prime_rad)
        - 115.0 * sin(l_prime_rad + m_prime_rad);

    // Ch. 22
    let (dpsi, eps) = nutation_obliquity(t);
    let eps_rad = eps.to_radians();

    // convert to radians for trig functions
    let l_rad = (l_prime + sum_l / 1e6 + dpsi).to_radians();
    let b_rad = (sum_b / 1e6).to_radians();

    // convert equatorial coordinates and distance in km
    let ra = atan2(
        sin(l_rad) * cos(eps_rad) - tan(b_rad) * sin(eps_rad),
        cos(l_rad),
    );
    let dec = asin(sin(b_rad) * cos(eps_rad) + cos(b_rad) * sin(eps_rad) * sin(l_rad));
    let dist = 385000.56 + sum_r / 1000.0;

    (ra, dec, dist)
}

///  Nutation in longitude (Δψ) and true obliquity of the ecliptic, in degress
fn nutation_obliquity(t: f64) -> (f64, f64) {
    let om = (125.04452 - 1934.136261 * t).to_radians(); // longitude of the Moon's ascending node
    let ls = (280.4665 + 36000.7698 * t).to_radians(); // mean longitude of the Sun
    let lm = (218.3165 + 481267.8813 * t).to_radians(); // mean longitude of the Moon
    let dpsi = (-17.20 * sin(om) - 1.32 * sin(2.0 * ls) - 0.23 * sin(2.0 * lm)
        + 0.21 * sin(2.0 * om))
        / 3600.0;
    let deps = (9.20 * cos(om) + 0.57 * cos(2.0 * ls) + 0.10 * cos(2.0 * lm)
        - 0.09 * cos(2.0 * om))
        / 3600.0;
    let eps0 = 23.439291 - t * (0.0130042 + t * (0.00000016 - t * 0.000000504)); // 22.2 mean obliquity
    let eps = eps0 + deps;

    (dpsi, eps)
}

/// return None if there is no rise/set
/// otherwise return rise and set time in UTC seconds since midnight
fn moon_rise_set(now_utc: &DateTime, lat: f64, lon: f64) -> Option<(f64, f64)> {
    let h0_rad = (0.125_f64).to_radians();
    let lat_rad = lat.to_radians();
    // Note: the longitude in the original formula is east negative while west negative is used here.
    let lon_rad = (-lon).to_radians();

    // GMST at 0h
    let gmst0_rad = now_utc.date.to_sidereal_time(0.0).to_radians();

    // apparent equatorial coordinates of moon at UTC 00:00 yesterday, today, tomorrow
    // 0 = yesteray, 1 = today, 2 = tomorrow
    let mut jd = [0.0_f64; 3];
    let mut ra_rad = [0.0_f64; 3];
    let mut dec_rad = [0.0_f64; 3];
    let dt = delta_t_2000(now_utc.date.decimal_year());
    for (idx, day_offset) in (-1..=1).enumerate() {
        jd[idx] = now_utc.date.to_julian_epoch_2000() + day_offset as f64;
        (ra_rad[idx], dec_rad[idx], _) = moon_coord(jd[idx]);
    }

    // Meeus, ch 15.1
    let (dec_today_sin, dec_today_cos) = sincos(dec_rad[1]);
    let cos_h = (sin(h0_rad) - sin(lat_rad) * dec_today_sin) / (cos(lat_rad) * dec_today_cos);
    if !(-1.0..=1.0).contains(&cos_h) {
        return None;
    }
    let h_rad = wrap_2pi(acos(cos_h));

    // Meeus, ch 15.2
    let m_transit = wrap1((ra_rad[1] + lon_rad - gmst0_rad) / TAU);
    let m_rise = wrap1(m_transit - h_rad / TAU);
    let m_set = wrap1(m_transit + h_rad / TAU);

    const C: f64 = 360.985647_f64.to_radians();
    let (gst_rad_rise, gst_rad_set) = (gmst0_rad + C * m_rise, gmst0_rad + C * m_set);

    // interpol RA at rise and set
    let (ra_rad_rise, ra_rad_set) = (
        interp3(ra_rad[0], ra_rad[1], ra_rad[2], m_rise + dt),
        interp3(ra_rad[0], ra_rad[1], ra_rad[2], m_set + dt),
    );

    // interpol DEC at rise and set
    let (dec_rad_rise, dec_rad_set) = (
        interp3(dec_rad[0], dec_rad[1], dec_rad[2], m_rise + dt),
        interp3(dec_rad[0], dec_rad[1], dec_rad[2], m_set + dt),
    );

    // hour angle of rise and set from interpolated DEC
    let (ha_rad_rise, ha_rad_set) = (
        gst_rad_rise - ra_rad_rise - lon_rad,
        gst_rad_set - ra_rad_set - lon_rad,
    );
    let (ha_rad_rise, ha_rad_set) = (wrap_pi(ha_rad_rise), wrap_pi(ha_rad_set));

    // correction factor for m_rise and m_set
    let dm_rise = (altitude(dec_rad_rise, lat_rad, ha_rad_rise) - h0_rad)
        / (TAU * cos(dec_rad_rise) * cos(lat_rad) * sin(ha_rad_rise));
    let dm_set = (altitude(dec_rad_set, lat_rad, ha_rad_set) - h0_rad)
        / (TAU * cos(dec_rad_set) * cos(lat_rad) * sin(ha_rad_set));

    let rise = SECONDS_PER_DAY * wrap1(m_rise + dm_rise);
    let set = SECONDS_PER_DAY * wrap1(m_set + dm_set);

    Some((rise, set))
}

/// wrap a value to [0.0, 1.0]
fn wrap1(m: f64) -> f64 {
    let m = fmod(m, 1.0);
    if m < 1.0 {
        return m + 1.0;
    }

    m
}

/// wrap an angle to (-PI, PI]
fn wrap_pi(a: f64) -> f64 {
    a - TAU * round(a / TAU)
}

/// wrap an angle to [0.0, 2*PI]
fn wrap_2pi(a: f64) -> f64 {
    let a = wrap_pi(a);

    if a < 0.0 { a + TAU } else { a }
}

fn interp3(start: f64, mid: f64, end: f64, n: f64) -> f64 {
    let a = mid - start;
    let b = end - mid;
    let c = b - a;

    mid + 0.5 * n * (a + b + c * n)
}
