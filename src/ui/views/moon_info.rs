use fasttime::Date;

use crate::{
    D2000,
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
    pub(in crate::ui::views) next_first_quarter_moon: Date,
    pub(in crate::ui::views) next_last_quarter_moon: Date,
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
                next_first_quarter,
                next_last_quarter,
            } => {
                self.lunar_phase = lunar_phase;
                self.lunar_illumination = lunar_illumination;
                self.next_full_moon = next_full_moon;
                self.next_new_moon = next_new_moon;
                self.next_first_quarter_moon = next_first_quarter;
                self.next_last_quarter_moon = next_last_quarter;
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
            next_first_quarter_moon: D2000,
            next_last_quarter_moon: D2000,
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            r#"{}
Lunar phase   {}({:>4.1} %)
New moon       {}
First quarter  {}
Full moon      {}
Last quarter   {}
"#,
            self.datetime,
            self.lunar_phase,
            self.lunar_illumination,
            self.next_new_moon,
            self.next_first_quarter_moon,
            self.next_full_moon,
            self.next_last_quarter_moon
        )
    }
}
