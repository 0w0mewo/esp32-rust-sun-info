use crate::ui::components::DEG_SYM;
use crate::{
    MIDNIGHT,
    solar::sun,
    ui::{
        UpdateCmd,
        views::{DatetimeStatus, UpdateableFromCmd},
    },
};
use fasttime::Time;

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
                lst,
                last_ntp_status,
            } => self.datetime.update(datetime, lst, last_ntp_status),
            UpdateCmd::SetSolar {
                day_progress,
                sunrise_at,
                sunset_at,
                sundawn_at,
                sundusk_at,
                sunrise_azim,
                sunset_azim,
            } => {
                self.day_progress = day_progress;
                self.sunrise_at = sunrise_at;
                self.sunset_at = sunset_at;
                self.dawn_at = sundawn_at;
                self.dusk_at = sundusk_at;
                self.sunrise_azim = sunrise_azim;
                self.sunset_azim = sunset_azim;
            }
            _ => {}
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            day_progress: sun::DayProgress::Night,
            sunrise_at: MIDNIGHT,
            sunset_at: MIDNIGHT,
            dawn_at: MIDNIGHT,
            dusk_at: MIDNIGHT,
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
Dawn            {}
Sunrise ({:>3.0}{DEG_SYM})   {}
Sunet   ({:>3.0}{DEG_SYM})   {}
Dusk            {} 
"#,
            self.datetime,
            self.day_progress,
            self.dawn_at,
            self.sunrise_azim,
            self.sunrise_at,
            self.sunset_azim,
            self.sunset_at,
            self.dusk_at,
        )
    }
}
