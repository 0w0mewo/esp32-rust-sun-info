use crate::{
    AstronDatetimeExt,
    events::NtpStatus,
    ui::{
        UpdateCmd, UpdateableFromCmd,
        components::{CommonStatusTexts, Compass, MOON_SYM, PolarLine, SUN_SYM},
    },
};
use alloc::format;
use embedded_graphics::{pixelcolor, prelude::*};
use fasttime::DateTime;

mod moon_info;
mod positions;
mod status;
mod sun_info;

extern crate alloc;

#[derive(Clone)]
pub(crate) struct DatetimeStatus {
    datetime: DateTime,
    last_ntp_status: NtpStatus,
    lst: (u8, u8, u8),
}

impl DatetimeStatus {
    pub fn update(&mut self, datetime: DateTime, lst: (u8, u8, u8), last_ntp_status: NtpStatus) {
        self.last_ntp_status = last_ntp_status;
        self.lst = lst;
        self.datetime = datetime;
    }
}

impl Default for DatetimeStatus {
    fn default() -> Self {
        Self {
            lst: (0, 0, 0),
            last_ntp_status: Default::default(),
            datetime: DateTime::from_unix_timestamp(0, 0).unwrap(),
        }
    }
}

impl core::fmt::Display for DatetimeStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let t = if let NtpStatus::OK = self.last_ntp_status {
            format!(
                "{:02}:{:02}:{:02}",
                self.datetime.time.hour, self.datetime.time.minute, self.datetime.time.second
            )
        } else {
            format!("NTP {}", self.last_ntp_status)
        };

        let lst = if let NtpStatus::OK = self.last_ntp_status {
            format!("{:02}:{:02}:{:02}", self.lst.0, self.lst.1, self.lst.2)
        } else {
            format!("NTP {}", self.last_ntp_status)
        };

        write!(
            f,
            r#"JD {:.2}  {}
Civil            {}
Sidereal         {}"#,
            self.datetime.to_julian(),
            self.datetime.date,
            t,
            lst
        )
    }
}

pub enum View {
    Sun(sun_info::State),
    Moon(moon_info::State),
    Position(positions::State),
    Status(status::State),
}

impl UpdateableFromCmd for View {
    fn update(&mut self, cmd: &UpdateCmd) {
        match self {
            Self::Moon(state) => state.update(cmd),
            Self::Position(state) => state.update(cmd),
            Self::Sun(state) => state.update(cmd),
            Self::Status(state) => state.update(cmd),
        }
    }
}

impl Drawable for View {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        if let View::Position(state) = self {
            let center = target.bounding_box().center() + Point::new(32, 0);

            let compass = Compass::new(center, 64);
            compass.draw(target)?;

            // sun and moon azimuths, draw while it's above horizon
            let arm_len = 0.5 * compass.diameter as f64;
            [&state.sun_pos, &state.moon_pos]
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
                        .draw(target)
                        .unwrap_or_default();
                });

            // sunrise and sunset azimuth
            [&state.sunrise_azim, &state.sunset_azim]
                .into_iter()
                .for_each(|&az| {
                    PolarLine::new(compass.center, az, arm_len)
                        .draw(target)
                        .unwrap_or_default();
                });
        }

        let status_txt = match self {
            Self::Moon(state) => format!("{}", state),
            Self::Position(state) => format!("{}", state),
            Self::Sun(state) => format!("{}", state),
            Self::Status(state) => format!("{}", state),
        };

        CommonStatusTexts::new(Point::zero(), &status_txt).draw(target)?;

        Ok(())
    }
}
