use crate::{
    AstronDatetimeExt, HorizontalCoordinate,
    solar::SolarObject,
    ui::{
        UpdateCmd,
        components::DEG_SYM,
        views::{DatetimeStatus, UpdateableFromCmd},
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
                lst,
                last_ntp_status,
            } => self.datetime.update(datetime, lst, last_ntp_status),

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
        write!(
            f,
            r#"JD {:.2}
LT   {:02}:{:02}:{:02}
LST  {:02}:{:02}:{:02}
Sun pos.
 Az  {:>6.2}{DEG_SYM} 
 Alt {:>6.2}{DEG_SYM}
Moon pos.
 Az  {:>6.2}{DEG_SYM} 
 Alt {:>6.2}{DEG_SYM}
  "#,
            self.datetime.datetime.to_julian(),
            self.datetime.datetime.time.hour,
            self.datetime.datetime.time.minute,
            self.datetime.datetime.time.second,
            self.datetime.lst.0,
            self.datetime.lst.1,
            self.datetime.lst.2,
            self.sun_pos.azimuth,
            self.sun_pos.altitude,
            self.moon_pos.azimuth,
            self.moon_pos.altitude
        )
    }
}
