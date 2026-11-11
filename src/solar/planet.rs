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

struct MeanAnomalyExtra {
    b: f64,
    c: f64,
    s: f64,
    f: f64,
}

impl MeanAnomalyExtra {
    pub fn with_radians(&self) -> Self {
        Self {
            b: self.b.to_radians(),
            c: self.c.to_radians(),
            s: self.s.to_radians(),
            f: self.f.to_radians(),
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
    /// Keplerian elements, in degrees
    const fn keplerian_element(&self) -> OrbitialElements {
        // table 2a
        match self {
            Self::Mercury => OrbitialElements {
                a: 0.38709843,
                e: 0.20563661,
                inc: 7.00559432,
                mean_lon: 252.25166724,
                omega_bar: 77.45771895,
                omega0: 48.33961819,
            },
            Self::Venus => OrbitialElements {
                a: 0.72332102,
                e: 0.00676399,
                inc: 3.39777545,
                mean_lon: 181.97970850,
                omega_bar: 131.76755713,
                omega0: 76.67261496,
            },
            Self::Earth => OrbitialElements {
                a: 1.00000018,
                e: 0.01673163,
                inc: -0.00054346,
                mean_lon: 100.46691572,
                omega_bar: 102.93005885,
                omega0: -5.11260389,
            },
            Self::Mars => OrbitialElements {
                a: 1.52371243,
                e: 0.09336511,
                inc: 1.85181869,
                mean_lon: -4.56813164,
                omega_bar: -23.91744784,
                omega0: 49.71320984,
            },
            Self::Jupiter => OrbitialElements {
                a: 5.20248019,
                e: 0.04853590,
                inc: 1.29861416,
                mean_lon: 34.33479152,
                omega_bar: 14.27495244,
                omega0: 100.29282654,
            },
            Self::Saturn => OrbitialElements {
                a: 9.54149883,
                e: 0.05550825,
                inc: 2.49424102,
                mean_lon: 50.07571329,
                omega_bar: 92.86136063,
                omega0: 113.63998702,
            },
            Self::Uranus => OrbitialElements {
                a: 19.18797948,
                e: 0.04685740,
                inc: 0.77298127,
                mean_lon: 314.20276625,
                omega_bar: 172.43404441,
                omega0: 73.96250215,
            },
            Self::Neptune => OrbitialElements {
                a: 30.06952752,
                e: 0.00895439,
                inc: 1.77005520,
                mean_lon: 304.22289287,
                omega_bar: 46.68158724,
                omega0: 131.78635853,
            },
            Self::Sun => OrbitialElements {
                a: 0.0,
                e: 0.0,
                inc: 0.0,
                mean_lon: 0.0,
                omega_bar: 0.0,
                omega0: 0.0,
            },
            Self::Pluto => OrbitialElements {
                a: 39.48686035,
                e: 0.24885238,
                inc: 17.14104260,
                mean_lon: 238.96535011,
                omega_bar: 224.09702598,
                omega0: 110.30167986,
            },
            _ => OrbitialElements {
                a: f64::INFINITY,
                e: f64::INFINITY,
                inc: f64::INFINITY,
                mean_lon: f64::INFINITY,
                omega_bar: f64::INFINITY,
                omega0: f64::INFINITY,
            },
        }
    }

    /// keplerian rates in degrees/century
    const fn keplerian_rate(&self) -> OrbitialElements {
        // table 2a
        match self {
            Self::Mercury => OrbitialElements {
                a: 0.00000000,
                e: 0.00002123,
                inc: -0.00590158,
                mean_lon: 149472.67486623,
                omega_bar: 0.15940013,
                omega0: -0.12214182,
            },
            Self::Venus => OrbitialElements {
                a: -0.00000026,
                e: -0.00005107,
                inc: 0.00043494,
                mean_lon: 58517.81560260,
                omega_bar: 0.05679648,
                omega0: -0.27274174,
            },
            Self::Earth => OrbitialElements {
                a: -0.00000003,
                e: -0.00003661,
                inc: -0.01337178,
                mean_lon: 35999.37306329,
                omega_bar: 0.31795260,
                omega0: -0.24123856,
            },
            Self::Mars => OrbitialElements {
                a: 0.00000097,
                e: 0.00009149,
                inc: -0.00724757,
                mean_lon: 19139.29934243,
                omega_bar: 0.45223625,
                omega0: -0.26852431,
            },
            Self::Jupiter => OrbitialElements {
                a: -0.00002864,
                e: 0.00018026,
                inc: -0.00322699,
                mean_lon: 3034.90371757,
                omega_bar: 0.18199196,
                omega0: 0.13024619,
            },
            Self::Saturn => OrbitialElements {
                a: -0.00003065,
                e: -0.00032044,
                inc: 0.00451969,
                mean_lon: 1221.11494724,
                omega_bar: 0.54179478,
                omega0: -0.25015002,
            },
            Self::Uranus => OrbitialElements {
                a: -0.00020455,
                e: -0.00001550,
                inc: -0.00180155,
                mean_lon: 428.49512595,
                omega_bar: 0.09266985,
                omega0: 0.05739699,
            },
            Self::Neptune => OrbitialElements {
                a: 0.00006447,
                e: 0.00000818,
                inc: 0.00022400,
                mean_lon: 218.46515314,
                omega_bar: 0.01009938,
                omega0: -0.00606302,
            },
            Self::Sun => OrbitialElements {
                a: 0.0,
                e: 0.0,
                inc: 0.0,
                mean_lon: 0.0,
                omega_bar: 0.0,
                omega0: 0.0,
            },
            Self::Pluto => OrbitialElements {
                a: 0.00449751,
                e: 0.00006016,
                inc: 0.00000501,
                mean_lon: 145.18042903,
                omega_bar: -0.00968827,
                omega0: -0.00809981,
            },
            _ => OrbitialElements {
                a: f64::INFINITY,
                e: f64::INFINITY,
                inc: f64::INFINITY,
                mean_lon: f64::INFINITY,
                omega_bar: f64::INFINITY,
                omega0: f64::INFINITY,
            },
        }
    }

    /// extra correction parameters when calculating mean anomaly of a planet,
    /// return `None` if no correction needed
    const fn mean_anomaly_extra(&self) -> Option<MeanAnomalyExtra> {
        // table 2b
        match self {
            Self::Jupiter => Some(MeanAnomalyExtra {
                b: -0.00012452,
                c: 0.06064060,
                s: -0.35635438,
                f: 38.35125000,
            }),
            Self::Saturn => Some(MeanAnomalyExtra {
                b: 0.00025899,
                c: -0.13434469,
                s: 0.87320147,
                f: 38.35125000,
            }),
            Self::Uranus => Some(MeanAnomalyExtra {
                b: 0.00058331,
                c: -0.97731848,
                s: 0.17689245,
                f: 7.67025000,
            }),
            Self::Neptune => Some(MeanAnomalyExtra {
                b: -0.00041348,
                c: 0.68346318,
                s: -0.10162547,
                f: 7.67025000,
            }),
            Self::Pluto => Some(MeanAnomalyExtra {
                b: -0.01262724,
                c: 0.0,
                s: 0.0,
                f: 0.0,
            }),
            _ => None,
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
        let anom_correction_rad = if let Some(coffs) = self.mean_anomaly_extra() {
            let coffs = coffs.with_radians();
            let (ft_sin, ft_cos) = sincos(t * coffs.f);

            coffs.b * t * t + coffs.c * ft_cos + coffs.s * ft_sin
        } else {
            0.0
        };

        // 2.
        let mean_anom_rad = wrap_pi(kep_rad.mean_lon - kep_rad.omega_bar + anom_correction_rad); // wrap between -pi to pi
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
