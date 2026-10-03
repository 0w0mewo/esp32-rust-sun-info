use embassy_net::Ipv4Cidr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::{pixelcolor, prelude::*};
use fasttime::{Date, DateTime, OffsetDateTime, Time};
use ssd1306::{Ssd1306Async, prelude::*};

use crate::board::I2cBusDeviceAsync;
use crate::datetime::{AstronDatetimeExt, UNIX_EPOCH, UtOffsetExt};
use crate::events::NtpStatus;
use crate::solar::moon::{self, Moon};
use crate::solar::sun::{
    self, NORTH_HEMISPHERE_ASTRON_SEASON_TRANSIT, SOUTH_HEMISPHERE_ASTRON_SEASON_TRANSIT, Sun,
};
use crate::solar::{PlanetUpdater, SolarObject};
use crate::ui::views::View;
use crate::{AppError, HorizontalCoordinate, SECONDS_PER_DAY, SSD1306};

extern crate alloc;
use alloc::string::String;

mod components;
mod views;

pub(crate) const PRIMITIVE_STYLE_DEFAULT: PrimitiveStyle<pixelcolor::BinaryColor> =
    PrimitiveStyle::with_stroke(pixelcolor::BinaryColor::On, 1);

pub struct Ui<DI> {
    disp: SSD1306<DI>,
    views: [View; 5],
    view_looper: Circulator,
}

impl<DI> Ui<DI>
where
    DI: display_interface::AsyncWriteOnlyDataCommand,
{
    pub fn new(disp_intf: DI) -> Self {
        let disp = Ssd1306Async::new(
            disp_intf,
            ssd1306::size::DisplaySize128x64,
            ssd1306::rotation::DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        let views = [
            View::Status(Default::default()),
            View::Position(Default::default()),
            View::Moon(Default::default()),
            View::Sun(Default::default()),
            View::Seasons(Default::default()),
        ];
        let view_circulator = Circulator::new(views.len());

        Self {
            disp,
            views,
            view_looper: view_circulator,
        }
    }

    pub async fn initialise(mut self) -> Self {
        self.disp.init().await.unwrap_or_default();
        self
    }

    fn draw(&mut self) -> Result<(), display_interface::DisplayError> {
        self.disp.clear_buffer();

        let cur_view_idx = self.view_looper.peek();
        if let Some(view) = self.views.get(cur_view_idx) {
            view.draw(&mut self.disp)?;
        }

        Ok(())
    }

    fn update_views(&mut self, cmd: &UpdateCmd) {
        if let UpdateCmd::SwitchView = cmd {
            self.view_looper.next();
        }

        self.views.iter_mut().for_each(|v| v.update(cmd));
    }

    async fn flush(&mut self) -> Result<(), AppError> {
        self.draw().map_err(|_| AppError::DrawError)?;
        self.disp.flush().await.map_err(|_| AppError::DrawError)?;

        Ok(())
    }
}

#[embassy_executor::task]
pub async fn ui_flush_task(mut ui: Ui<I2CInterface<I2cBusDeviceAsync<'static>>>) {
    loop {
        let cmd = UPDATE_CMD_CHAN.receive().await;
        ui.update_views(&cmd);

        if let UpdateCmd::Draw = cmd {
            ui.flush().await.unwrap_or_default();
        }
    }
}

struct Circulator {
    cur_idx: usize,
    end: usize,
}

impl Circulator {
    pub fn new(end: usize) -> Self {
        Self { cur_idx: 0, end }
    }

    pub fn peek(&self) -> usize {
        self.cur_idx
    }
}

impl Iterator for Circulator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.cur_idx = (self.cur_idx + 1) % self.end;

        Some(self.cur_idx)
    }
}

/// convert to polar coordinate
pub fn polar(center: Point, angle: f64, radius: f64) -> Point {
    let (angle_sin, angle_cos) = libm::sincos(angle.to_radians());

    let (x, y) = (radius * angle_sin, -radius * angle_cos);
    center + Point::new(libm::round(x) as i32, libm::round(y) as i32)
}

static UPDATE_CMD_CHAN: channel::Channel<CriticalSectionRawMutex, UpdateCmd, 5> =
    channel::Channel::new();

#[derive(Clone)]
pub enum UpdateCmd {
    SetDatetime {
        datetime: OffsetDateTime,
        last_ntp_status: NtpStatus,
    },
    SetLunar {
        lunar_phase: moon::Phase,
        lunar_illumination: f64,
        next_new_moon: Date,
        next_full_moon: Date,
    },
    SetSolar {
        day_progress: sun::DayProgress,
        sundawn_at: Time,
        sundusk_at: Time,
    },
    SetRiseSet {
        rise_at: Option<DateTime>,
        set_at: Option<DateTime>,
        rise_azim: f64,
        set_azim: f64,
        obj: SolarObject,
    },
    SetPosition {
        obj: SolarObject,
        pos: HorizontalCoordinate,
    },
    SetEquinoxSolstice {
        spring_jd: f64,
        summer_jd: f64,
        autumn_jd: f64,
        winter_jd: f64,
    },
    SetIpStatus(Ipv4Cidr),
    SetApStatus(String),
    Draw,
    SwitchView,
}

impl UpdateCmd {
    pub async fn notify(self) {
        UPDATE_CMD_CHAN.send(self).await
    }

    pub async fn notify_new_ip_address(ip_addr: Ipv4Cidr) {
        UpdateCmd::SetIpStatus(ip_addr).notify().await
    }

    pub async fn notifiy_new_ap_name(connected_ap: &str) {
        UpdateCmd::SetApStatus(connected_ap.into()).notify().await
    }

    pub async fn notifiy_new_lunar_state(datetime: &OffsetDateTime, moon: &Moon) {
        let next_new_moon_utc = moon.upcoming_new_moon();
        let next_full_moon_utc = moon.upcoming_full_moon();

        // apply timezone standard offset and DST
        let tz_offset = Some(&datetime.offset);
        let next_new_moon = next_new_moon_utc.add_ut_offset(tz_offset).date;
        let next_full_moon = next_full_moon_utc.add_ut_offset(tz_offset).date;

        // update moon info view
        (UpdateCmd::SetLunar {
            lunar_phase: moon.phase(),
            lunar_illumination: moon.illumination(),
            next_new_moon,
            next_full_moon,
        })
        .notify()
        .await;
    }

    /// push new datetime, sun and moon state to UI
    pub async fn notify_new_solar_state(datetime: &OffsetDateTime, sun: &Sun) {
        // in UTC
        let sundusk_at = sun.dusk_at().unwrap_or(UNIX_EPOCH);
        let sundawn_at = sun.dawn_at().unwrap_or(UNIX_EPOCH);

        // apply timezone standard offset and DST
        let tz_offset = Some(&datetime.offset);
        let sundusk_at = sundusk_at.add_ut_offset(tz_offset).time;
        let sundawn_at = sundawn_at.add_ut_offset(tz_offset).time;

        // update sun info view
        (UpdateCmd::SetSolar {
            day_progress: sun.day_progress(&datetime.utc.time),
            sundusk_at,
            sundawn_at,
        })
        .notify()
        .await;
    }

    /// push new rise/set event infos and current position of a planet/sun/moon
    pub async fn notify_new_object_state<PLANET: PlanetUpdater>(
        datetime: &OffsetDateTime,
        planet: &PLANET,
    ) {
        // convert rise/set to local time
        let tz_offset = Some(&datetime.offset);
        let rise_at = planet
            .rise_at()
            .map(|jd| DateTime::from_julian(jd).add_ut_offset(tz_offset));
        let set_at = planet
            .set_at()
            .map(|jd| DateTime::from_julian(jd).add_ut_offset(tz_offset));

        // update rise set
        (UpdateCmd::SetRiseSet {
            rise_at,
            set_at,
            rise_azim: planet.rise_azimuth(),
            set_azim: planet.set_azimuth(),
            obj: planet.planet(),
        })
        .notify()
        .await;

        // update position view
        (UpdateCmd::SetPosition {
            pos: planet.pos(),
            obj: planet.planet(),
        })
        .notify()
        .await;
    }

    /// push new local datetime, last NTP status to UI
    pub async fn notify_new_datetime(datetime: &OffsetDateTime, last_ntp_status: NtpStatus) {
        let datetime = datetime.add_ut_offset(None);
        (UpdateCmd::SetDatetime {
            datetime,
            last_ntp_status,
        })
        .notify()
        .await;
    }

    pub async fn update_season_start(datetime: &OffsetDateTime, lat: f64) {
        let year = datetime.utc.date.year as f64;
        let tz_days_offset = datetime.offset.as_seconds() as f64 / SECONDS_PER_DAY;

        // seasons starting in local datetime without DST encounted
        let local_seasons = if lat >= 0.0 {
            NORTH_HEMISPHERE_ASTRON_SEASON_TRANSIT
        } else {
            SOUTH_HEMISPHERE_ASTRON_SEASON_TRANSIT
        };
        let local_seasons = local_seasons.map(|s| s.equinox_solstice_jd(year) + tz_days_offset);

        (UpdateCmd::SetEquinoxSolstice {
            spring_jd: local_seasons[0],
            summer_jd: local_seasons[1],
            autumn_jd: local_seasons[2],
            winter_jd: local_seasons[3],
        })
        .notify()
        .await;
    }

    /// redraw screen
    pub async fn redraw() {
        UpdateCmd::Draw.notify().await;
    }

    pub async fn next_view() {
        UpdateCmd::SwitchView.notify().await;
    }
}

pub trait UpdateableFromCmd {
    fn update(&mut self, cmd: &UpdateCmd);
}
