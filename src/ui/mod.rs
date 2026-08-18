use embassy_net::Ipv4Cidr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::{pixelcolor, prelude::*};
use fasttime::{Date, DateTime, Time};
use ssd1306::{Ssd1306Async, prelude::*};

use crate::board::I2cBusDeviceAsync;
use crate::events::NtpStatus;
use crate::solar::moon::{self, Moon};
use crate::solar::sun::{self, Sun};
use crate::ui::views::View;
use crate::{AppError, HorizontalCoordinate, SSD1306};

extern crate alloc;
use alloc::string::String;

mod components;
mod views;

pub(crate) const PRIMITIVE_STYLE_DEFAULT: PrimitiveStyle<pixelcolor::BinaryColor> =
    PrimitiveStyle::with_stroke(pixelcolor::BinaryColor::On, 1);

pub struct Ui<DI> {
    disp: SSD1306<DI>,
    views: [View; 4],
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
            View::Position(Default::default()),
            View::Moon(Default::default()),
            View::Sun(Default::default()),
            View::Status(Default::default()),
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

static UPDATE_CMD_CHAN: channel::Channel<CriticalSectionRawMutex, UpdateCmd, 3> =
    channel::Channel::new();

#[derive(Clone)]
pub enum UpdateCmd {
    SetDatetime {
        datetime: DateTime,
        lst: (u8, u8, u8),
        last_ntp_status: NtpStatus,
    },
    SetLunar {
        lunar_phase: moon::Phase,
        lunar_illumination: f64,
        next_new_moon: Date,
        next_full_moon: Date,
        moonrise_at: DateTime,
        moonset_at: DateTime,
        moonrise_azim: f64,
        moonset_azim: f64,
    },
    SetSolar {
        day_progress: sun::DayProgress,
        sunrise_at: Time,
        sunset_at: Time,
        sundawn_at: Time,
        sundusk_at: Time,
        sunrise_azim: f64,
        sunset_azim: f64,
    },
    SetPosition {
        sun_pos: HorizontalCoordinate,
        moon_pos: HorizontalCoordinate,
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

    /// push new datetime, sun and moon state to UI
    pub async fn notify_new_solar_state(datetime: DateTime, sun: &Sun, moon: &Moon) {
        // update moon info view
        (UpdateCmd::SetLunar {
            lunar_phase: moon.phase(),
            lunar_illumination: moon.illumination(),
            next_new_moon: moon.upcoming_new_moon().date,
            next_full_moon: moon.upcoming_full_moon().date,
            moonrise_at: moon.rise_at(),
            moonset_at: moon.set_at(),
            moonrise_azim: moon.rise_azimuth(),
            moonset_azim: moon.set_azimuth(),
        })
        .notify()
        .await;

        // update sun info view
        (UpdateCmd::SetSolar {
            day_progress: sun.day_progress(&datetime.time),
            sunrise_at: sun.rise_at(),
            sunset_at: sun.set_at(),
            sundusk_at: sun.dusk_at(),
            sundawn_at: sun.dawn_at(),
            sunrise_azim: sun.rise_azimuth(),
            sunset_azim: sun.set_azimuth(),
        })
        .notify()
        .await;

        // update sun and moon position view
        (UpdateCmd::SetPosition {
            sun_pos: sun.pos(),
            moon_pos: moon.pos(),
        })
        .notify()
        .await;
    }

    /// push new datetime, LST, last NTP status to UI
    pub async fn notify_new_datetime(
        datetime: DateTime,
        lst: (u8, u8, u8),
        last_ntp_status: NtpStatus,
    ) {
        (UpdateCmd::SetDatetime {
            datetime,
            lst,
            last_ntp_status,
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
