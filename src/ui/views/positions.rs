use embedded_graphics::{
    Drawable,
    mono_font::MonoTextStyle,
    pixelcolor,
    prelude::*,
    text::{Baseline, Text},
};
extern crate alloc;
use alloc::format;
use embedded_graphics_unicodefonts::MONO_5X7;
use libm::{round, sincos};

use crate::{
    AstronDatetimeExt, HorizontalCoordinate,
    solar::SolarObject,
    ui::{
        UpdateCmd,
        components::{CommonStatusTexts, Compass, DEG_SYM, MOON_SYM, PolarLine, SUN_SYM},
        views::{DatetimeStatus, UpdateableFromCmd},
    },
};

#[derive(Default)]
pub struct State {
    pub(in crate::ui::views) datetime: DatetimeStatus,
    pub(in crate::ui::views) sun_pos: HorizontalCoordinate,
    pub(in crate::ui::views) moon_pos: HorizontalCoordinate,
    pub(in crate::ui::views) sunrise_azim: f64,
    pub(in crate::ui::views) sunset_azim: f64,
    pub(in crate::ui::views) moonrise_azim: f64,
    pub(in crate::ui::views) moonset_azim: f64,
    pub(in crate::ui::views) altitude_view: bool,
}

impl UpdateableFromCmd for State {
    fn update(&mut self, cmd: &UpdateCmd) {
        match *cmd {
            UpdateCmd::SetDatetime {
                datetime,
                last_ntp_status,
            } => self.datetime.update(datetime, last_ntp_status),

            UpdateCmd::SetPosition { pos, obj } => match obj {
                SolarObject::Moon => self.moon_pos = pos,
                SolarObject::Sun => self.sun_pos = pos,
            },

            UpdateCmd::SetRiseSetDirection {
                obj,
                rise_azim,
                set_azim,
                ..
            } => match obj {
                SolarObject::Moon => {
                    self.moonrise_azim = rise_azim;
                    self.moonset_azim = set_azim;
                }

                SolarObject::Sun => {
                    self.sunrise_azim = rise_azim;
                    self.sunset_azim = set_azim;
                }
            },

            UpdateCmd::SwitchView => {
                self.altitude_view = !self.altitude_view;
            }

            _ => {}
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let utc_time = &self.datetime.datetime.utc;
        let local_time = self.datetime.datetime.to_local().unwrap();
        write!(
            f,
            r#"JD {:.2}
UTC  {:02}:{:02}:{:02}
LT   {:02}:{:02}:{:02}
Sun pos.
 Az  {:>6.2}{DEG_SYM} 
 Alt {:>6.2}{DEG_SYM}
Moon pos.
 Az  {:>6.2}{DEG_SYM} 
 Alt {:>6.2}{DEG_SYM}
  "#,
            utc_time.to_julian(),
            utc_time.time.hour,
            utc_time.time.minute,
            utc_time.time.second,
            local_time.time.hour,
            local_time.time.minute,
            local_time.time.second,
            self.sun_pos.azimuth,
            self.sun_pos.altitude,
            self.moon_pos.azimuth,
            self.moon_pos.altitude
        )
    }
}

impl Drawable for State {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        let center = target.bounding_box().center() + Point::new(32, 0);

        let compass = Compass::new(center, 64).altitude_mode(self.altitude_view);
        compass.draw(target)?;

        // sun and moon azimuths and altitudes, draw while it's above horizon
        let arm_len = 0.5 * compass.diameter as f64;
        [&self.sun_pos, &self.moon_pos]
            .into_iter()
            .enumerate()
            .filter(|(_, pos)| pos.altitude >= 0.0)
            .for_each(|(id, pos)| {
                // select symbol
                let symb = match id {
                    0 => SUN_SYM,
                    1 => MOON_SYM,
                    _ => unreachable!(),
                };

                let az_rad = pos.azimuth.to_radians();
                let alt_rad = pos.altitude.to_radians();
                let (alt_sin, alt_cos) = sincos(alt_rad);
                let (az_sin, az_cos) = sincos(az_rad);

                // convert spherical coordinate to cartesian coordinates for simple 2D projection on
                // screen
                // x = r*sin(inclination)*sin(azimuth) = r*cos(declination)*sin(azimuth)
                // y = r*sin(inclination)*cos(azimuth) = r*cos(declination)*cos(azimuth)
                // z = r*cos(inclination) = r*sin(declination)
                let r = arm_len - 3.0;
                let x = r * alt_cos * az_sin;
                let y = -r * alt_cos * az_cos;
                let z = -r * alt_sin;

                // projects the converted XZ plane to the screen in altitude view,
                // XY plane in azimuth view
                let pos = center
                    + if self.altitude_view {
                        Point::new(round(x) as i32, round(z) as i32)
                    } else {
                        Point::new(round(x) as i32, round(y) as i32)
                    };
                Text::with_baseline(
                    symb,
                    pos,
                    MonoTextStyle::new(&MONO_5X7, pixelcolor::BinaryColor::On),
                    Baseline::Middle,
                )
                .draw(target)
                .unwrap_or_default();
            });

        // draw rise/set azimuth when it's not altitude view
        if !self.altitude_view {
            // sunrise and sunset azimuth
            [&self.sunrise_azim, &self.sunset_azim]
                .into_iter()
                .for_each(|&az| {
                    PolarLine::new(compass.center, az, arm_len)
                        .draw(target)
                        .unwrap_or_default();
                });

            // moonrise and moonset azimuth
            [&self.moonrise_azim, &self.moonset_azim]
                .into_iter()
                .for_each(|&az| {
                    PolarLine::with_label(compass.center, az, arm_len, "m")
                        .label_at_line_middle(true)
                        .draw(target)
                        .unwrap_or_default();
                });
        }

        CommonStatusTexts::new(Point::zero(), &format!("{}", self)).draw(target)?;

        Ok(())
    }
}
