use fasttime::{Date, DateTime};

use crate::{
    D2000, MIDNIGHT,
    solar::moon,
    ui::{
        UpdateCmd,
        components::DEG_SYM,
        views::{DatetimeStatus, UpdateableFromCmd},
    },
};

pub struct State {
    pub(in crate::ui::views) datetime: DatetimeStatus,
    pub(in crate::ui::views) lunar_phase: moon::Phase,
    pub(in crate::ui::views) lunar_illumination: f64,
    pub(in crate::ui::views) next_new_moon: Date,
    pub(in crate::ui::views) next_full_moon: Date,
    pub(in crate::ui::views) moonrise: DateTime,
    pub(in crate::ui::views) moonset: DateTime,
    pub(in crate::ui::views) moonset_azimuth: f64,
    pub(in crate::ui::views) moonrise_azimuth: f64,
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
                moonrise_azim,
                moonset_azim,
            } => {
                self.lunar_phase = lunar_phase;
                self.lunar_illumination = lunar_illumination;
                self.next_full_moon = next_full_moon;
                self.next_new_moon = next_new_moon;
                self.moonrise = moonrise_at;
                self.moonset = moonset_at;
                self.moonrise_azimuth = moonrise_azim;
                self.moonset_azimuth = moonset_azim;
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
            moonrise: DateTime::new(D2000, MIDNIGHT),
            moonset: DateTime::new(D2000, MIDNIGHT),
            moonrise_azimuth: 0.0,
            moonset_azimuth: 0.0,
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            r#"{}
Lunar phase   {}({:>4.1} %)
Rise  ({:>3.0}{DEG_SYM})  {:02}-{:02} {:02}:{:02}
Set   ({:>3.0}{DEG_SYM})  {:02}-{:02} {:02}:{:02}
New moon       {}
Full moon      {}
"#,
            self.datetime,
            self.lunar_phase,
            self.lunar_illumination,
            self.moonrise_azimuth,
            self.moonrise.date.month,
            self.moonrise.date.day,
            self.moonrise.time.hour,
            self.moonrise.time.minute,
            self.moonset_azimuth,
            self.moonset.date.month,
            self.moonset.date.day,
            self.moonset.time.hour,
            self.moonset.time.minute,
            self.next_new_moon,
            self.next_full_moon,
        )
    }
}
