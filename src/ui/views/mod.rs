use crate::{
    D2000, MIDNIGHT,
    events::NtpStatus,
    ui::{UpdateCmd, UpdateableFromCmd, components::CommonStatusTexts},
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
        match self {
            Self::Moon(state) => state.draw(target),
            Self::Position(state) => state.draw(target),
            Self::Sun(state) => state.draw(target),
            Self::Status(state) => state.draw(target),
            Self::Seasons(state) => state.draw(target),
        }
    }
}

pub(crate) trait TextBasedView: core::fmt::Display {
    fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = pixelcolor::BinaryColor>,
    {
        CommonStatusTexts::new(Point::zero(), &format!("{}", self)).draw(target)
    }
}
