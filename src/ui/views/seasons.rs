use crate::datetime::{AstronDatetimeExt, UtOffsetExt};
use crate::ui::views::TextBasedView;
use crate::ui::{
    UpdateCmd,
    views::{DatetimeStatus, UpdateableFromCmd},
};
use time::PlainDateTime;

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
        let spring = PlainDateTime::from_julian(self.spring_jd);
        let summer = PlainDateTime::from_julian(self.summer_jd);
        let autumn = PlainDateTime::from_julian(self.autumn_jd);
        let winter = PlainDateTime::from_julian(self.winter_jd);

        // apply extra DST offset if any
        let spring = spring.saturating_add(spring.dst_offset_duration());
        let summer = summer.saturating_add(summer.dst_offset_duration());
        let autumn = autumn.saturating_add(autumn.dst_offset_duration());
        let winter = winter.saturating_add(winter.dst_offset_duration());

        write!(
            f,
            r#"{}
Spring   {} {:02}:{:02}
Summer   {} {:02}:{:02}
Autumn   {} {:02}:{:02}
Winter   {} {:02}:{:02}
"#,
            self.datetime,
            spring.date(),
            spring.hour(),
            spring.minute(),
            summer.date(),
            summer.hour(),
            summer.minute(),
            autumn.date(),
            autumn.hour(),
            autumn.minute(),
            winter.date(),
            winter.hour(),
            winter.minute(),
        )
    }
}

impl TextBasedView for State {}
