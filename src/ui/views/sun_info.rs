use time::Time;

use crate::datetime::UNIX_EPOCH_PLAIN;
use crate::solar::SolarObject;
use crate::ui::components::DEG_SYM;
use crate::ui::views::TextBasedView;
use crate::{
    solar::sun,
    ui::{
        UpdateCmd,
        views::{DatetimeStatus, UpdateableFromCmd},
    },
};

pub struct State {
    pub(in crate::ui::views) datetime: DatetimeStatus,
    pub(in crate::ui::views) day_progress: sun::DayProgress,
    pub(in crate::ui::views) sunrise_at: Time,
    pub(in crate::ui::views) sunset_at: Time,
    pub(in crate::ui::views) dawn_at: Time,
    pub(in crate::ui::views) dusk_at: Time,
    pub(in crate::ui::views) sunrise_azim: f64,
    pub(in crate::ui::views) sunset_azim: f64,
}

impl UpdateableFromCmd for State {
    fn update(&mut self, cmd: &UpdateCmd) {
        match *cmd {
            UpdateCmd::SetDatetime {
                datetime,
                last_ntp_status,
            } => self.datetime.update(datetime, last_ntp_status),

            UpdateCmd::SetRiseSet {
                rise_at,
                set_at,
                rise_azim,
                set_azim,
                obj,
            } => {
                if let SolarObject::Sun = obj {
                    self.sunrise_at = rise_at.unwrap_or(UNIX_EPOCH_PLAIN).time();
                    self.sunset_at = set_at.unwrap_or(UNIX_EPOCH_PLAIN).time();
                    self.sunrise_azim = rise_azim;
                    self.sunset_azim = set_azim;
                }
            }

            UpdateCmd::SetSolar {
                day_progress,
                sundawn_at,
                sundusk_at,
            } => {
                self.day_progress = day_progress;
                self.dawn_at = sundawn_at;
                self.dusk_at = sundusk_at;
            }
            _ => {}
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            day_progress: sun::DayProgress::Night,
            sunrise_at: Time::MIDNIGHT,
            sunset_at: Time::MIDNIGHT,
            dawn_at: Time::MIDNIGHT,
            dusk_at: Time::MIDNIGHT,
            datetime: Default::default(),
            sunrise_azim: Default::default(),
            sunset_azim: Default::default(),
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            r#"{}
Solar prog.     {}
Dawn            {:02}:{:02}:{:02}
Sunrise ({:>3.0}{DEG_SYM})  {:02}:{:02}:{:02}
Sunet   ({:>3.0}{DEG_SYM})  {:02}:{:02}:{:02}
Dusk            {:02}:{:02}:{:02} 
"#,
            self.datetime,
            self.day_progress,
            self.dawn_at.hour(),
            self.dawn_at.minute(),
            self.dawn_at.second(),
            self.sunrise_azim,
            self.sunrise_at.hour(),
            self.sunrise_at.minute(),
            self.sunrise_at.second(),
            self.sunset_azim,
            self.sunset_at.hour(),
            self.sunset_at.minute(),
            self.sunset_at.second(),
            self.dusk_at.hour(),
            self.dusk_at.minute(),
            self.dusk_at.second(),
        )
    }
}

impl TextBasedView for State {}
