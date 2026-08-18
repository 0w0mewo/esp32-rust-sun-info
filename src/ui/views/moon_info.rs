use fasttime::{Date, Time};

use crate::{
    D2000, MIDNIGHT,
    solar::moon,
    ui::{
        UpdateCmd,
        views::{DatetimeStatus, UpdateableFromCmd},
    },
};

pub struct State {
    pub(in crate::ui::views) datetime: DatetimeStatus,
    pub(in crate::ui::views) lunar_phase: moon::Phase,
    pub(in crate::ui::views) lunar_illumination: f64,
    pub(in crate::ui::views) next_new_moon: Date,
    pub(in crate::ui::views) next_full_moon: Date,
    pub(in crate::ui::views) moonrise: Time,
    pub(in crate::ui::views) moonset: Time,
}

impl UpdateableFromCmd for State {
    fn update(&mut self, cmd: &UpdateCmd) {
        match *cmd {
            UpdateCmd::SetDatetime {
                datetime,
                lst,
                last_ntp_status,
            } => self.datetime.update(datetime, lst, last_ntp_status),

            UpdateCmd::SetLunar {
                lunar_phase,
                lunar_illumination,
                next_new_moon,
                next_full_moon,
                moonrise_at,
                moonset_at,
            } => {
                self.lunar_phase = lunar_phase;
                self.lunar_illumination = lunar_illumination;
                self.next_full_moon = next_full_moon;
                self.next_new_moon = next_new_moon;
                self.moonrise = moonrise_at;
                self.moonset = moonset_at;
            }

            _ => (),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            datetime: Default::default(),
            lunar_phase: Default::default(),
            lunar_illumination: Default::default(),
            next_new_moon: D2000,
            next_full_moon: D2000,
            moonrise: MIDNIGHT,
            moonset: MIDNIGHT,
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            r#"{}
Lunar phase   {}({:>4.1} %)
Moonrise         {:02}:{:02}
Moonset          {:02}:{:02}
New moon       {}
Full moon      {}
"#,
            self.datetime,
            self.lunar_phase,
            self.lunar_illumination,
            self.moonrise.hour,
            self.moonrise.minute,
            self.moonset.hour,
            self.moonset.minute,
            self.next_new_moon,
            self.next_full_moon,
        )
    }
}
