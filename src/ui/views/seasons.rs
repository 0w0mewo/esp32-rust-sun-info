use crate::AstronDatetimeExt;
use crate::ui::{
    UpdateCmd,
    views::{DatetimeStatus, UpdateableFromCmd},
};
use fasttime::DateTime;

#[derive(Default)]
pub struct State {
    pub(in crate::ui::views) datetime: DatetimeStatus,
    pub(in crate::ui::views) spring_jd: f64,
    pub(in crate::ui::views) summer_jd: f64,
    pub(in crate::ui::views) autumn_jd: f64,
    pub(in crate::ui::views) winter_jd: f64,
}

impl UpdateableFromCmd for State {
    fn update(&mut self, cmd: &UpdateCmd) {
        match *cmd {
            UpdateCmd::SetDatetime {
                datetime,
                last_ntp_status,
            } => self.datetime.update(datetime, last_ntp_status),

            UpdateCmd::SetEquinoxSolstice {
                spring_jd,
                summer_jd,
                autumn_jd,
                winter_jd,
            } => {
                self.spring_jd = spring_jd;
                self.summer_jd = summer_jd;
                self.autumn_jd = autumn_jd;
                self.winter_jd = winter_jd;
            }

            _ => {}
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let spring = DateTime::from_julian(self.spring_jd);
        let summer = DateTime::from_julian(self.summer_jd);
        let autumn = DateTime::from_julian(self.autumn_jd);
        let winter = DateTime::from_julian(self.winter_jd);
        write!(
            f,
            r#"{}
Spring   {} {:02}:{:02}
Summer   {} {:02}:{:02}
Autumn   {} {:02}:{:02}
Winter   {} {:02}:{:02}
"#,
            self.datetime,
            spring.date,
            spring.time.hour,
            spring.time.minute,
            summer.date,
            summer.time.hour,
            summer.time.minute,
            autumn.date,
            autumn.time.hour,
            autumn.time.minute,
            winter.date,
            winter.time.hour,
            winter.time.minute
        )
    }
}
