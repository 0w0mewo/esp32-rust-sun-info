use core::{
    f64::consts::{PI, TAU},
    ops::{Add, Mul},
};

use libm::{acos, asin, atan, atan2, cos, sin, sincos, sqrt, tan};
use time::UtcDateTime;

use crate::{
    SECONDS_PER_DAY,
    config::OBSERVER_ALTITUDE,
    datetime::{AstronDatetimeExt, DAY_PER_HOUR, DAYS_PER_JULIAN_CENTURY, J2000},
    math::{QuadraticInterpolator, QuadraticRoots, wrap_pi},
    solar::{
        EventInfo, MEAN_OBLIQUITY_J2000_RAD, apparent_sidereal_time, coord::HorizontalCoordinate,
        moon::moon_coord, npb::NpbTransformer, sine_altitude,
    },
};

#[derive(Clone)]
struct OrbitialElements {
    /// semi-major axis in AU
    a: f64,
    /// eccentritiy in radians
    e: f64,
    /// inclination in degrees, in radians if `with_radians()` applied
    inc: f64,
    /// mean longitude in degrees, in radians if `with_radians()` applied
    mean_lon: f64,
    /// longitude of perihelion in degrees, in radians if `with_radians()` applied
    omega_bar: f64,
    /// longitude of ascending node in degrees, in radians if `with_radians()` applied
    omega0: f64,
}

impl OrbitialElements {
    pub fn with_radians(&self) -> Self {
        Self {
            a: self.a,
            e: self.e,
            inc: self.inc.to_radians(),
            mean_lon: self.mean_lon.to_radians(),
            omega_bar: self.omega_bar.to_radians(),
            omega0: self.omega0.to_radians(),
        }
    }
}

impl Default for OrbitialElements {
    fn default() -> Self {
        Self {
            a: f64::INFINITY,
            e: f64::INFINITY,
            inc: f64::INFINITY,
            mean_lon: f64::INFINITY,
            omega0: f64::INFINITY,
            omega_bar: f64::INFINITY,
        }
    }
}

impl Add for OrbitialElements {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            a: self.a + rhs.a,
            e: self.e + rhs.e,
            inc: self.inc + rhs.inc,
            mean_lon: self.mean_lon + rhs.mean_lon,
            omega_bar: self.omega_bar + rhs.omega_bar,
            omega0: self.omega0 + rhs.omega0,
        }
    }
}

impl Mul<f64> for OrbitialElements {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            a: self.a * rhs,
            e: self.e * rhs,
            inc: self.inc * rhs,
            mean_lon: self.mean_lon * rhs,
            omega_bar: self.omega_bar * rhs,
            omega0: self.omega0 * rhs,
        }
    }
}

#[derive(Clone, Copy)]
pub enum SolarObject {
    Sun,
    Moon,
    Mercury,
    Venus,
    Earth,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
}

/// https://ssd.jpl.nasa.gov/planets/approx_pos.html
impl SolarObject {
    /// keplerian rates in degrees/century
    const fn keplerian_rate(&self) -> OrbitialElements {
        // table 1
        match self {
            Self::Sun => OrbitialElements {
                a: 0.0,
                e: 0.0,
                inc: 0.0,
                mean_lon: 0.0,
                omega_bar: 0.0,
                omega0: 0.0,
            },
            Self::Mercury => OrbitialElements {
                a: 3.7e-7,
                e: 0.00001906,
                inc: -0.00594749,
                mean_lon: 149472.67411175,
                omega_bar: 0.16047689,
                omega0: -0.12534081,
            },
            Self::Venus => OrbitialElements {
                a: 0.0000039,
                e: -0.00004107,
                inc: -0.0007889,
                mean_lon: 58517.81538729,
                omega_bar: 0.00268329,
                omega0: -0.27769418,
            },
            Self::Earth => OrbitialElements {
                a: 0.00000562,
                e: -0.00004392,
                inc: -0.01294668,
                mean_lon: 35999.37244981,
                omega_bar: 0.32327364,
                omega0: 0.0,
            },
            Self::Mars => OrbitialElements {
                a: 0.00001847,
                e: 0.00007882,
                inc: -0.00813131,
                mean_lon: 19140.30268499,
                omega_bar: 0.44441088,
                omega0: -0.29257343,
            },
            Self::Jupiter => OrbitialElements {
                a: -0.00011607,
                e: -0.00013253,
                inc: -0.00183714,
                mean_lon: 3034.74612775,
                omega_bar: 0.21252668,
                omega0: 0.20469106,
            },
            Self::Saturn => OrbitialElements {
                a: -0.0012506,
                e: -0.00050991,
                inc: 0.00193609,
                mean_lon: 1222.49362201,
                omega_bar: -0.41897216,
                omega0: -0.28867794,
            },
            Self::Uranus => OrbitialElements {
                a: -0.00196176,
                e: -0.00004397,
                inc: -0.00242939,
                mean_lon: 428.48202785,
                omega_bar: 0.40805281,
                omega0: 0.04240589,
            },
            Self::Neptune => OrbitialElements {
                a: 0.00026291,
                e: 0.00005105,
                inc: 0.00035372,
                mean_lon: 218.45945325,
                omega_bar: -0.32241464,
                omega0: -0.00508664,
            },
            Self::Pluto => OrbitialElements {
                a: -0.00031596,
                e: 0.0000517,
                inc: 0.00004818,
                mean_lon: 145.20780515,
                omega_bar: -0.04062942,
                omega0: -0.01183482,
            },
            _ => OrbitialElements {
                a: f64::NAN,
                e: f64::NAN,
                inc: f64::NAN,
                mean_lon: f64::NAN,
                omega_bar: f64::NAN,
                omega0: f64::NAN,
            },
        }
    }

    /// Keplerian elements, in degrees, valid between 1800AD to 2050AD
    const fn keplerian_element(&self) -> OrbitialElements {
        // table 1
        match self {
            Self::Sun => OrbitialElements {
                a: 0.0,
                e: 0.0,
                inc: 0.0,
                mean_lon: 0.0,
                omega_bar: 0.0,
                omega0: 0.0,
            },
            Self::Mercury => OrbitialElements {
                a: 0.38709927,
                e: 0.20563593,
                inc: 7.00497902,
                mean_lon: 252.2503235,
                omega_bar: 77.45779628,
                omega0: 48.33076593,
            },
            Self::Venus => OrbitialElements {
                a: 0.72333566,
                e: 0.00677672,
                inc: 3.39467605,
                mean_lon: 181.9790995,
                omega_bar: 131.60246718,
                omega0: 76.67984255,
            },
            Self::Earth => OrbitialElements {
                a: 1.00000261,
                e: 0.01671123,
                inc: -0.00001531,
                mean_lon: 100.46457166,
                omega_bar: 102.93768193,
                omega0: 0.0,
            },
            Self::Mars => OrbitialElements {
                a: 1.52371034,
                e: 0.0933941,
                inc: 1.84969142,
                mean_lon: -4.55343205,
                omega_bar: -23.94362959,
                omega0: 49.55953891,
            },
            Self::Jupiter => OrbitialElements {
                a: 5.202887,
                e: 0.04838624,
                inc: 1.30439695,
                mean_lon: 34.39644051,
                omega_bar: 14.72847983,
                omega0: 100.47390909,
            },
            Self::Saturn => OrbitialElements {
                a: 9.53667594,
                e: 0.05386179,
                inc: 2.48599187,
                mean_lon: 49.95424423,
                omega_bar: 92.59887831,
                omega0: 113.66242448,
            },
            Self::Uranus => OrbitialElements {
                a: 19.18916464,
                e: 0.04725744,
                inc: 0.77263783,
                mean_lon: 313.23810451,
                omega_bar: 170.9542763,
                omega0: 74.01692503,
            },
            Self::Neptune => OrbitialElements {
                a: 30.06992276,
                e: 0.00859048,
                inc: 1.77004347,
                mean_lon: -55.12002969,
                omega_bar: 44.96476227,
                omega0: 131.78422574,
            },
            Self::Pluto => OrbitialElements {
                a: 39.48211675,
                e: 0.2488273,
                inc: 17.14001206,
                mean_lon: 238.92903833,
                omega_bar: 224.06891629,
                omega0: 110.30393684,
            },
            _ => OrbitialElements {
                a: f64::NAN,
                e: f64::NAN,
                inc: f64::NAN,
                mean_lon: f64::NAN,
                omega_bar: f64::NAN,
                omega0: f64::NAN,
            },
        }
    }

    /// heliocentric equatorial rectangle coordinates(refers J2000 equinox) of the planet
    ///
    /// `t` should be julian centuries sicne JD2000,
    /// return (x, y, z) tuple
    fn icrs(&self, t: f64) -> (f64, f64, f64) {
        if let Self::Sun = self {
            return (0.0, 0.0, 0.0);
        }

        // 1.
        let kep_rad =
            self.keplerian_element().with_radians() + self.keplerian_rate().with_radians() * t;

        // 2.
        let mean_anom_rad = wrap_pi(kep_rad.mean_lon - kep_rad.omega_bar); // wrap between -pi to pi
        let w_rad = kep_rad.omega_bar - kep_rad.omega0;

        // 3.
        let ecc_anom = find_eccentritiy_anomaly(mean_anom_rad, kep_rad.e);

        // 4.
        let (ecc_anom_sin, ecc_anom_cos) = sincos(ecc_anom);
        let (x_prime, y_prime) = (
            kep_rad.a * (ecc_anom_cos - kep_rad.e),
            kep_rad.a * ecc_anom_sin * sqrt(1.0 - kep_rad.e * kep_rad.e),
        );

        // 5.
        let (w_sin, w_cos) = sincos(w_rad);
        let (omega0_sin, omega0_cos) = sincos(kep_rad.omega0);
        let (inc_sin, inc_cos) = sincos(kep_rad.inc);

        let x_ecl = (w_cos * omega0_cos - w_sin * omega0_sin * inc_cos) * x_prime
            + (-w_sin * omega0_cos - w_cos * omega0_sin * inc_cos) * y_prime;
        let y_ecl = (w_cos * omega0_sin + w_sin * omega0_cos * inc_cos) * x_prime
            + (-w_sin * omega0_sin + w_cos * omega0_cos * inc_cos) * y_prime;
        let z_ecl = (w_sin * inc_sin) * x_prime + (w_cos * inc_sin) * y_prime;

        // ecliptic to equatorial coordinates, refers to J2000 equinox
        let (eps_sin, eps_cos) = sincos(MEAN_OBLIQUITY_J2000_RAD);
        let x_eq = x_ecl;
        let y_eq = eps_cos * y_ecl - eps_sin * z_ecl;
        let z_eq = eps_sin * y_ecl + eps_cos * z_ecl;

        (x_eq, y_eq, z_eq)
    }

    /// geocentric equatorial rectangle coordinates(refers to J2000 equinox) of the planet
    ///
    /// `t` should be julian centuries sicne JD2000,
    /// return (x, y, z) tuple
    fn gcrs(&self, t: f64) -> (f64, f64, f64) {
        let (x_earth, y_earth, z_earth) = Self::Earth.icrs(t);
        let (x, y, z) = self.icrs(t);

        (x - x_earth, y - y_earth, z - z_earth)
    }

    /// geocentric equatorial rectangle coordinates(refers to of-date equinox) of the planet
    ///
    /// `t` should be julian centuries sicne JD2000,
    /// return (x, y, z) tuple
    fn gcrs_ofdate(&self, t: f64) -> (f64, f64, f64) {
        let (x, y, z) = self.gcrs(t);

        NpbTransformer::new(t).transform(x, y, z)
    }

    /// apparent geocentric RA, DEC and distance to the planet
    /// RA and DEC are in radians, distance is in AU
    ///
    /// return `(ra, dec, dist)`
    pub fn apparent_ra_dec_r(&self, jde: f64) -> (f64, f64, f64) {
        let t = (jde - J2000) / DAYS_PER_JULIAN_CENTURY;
        match self {
            SolarObject::Moon => moon_coord(t),
            _ => {
                let (x, y, z) = self.gcrs_ofdate(t);

                // convert to spherical coordinates
                let dist = sqrt(x * x + y * y + z * z);
                let mut ra = atan2(y, x);
                let mut dec = acos(z / dist);

                // ra should in range [0, 2*pi]
                if ra < 0.0 {
                    ra += TAU;
                }

                // dec should in range [-pi/2, pi/2]
                dec = 0.5 * PI - dec;

                (ra, dec, dist)
            }
        }
    }

    /// topocentric hour angle and declination by
    /// applying parallex to apparent geocentric equatorial coordinates
    ///
    /// return `(hour angle, dec)` in radians
    fn apparent_ha_dec_topocentric(
        &self,
        jd: f64,
        delta_t: f64,
        lat_rad: f64,
        lon_rad: f64,
    ) -> (f64, f64) {
        // sin(p_sun) = sin((8.7941433 * (1.0/3600.0)).to_radians())
        const SINE_EQUATORIAL_SOLAR_PARALLAX: f64 = 4.2635209841860855e-5;

        let (ra_rad, dec_rad, dist_au) = self.apparent_ra_dec_r(jd + delta_t);
        let ha_rad = apparent_sidereal_time(jd, delta_t, lon_rad) - ra_rad;

        // meeus ch.11
        // geocentric latitude
        let u = atan(0.99664719 * tan(lat_rad));
        let rho_sin_phi_prime =
            0.99664719 * sin(u) + (OBSERVER_ALTITUDE / 6378140.0) * sin(lat_rad);
        let rho_cos_phi_prime = cos(u) + (OBSERVER_ALTITUDE / 6378140.0) * cos(lat_rad);

        // convert to topcentric HA/DEC
        // meeus ch.40
        let par_sin = SINE_EQUATORIAL_SOLAR_PARALLAX / dist_au;

        // 40.6
        let (ha_sin, ha_cos) = sincos(ha_rad);
        let (dec_sin, dec_cos) = sincos(dec_rad);
        let a = dec_cos * ha_sin;
        let b = dec_cos * ha_cos - rho_cos_phi_prime * par_sin;
        let c = dec_sin - rho_sin_phi_prime * par_sin;
        let q = sqrt(a * a + b * b + c * c); // 40.7

        let ha_corrected = atan2(a, b);
        let dec_corrected = asin(c / q);

        (ha_corrected, dec_corrected)
    }

    /// horizontal position of the planet
    ///
    /// `jd`: julian days, `delta_t`: delta T in days,
    /// `lat`: latitude in degrees, `lon`: longitude in degrees
    pub fn get_pos(&self, jd: f64, delta_t: f64, lat: f64, lon: f64) -> HorizontalCoordinate {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();

        let (ha_rad, dec_rad) = self.apparent_ha_dec_topocentric(jd, delta_t, lat_rad, lon_rad);

        HorizontalCoordinate::from_equatorial(ha_rad, lat_rad, dec_rad).apparent_altitude()
    }

    /// find upcoming rise and set events,
    /// it searches forward 24 hours refers to the local midnight
    ///
    /// the event JD is in UT
    #[inline]
    pub fn get_rise_set_today(
        &self,
        local_midnight: &UtcDateTime,
        lat: f64,
        lon: f64,
        refracted_horizon_rad: f64,
    ) -> (Option<EventInfo>, Option<EventInfo>) {
        self.get_rise_set_with_range(local_midnight, 0, 24, lat, lon, refracted_horizon_rad)
    }

    /// find upcoming rise and set events by brute forcing the crossing point,
    /// it searches `hr_forward` hours forward and `hr_backward` hours backward refers to the `start` time
    ///
    /// the event JD is in UT
    ///
    /// derive from 'Astronomy on the Personal Computer, ch 3'
    pub fn get_rise_set_with_range(
        &self,
        start: &UtcDateTime,
        hr_backward: u8,
        hr_forward: u8,
        lat: f64,
        lon: f64,
        refracted_horizon_rad: f64,
    ) -> (Option<EventInfo>, Option<EventInfo>) {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();

        let jd0 = start.to_julian();
        let dt_days = start.delta_t(); // delta T

        // refraction
        let refracted_sine_horizon_altitude = sin(refracted_horizon_rad);

        let mut jd_rise = None;
        let mut jd_set = None;

        let mut quadratic = QuadraticInterpolator::default();

        // sine altitude at JD
        let sin_altitude = |jd: f64| {
            let (ha_rad, dec_rad) = self.apparent_ha_dec_topocentric(jd, dt_days, lat_rad, lon_rad);

            sine_altitude(dec_rad, lat_rad, ha_rad) - refracted_sine_horizon_altitude
        };

        // sine altitude of the object from given hour offset of JD0
        let sin_altitude_hr = |hr: f64| {
            let jd = jd0 + hr * DAY_PER_HOUR;
            sin_altitude(jd)
        };

        // search for any rise/set in jd0 - hr_backward hours to jd0 + hr_forward hours interval
        let mut hour_offset = -(hr_backward as f64) + 1.0;
        let mut y_minus = sin_altitude_hr(hour_offset - 1.0);
        while hour_offset <= hr_forward as f64 {
            if jd_rise.is_some() && jd_set.is_some() {
                break;
            }

            let y0 = sin_altitude_hr(hour_offset);
            let y_plus = sin_altitude_hr(hour_offset + 1.0);

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

        let to_event_info = |mut jd| {
            const DT: f64 = 50.0 / SECONDS_PER_DAY;
            const INV_2DT: f64 = 1.0 / (2.0 * DT);
            const MAX_ITER: u8 = 2;

            // newton iteration to refine the root, i.e, the rise/set time
            for _ in 0..MAX_ITER {
                let altitude = sin_altitude(jd);

                // central difference: dh/dt = (h(t + dt) - h(t - dt)) / (2*dt)
                // t0_next = t0_now - h/(dh/dt)
                let delta_altitude = INV_2DT * (sin_altitude(jd + DT) - sin_altitude(jd - DT));
                jd -= altitude / delta_altitude;
            }

            let HorizontalCoordinate { azimuth, .. } = self.get_pos(jd, dt_days, lat, lon);

            EventInfo { jd, azimuth }
        };
        let rise = jd_rise.map(to_event_info);
        let set = jd_set.map(to_event_info);

        (rise, set)
    }
}

fn find_eccentritiy_anomaly(mean_anom_rad: f64, e: f64) -> f64 {
    let mut delta_ecc_anom = 1.0_f64;
    let mut ecc_anom = mean_anom_rad + e * sin(mean_anom_rad); // E0

    // find E by solving kepler's equation by iteration
    // E0 = M + e*sin(M)
    // E = E0
    // while dE < 1e-6 {
    //     dM = M - E + e * sinE
    //     dE = dM / (1 - e * cosE)
    //     E += dE
    // }
    for _ in 0..15 {
        if delta_ecc_anom.abs() <= 1e-6 {
            break;
        }

        let (ecc_anom_sin, ecc_anom_cos) = sincos(ecc_anom);
        let delta_mean_anom = mean_anom_rad - ecc_anom + e * ecc_anom_sin;
        delta_ecc_anom = delta_mean_anom / (1.0 - e * ecc_anom_cos);

        ecc_anom += delta_ecc_anom;
    }

    ecc_anom
}
