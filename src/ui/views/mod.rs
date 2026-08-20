use crate::{
    D2000, MIDNIGHT,
    events::NtpStatus,
    ui::{
        UpdateCmd, UpdateableFromCmd,
        components::{CommonStatusTexts, Compass, MOON_SYM, PolarLine, SUN_SYM},
    },
};
use alloc::format;
use embedded_graphics::{pixelcolor, prelude::*};
use fasttime::{DateTime, OffsetDateTime, UtcOffset};

mod moon_info;
mod positions;
mod seasons;
mod status;
mod sun_info;

extern crate alloc;

#[derive(Clone)]
pub(crate) struct DatetimeStatus {
    datetime: OffsetDateTime,
    last_ntp_status: NtpStatus,
}

impl DatetimeStatus {
    pub fn update(&mut self, datetime: OffsetDateTime, last_ntp_status: NtpStatus) {
        self.last_ntp_status = last_ntp_status;
        self.datetime = datetime;
    }
}

impl Default for DatetimeStatus {
    fn default() -> Self {
        Self {
            last_ntp_status: Default::default(),
            datetime: OffsetDateTime::from_utc(
                DateTime::new(D2000, MIDNIGHT),
                UtcOffset::from_seconds(0).unwrap(),
            ),
        }
    }
}

impl core::fmt::Display for DatetimeStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let local_now = self.datetime.to_local().unwrap();
        let utc_now = &self.datetime.utc;

        let local_time_line = if let NtpStatus::OK = self.last_ntp_status {
            format!(
                "{} {:02}:{:02}:{:02}",
                local_now.date, local_now.time.hour, local_now.time.minute, local_now.time.second
            )
        } else {
            format!("NTP {}", self.last_ntp_status)
        };

        let utc_time_line = if let NtpStatus::OK = self.last_ntp_status {
            format!(
                "{} {:02}:{:02}:{:02}",
                utc_now.date, utc_now.time.hour, utc_now.time.minute, utc_now.time.second
            )
        } else {
            format!("NTP {}", self.last_ntp_status)
        };

        write!(
            f,
            r#"UTC   {}
LT    {}
---"#,
            utc_time_line, local_time_line,
        )
    }
}

pub enum View {
    Sun(sun_info::State),
    Moon(moon_info::State),
    Position(positions::State),
    Status(status::State),
    Seasons(seasons::State),
}

impl UpdateableFromCmd for View {
    fn update(&mut self, cmd: &UpdateCmd) {
        match self {
            Self::Moon(state) => state.update(cmd),
            Self::Position(state) => state.update(cmd),
            Self::Sun(state) => state.update(cmd),
            Self::Status(state) => state.update(cmd),
            Self::Seasons(state) => state.update(cmd),
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
                        .draw_line(false)
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

            // moonrise and moonset azimuth
            [&state.moonrise_azim, &state.moonset_azim]
                .into_iter()
                .for_each(|&az| {
                    PolarLine::with_label(compass.center, az, arm_len, "m")
                        .label_at_line_middle(true)
                        .draw(target)
                        .unwrap_or_default();
                });
        }

        let status_txt = match self {
            Self::Moon(state) => format!("{}", state),
            Self::Position(state) => format!("{}", state),
            Self::Sun(state) => format!("{}", state),
            Self::Status(state) => format!("{}", state),
            Self::Seasons(state) => format!("{}", state),
        };

        CommonStatusTexts::new(Point::zero(), &status_txt).draw(target)?;

        Ok(())
    }
}
