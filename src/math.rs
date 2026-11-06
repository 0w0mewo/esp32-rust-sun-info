use core::f64::consts::{PI, TAU};

use libm::sqrt;

pub const HOUR_PER_RAD: f64 = (1.0_f64 / 15.0).to_radians();

/// radian per arcseconds, multiply this constant with arcseconds to convert to radians
pub const RADS_PER_ARCSECOND: f64 = (1.0_f64 / 3600.0).to_radians();

#[derive(Clone)]
pub enum QuadraticRoots {
    /// one root
    One { root: f64 },
    /// two roots
    Two { root1: f64, root2: f64 },
}

/// fit to y = a*x^2 + b*x + c
#[derive(Default)]
pub struct QuadraticInterpolator {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    xe: f64,
    ye: f64,
}

impl QuadraticInterpolator {
    /// x extremum of the fitted quadratic
    pub fn x_extremum(&self) -> f64 {
        self.xe
    }

    /// y extremum of the fitted quadratic
    pub fn y_extremum(&self) -> f64 {
        self.ye
    }

    /// compute roots of the fitted quadratic, i.e, the x value of the quadratic when y = 0
    ///
    /// return None if no roots are found
    pub fn roots(&self) -> Option<QuadraticRoots> {
        let dis = self.b * self.b - 4.0 * self.a * self.c;
        if dis < 0.0 {
            return None;
        }

        let dx = 0.5 * sqrt(dis) / self.a.abs();
        let mut root1 = self.xe - dx;
        let root2 = self.xe + dx;

        let mut nroots = 0;
        if root1.abs() <= 1.0 {
            nroots += 1;
        }

        if root2.abs() <= 1.0 {
            nroots += 1;
        }

        if root1 < -1.0 {
            root1 = root2;
        }

        match nroots {
            0 => None,
            1 => Some(QuadraticRoots::One { root: root1 }),
            2 => Some(QuadraticRoots::Two { root1, root2 }),
            _ => unreachable!(),
        }
    }

    /// quadratic interpolation from given y values
    pub fn fit(&mut self, y_start: f64, y_mid: f64, y_end: f64) {
        self.a = 0.5 * (y_end + y_start) - y_mid;
        self.b = 0.5 * (y_end - y_start);
        self.c = y_mid;

        self.xe = -self.b / (2.0 * self.a);
        self.ye = (self.a * self.xe + self.b) * self.xe + self.c;
    }
}

pub fn wrap_pi(rad: f64) -> f64 {
    let rad = rad % TAU;
    if rad.abs() > PI { rad - TAU } else { rad }
}
