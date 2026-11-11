use libm::sincos;

use crate::{
    datetime::{DAYS_PER_JULIAN_CENTURY, J2000},
    math::RADS_PER_ARCSECOND,
};

/// IAU20006 nutation model in radians,
///
/// `t`: julian century since JD2000
///
/// return (dpsi, deps)
pub(crate) fn nutation_iau2006(t: f64) -> (f64, f64) {
    let jd = t / DAYS_PER_JULIAN_CENTURY + J2000;
    let fj2 = -2.7774e-6 * jd;
    let (mut dpsi, mut deps) = nutation_iau2000b(t);

    dpsi += dpsi * (0.4697e-6 + fj2) * RADS_PER_ARCSECOND;
    deps += deps * fj2 * RADS_PER_ARCSECOND;

    (dpsi, deps)
}

struct NutationCoff {
    /// l
    nl: i8,
    /// l'
    nlp: i8,
    /// F
    nf: i8,
    /// D
    nd: i8,
    /// Om
    nom: i8,
    /// longitude sin0
    ps: f64,
    /// t * sin
    pst: f64,
    /// cos cofficient
    pc: f64,
    /// obliquity cos
    ec: f64,
    /// t * cos
    ect: f64,
    /// sin cofficient
    es: f64,
}

const NUTATION_COFFS: &[NutationCoff] = &[
    NutationCoff {
        nl: 0,
        nlp: 0,
        nf: 0,
        nd: 0,
        nom: 1,
        ps: -172064161.0,
        pst: -174666.0,
        pc: 33386.0,
        ec: 92052331.0,
        ect: 9086.0,
        es: 15377.0,
    },
    NutationCoff {
        nl: 0,
        nlp: 0,
        nf: 2,
        nd: -2,
        nom: 2,
        ps: -13170906.0,
        pst: -1675.0,
        pc: -13696.0,
        ec: 5730336.0,
        ect: -3015.0,
        es: -4587.0,
    },
    NutationCoff {
        nl: 0,
        nlp: 0,
        nf: 2,
        nd: 0,
        nom: 2,
        ps: -2276413.0,
        pst: -234.0,
        pc: -11467.0,
        ec: 978459.0,
        ect: -485.0,
        es: -1455.0,
    },
    NutationCoff {
        nl: 0,
        nlp: 0,
        nf: 0,
        nd: 0,
        nom: 2,
        ps: 2074554.0,
        pst: 207.0,
        pc: 5765.0,
        ec: -897492.0,
        ect: 470.0,
        es: -514.0,
    },
    NutationCoff {
        nl: 0,
        nlp: 1,
        nf: 0,
        nd: 0,
        nom: 0,
        ps: 1475877.0,
        pst: -3633.0,
        pc: 0.0,
        ec: 73871.0,
        ect: -184.0,
        es: 0.0,
    },
    NutationCoff {
        nl: 0,
        nlp: 1,
        nf: 2,
        nd: -2,
        nom: 2,
        ps: -516821.0,
        pst: 1226.0,
        pc: -524.0,
        ec: 224386.0,
        ect: -677.0,
        es: -174.0,
    },
    NutationCoff {
        nl: 1,
        nlp: 0,
        nf: 0,
        nd: 0,
        nom: 0,
        ps: 711159.0,
        pst: 73.0,
        pc: 0.0,
        ec: -6750.0,
        ect: 0.0,
        es: 0.0,
    },
    NutationCoff {
        nl: 0,
        nlp: 0,
        nf: 2,
        nd: 0,
        nom: 1,
        ps: -387298.0,
        pst: -367.0,
        pc: 0.0,
        ec: 200728.0,
        ect: 18.0,
        es: 0.0,
    },
    NutationCoff {
        nl: 1,
        nlp: 0,
        nf: 2,
        nd: 0,
        nom: 2,
        ps: -301461.0,
        pst: -36.0,
        pc: 0.0,
        ec: 129025.0,
        ect: -63.0,
        es: 0.0,
    },
];

/// IAU2000B nutation model in radians, `t`: julian centuries sicne JD2000
///
/// return (dpsi, deps)
pub(crate) fn nutation_iau2000b(t: f64) -> (f64, f64) {
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;

    // mean anomaly of the moon
    let el = (485868.249036 + 1717915923.2178 * t + 31.8792 * t2 + 0.051635 * t3 - 0.00024470 * t4)
        * RADS_PER_ARCSECOND;

    // mean anomaly of the sun
    let elp = (1287104.79305 + 129596581.0481 * t - 0.5532 * t2 + 0.000136 * t3 - 0.00001149 * t4)
        * RADS_PER_ARCSECOND;

    // mean argument of the latitude of the moon
    let f = (335779.526232 + 1739527262.8478 * t - 12.7512 * t2 - 0.001037 * t3 + 0.00000417 * t4)
        * RADS_PER_ARCSECOND;

    // mean elongation of the moon from the sun
    let d = (1072260.70369 + 1602961601.2090 * t - 6.3706 * t2 + 0.006593 * t3 - 0.00003169 * t4)
        * RADS_PER_ARCSECOND;

    // mean longitude of the ascending node of the moon
    let om = (450160.398036 - 6962890.5431 * t + 7.4722 * t2 + 0.007702 * t3 - 0.00005939 * t4)
        * RADS_PER_ARCSECOND;

    // sum of luni-solar nutation series
    let (mut dpsi, mut deps) = NUTATION_COFFS.iter().fold((0.0, 0.0), |acc, term| {
        let arg = term.nl as f64 * el
            + term.nlp as f64 * elp
            + term.nf as f64 * f
            + term.nd as f64 * d
            + term.nom as f64 * om;
        let (arg_sin, arg_cos) = sincos(arg);

        let dp = (term.ps + term.pst * t) * arg_sin + term.pc * arg_cos;
        let de = (term.ec + term.ect * t) * arg_cos + term.es * arg_sin;

        (acc.0 + dp, acc.1 + de)
    });

    // Convert from 0.1 µas to radians
    // 0.1 µas = 0.1e-6 arcsec = 1e-7 arcsec
    let factor = 1e-7 * RADS_PER_ARCSECOND;

    dpsi *= factor;
    deps *= factor;

    dpsi += -0.135 * RADS_PER_ARCSECOND / 1000.0; // -0.135 mas
    deps += 0.388 * RADS_PER_ARCSECOND / 1000.0; //  0.388 mas

    (dpsi, deps)
}
