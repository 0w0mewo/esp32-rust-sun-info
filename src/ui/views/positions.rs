use embedded_graphics::{Drawable, pixelcolor, prelude::*};
extern crate alloc;
use alloc::format;

use crate::{
    AstronDatetimeExt, HorizontalCoordinate, solar::SolarObject, ui::{
        UpdateCmd, components::{CommonStatusTexts, Compass, DEG_SYM, MOON_SYM, PolarLine, SUN_SYM}, views::{DatetimeStatus, UpdateableFromCmd},
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

        let compass = Compass::new(center, 64);
        compass.draw(target)?;

        // sun and moon azimuths, draw while it's above horizon
        let arm_len = 0.5 * compass.diameter as f64;
        [&self.sun_pos, &self.moon_pos]
            .into_iter()
            .enumerate()
            .filter(|(_, pos)| pos.altitude >= 0.0)
            .for_each(|(id, pos)| {
                // the closer to zenith, the shorter the arm length
                let arm_len = arm_len * (1.0 - (pos.altitude.abs() / 90.0));

                // select symbol
                let symb = match id {
                    0 => SUN_SYM,
                    1 => MOON_SYM,
                    _ => unreachable!(),
                };

                PolarLine::with_label(compass.center, pos.azimuth, arm_len, symb)
                    .draw_line(false)
                    .draw(target)
                    .unwrap_or_default();
            });

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
        
        CommonStatusTexts::new(Point::zero(), &format!("{}", self)).draw(target)?;

        Ok(())
    }
}
