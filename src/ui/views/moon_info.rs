use fasttime::{Date, DateTime};
extern crate alloc;
use alloc::format;

use crate::{
    D2000,
    solar::{SolarObject, moon},
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
    pub(in crate::ui::views) moonrise: Option<DateTime>,
    pub(in crate::ui::views) moonset: Option<DateTime>,
    pub(in crate::ui::views) moonset_azimuth: f64,
    pub(in crate::ui::views) moonrise_azimuth: f64,
}

impl UpdateableFromCmd for State {
    fn update(&mut self, cmd: &UpdateCmd) {
        match *cmd {
            UpdateCmd::SetDatetime {
                datetime,
                last_ntp_status,
            } => self.datetime.update(datetime, last_ntp_status),

            UpdateCmd::SetMoonRiseSet { rise_at, set_at } => {
                self.moonrise = rise_at;
                self.moonset = set_at;
            }

            UpdateCmd::SetRiseSetDirection {
                obj,
                rise_azim,
                set_azim,
            } => {
                if let SolarObject::Moon = obj {
                    self.moonrise_azimuth = rise_azim;
                    self.moonset_azimuth = set_azim;
                }
            }

            UpdateCmd::SetLunar {
                lunar_phase,
                lunar_illumination,
                next_new_moon,
                next_full_moon,
            } => {
                self.lunar_phase = lunar_phase;
                self.lunar_illumination = lunar_illumination;
                self.next_full_moon = next_full_moon;
                self.next_new_moon = next_new_moon;
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
            moonrise: None,
            moonset: None,
            moonrise_azimuth: 0.0,
            moonset_azimuth: 0.0,
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let rise = self.moonrise.map_or(">23:59".into(), |rise| {
            format!(
                "({:>3.0}{DEG_SYM})  {:02}-{:02} {:02}:{:02}",
                self.moonrise_azimuth,
                rise.date.month,
                rise.date.day,
                rise.time.hour,
                rise.time.minute,
            )
        });
        let set = self.moonset.map_or(">23:59".into(), |set| {
            format!(
                "({:>3.0}{DEG_SYM})  {:02}-{:02} {:02}:{:02}",
                self.moonset_azimuth, set.date.month, set.date.day, set.time.hour, set.time.minute,
            )
        });

        write!(
            f,
            r#"{}
Lunar phase   {}({:>4.1} %)
Rise  {} 
Set   {}
New moon       {}
Full moon      {}
"#,
            self.datetime,
            self.lunar_phase,
            self.lunar_illumination,
            rise,
            set,
            self.next_new_moon,
            self.next_full_moon,
        )
    }
}
