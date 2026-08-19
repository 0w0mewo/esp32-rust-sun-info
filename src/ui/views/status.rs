use core::str::FromStr;

use embassy_net::Ipv4Cidr;
use embassy_time::Instant;

use crate::ui::{UpdateCmd, UpdateableFromCmd, views::DatetimeStatus};
extern crate alloc;
use alloc::string::String;

pub struct State {
    pub(in crate::ui::views) datetime: DatetimeStatus,
    pub(in crate::ui::views) ip_addr: Ipv4Cidr,
    pub(in crate::ui::views) connected_ap: String,
    pub(in crate::ui::views) booted: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            datetime: Default::default(),
            ip_addr: Ipv4Cidr::from_str("0.0.0.0/0").unwrap(),
            connected_ap: "None".into(),
            booted: false,
        }
    }
}

impl UpdateableFromCmd for State {
    fn update(&mut self, cmd: &UpdateCmd) {
        match cmd {
            &UpdateCmd::SetDatetime {
                datetime,
                last_ntp_status,
                ..
            } => {
                self.datetime.update(datetime, (0, 0, 0), last_ntp_status);

                // when the everything is ready, it begins with setting up datetime.
                self.booted = true;
            }

            &UpdateCmd::SetIpStatus(ip_address) => {
                self.ip_addr = ip_address;
            }

            UpdateCmd::SetApStatus(ap_name) => self.connected_ap = ap_name.clone(),

            _ => (),
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.booted {
            write!(f, "Starting up...")
        } else {
            let datetime = self.datetime.datetime;
            let uptime = Instant::now();
            write!(
                f,
                r#"{}       {:02}:{:02}:{:02}
Uptime {} s
AP 
  {}
IPv4 
  {}
NTP   [{}]
"#,
                datetime.date,
                datetime.time.hour,
                datetime.time.minute,
                datetime.time.second,
                uptime.as_secs(),
                self.connected_ap,
                self.ip_addr,
                self.datetime.last_ntp_status,
            )
        }
    }
}
