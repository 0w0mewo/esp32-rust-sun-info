use libm::sincos;

use crate::{
    math::RADS_PER_ARCSECOND,
    solar::{mean_obliq, nutation::nutation_iau2006},
};

/// IAU2006 precession and nutation model
pub(crate) struct NpbTransformer {
    pub gamma: f64,
    pub phi: f64,
    pub psi: f64,
    pub eps: f64,
}

impl NpbTransformer {
    /// `t`: julian centuries since JD2000
    pub fn new(t: f64) -> Self {
        let mut angles = Self {
            eps: 0.0,
            gamma: 0.0,
            phi: 0.0,
            psi: 0.0,
        };

        angles.update(t);

        angles
    }

    /// recalculate angles with new julian centuries, the `t` should be julian centuries since JD2000
    pub fn update(&mut self, t: f64) {
        let t2 = t * t;
        let t3 = t2 * t;
        let t4 = t3 * t;
        let t5 = t4 * t;

        // recalc Fukushima-Williams angles for building PB matrix, converted from arcseconds to radians
        self.gamma =
            (-0.052928 + 10.556378 * t + 0.4932044 * t2 - 0.00031238 * t3 - 0.000002788 * t4
                + 0.0000000260 * t5)
                * RADS_PER_ARCSECOND;
        self.phi = (84381.412819 - 46.811016 * t + 0.0511268 * t2 + 0.00053289 * t3
            - 0.000000440 * t4
            - 0.0000000176 * t5)
            * RADS_PER_ARCSECOND;
        self.psi = (-0.041775 + 5038.481484 * t + 1.5584175 * t2
            - 0.00018522 * t3
            - 0.000026452 * t4
            - 0.0000000148 * t5)
            * RADS_PER_ARCSECOND;
        self.eps = mean_obliq(t);

        // apply nutation to eps and psi
        let (dpsi, deps) = nutation_iau2006(t);
        self.eps += deps;
        self.psi += dpsi;
    }

    /// transform geocentric equatorial coordinates that refers to J2000 equinox to of-date equinox
    /// return `(x, y, z)`
    pub fn transform(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let (eps_sin, eps_cos) = sincos(self.eps);
        let (psi_sin, psi_cos) = sincos(self.psi);
        let (phi_sin, phi_cos) = sincos(self.phi);
        let (gamma_sin, gamma_cos) = sincos(self.gamma);

        // r_to_date = Rx(-eps) * Rz(-psi) * Rx(phi) * Rz(gamma) * r_j2000
        let x_prime = (psi_cos * gamma_cos + psi_sin * phi_cos * gamma_sin) * x
            + (psi_cos * gamma_sin - psi_sin * phi_cos * gamma_cos) * y
            - psi_sin * phi_sin * z;
        let y_prime = (eps_cos * psi_sin * gamma_cos
            - eps_cos * psi_cos * phi_cos * gamma_sin
            - eps_sin * phi_sin * gamma_sin)
            * x
            + (eps_cos * psi_sin * gamma_sin
                + eps_cos * psi_cos * phi_cos * gamma_cos
                + eps_sin * phi_sin * gamma_cos)
                * y
            + (eps_cos * psi_cos * phi_sin - eps_sin * phi_cos) * z;
        let z_prime = (eps_sin * psi_sin * gamma_cos - eps_sin * psi_cos * phi_cos * gamma_sin
            + eps_cos * phi_sin * gamma_sin)
            * x
            + (eps_sin * psi_sin * gamma_sin + eps_sin * psi_cos * phi_cos * gamma_cos
                - eps_cos * phi_sin * gamma_cos)
                * y
            + (eps_sin * psi_cos * phi_sin + eps_cos * phi_cos) * z;

        (x_prime, y_prime, z_prime)
    }
}
