use core::f64::consts::TAU;
use fasttime::{DateTime, OffsetDateTime};
use libm::{cos, floor, fmod, sin};

use crate::{AstronDatetimeExt, DateExt, SECONDS_PER_DAY, delta_t_2000};

const LUNAR_ORBIT_PERIOD: f64 = 29.530588861;

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
    /// local JD of upcoming first quarter
    first_quarter: f64,
    /// local JD of upcoming last quarter
    last_quarter: f64,
}

/// get decimal year by today with `offset` days
#[inline]
fn decimal_year(now: &DateTime, offset: f64) -> f64 {
    (now.date.ordinal() as f64 + offset) / now.days_per_year() as f64 + now.date.year as f64
}

impl Moon {
    pub fn update(&mut self, now: &OffsetDateTime) {
        let now_utc = &now.utc;
        let jd_now_utc = now_utc.to_julian();
        let tz_offset_days = now.offset.as_seconds() as f64 / SECONDS_PER_DAY;

        // upcoming moon events
        self.new_moon = upcoming_moon_phase_jd(now_utc, Phase::New) + tz_offset_days;
        self.full_moon = upcoming_moon_phase_jd(now_utc, Phase::Full) + tz_offset_days;
        self.first_quarter = upcoming_moon_phase_jd(now_utc, Phase::FirstQuarter) + tz_offset_days;
        self.last_quarter = upcoming_moon_phase_jd(now_utc, Phase::LastQuarter) + tz_offset_days;

        // find the Julian days of last new moon,
        // push back one lunar period and re-calculate it if the day is in the future.
        let mut jd_last_new_moon = moon_phase_jd(now_utc.decimal_year(), Phase::New);
        if jd_last_new_moon > jd_now_utc {
            jd_last_new_moon = moon_phase_jd(decimal_year(now_utc, -LUNAR_ORBIT_PERIOD), Phase::New);
        }


        // other stuffs
        let (age, illumination) = Self::approx_phase(jd_now_utc, jd_last_new_moon);
        self.illumination = illumination * 100.0;
        self.phase = Phase::from_age(age);
    }

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
    /// upcoming first quarter in local time
    pub fn upcoming_first_quarter(&self) -> DateTime {
        DateTime::from_julian(self.first_quarter)
    }
    
    #[inline]
    /// upcoming last quarter in local time
    pub fn upcoming_last_quarter(&self) -> DateTime {
        DateTime::from_julian(self.last_quarter)
    }

    // lunar age and illumination
    fn approx_phase(jd: f64, last_new_moon_jd: f64) -> (f64, f64) {
        let age = fmod(jd - last_new_moon_jd, LUNAR_ORBIT_PERIOD);
        let illumination = 0.5 * (1.0 - cos(age * TAU / LUNAR_ORBIT_PERIOD));

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
        jd_phase_utc = moon_phase_jd(decimal_year(now, LUNAR_ORBIT_PERIOD), phase);
    }

    jd_phase_utc
}