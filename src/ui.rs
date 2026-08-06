use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder, ascii};
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::{pixelcolor, prelude::Drawable, prelude::Point};
use fasttime::{Date, DateTime, Time};
use ssd1306::{Ssd1306Async, prelude::*};

use crate::events::NtpStatus;
use crate::moon::{self, Moon};
use crate::sun::{DayProgress, Sun};
use crate::{AppError, AstronDatetimeExt, HorizontalCoordinate, SSD1306};

extern crate alloc;
use alloc::format;

#[derive(Clone)]
pub struct CommonState {
    datetime: DateTime,
    last_ntp_status: NtpStatus,
    lst: (u8, u8, u8),
}

impl CommonState {
    pub fn update(&mut self, datetime: DateTime, lst: (u8, u8, u8), last_ntp_status: NtpStatus) {
        self.last_ntp_status = last_ntp_status;
        self.lst = lst;
        self.datetime = datetime;
    }
}

impl Default for CommonState {
    fn default() -> Self {
        Self {
            lst: (0, 0, 0),
            last_ntp_status: Default::default(),
            datetime: DateTime::from_unix_timestamp(0, 0).unwrap(),
        }
    }
}

impl core::fmt::Display for CommonState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let t = if let NtpStatus::OK = self.last_ntp_status {
            format!(
                "{:02}:{:02}:{:02}",
                self.datetime.time.hour, self.datetime.time.minute, self.datetime.time.second
            )
        } else {
            format!("NTP {}", self.last_ntp_status)
        };

        let lst = if let NtpStatus::OK = self.last_ntp_status {
            format!("{:02}:{:02}:{:02}", self.lst.0, self.lst.1, self.lst.2)
        } else {
            format!("NTP {}", self.last_ntp_status)
        };

        write!(
            f,
            r#"JD {:.2}  {}
Civil            {}
Sidereal         {}"#,
            self.datetime.to_julian(),
            self.datetime.date,
            t,
            lst
        )
    }
}

#[derive(Clone)]
pub enum View {
    Lunar {
        now: CommonState,
        lunar_phase: moon::Phase,
        lunar_illumination: f64,
        next_new_moon: Date,
        next_full_moon: Date,
        next_first_quarter_moon: Date,
        next_last_quarter_moon: Date,
    },
    Solar {
        now: CommonState,
        day_progress: DayProgress,
        sunrise_at: Time,
        sunset_at: Time,
        sun_pos: HorizontalCoordinate,
    },
}

impl View {
    fn new_lunar_view() -> Self {
        Self::Lunar {
            lunar_phase: moon::Phase::New,
            lunar_illumination: 0.0,
            now: Default::default(),
            next_new_moon: Date::from_ymd_unchecked(1970, 1, 1),
            next_full_moon: Date::from_ymd_unchecked(1970, 1, 1),
            next_first_quarter_moon: Date::from_ymd_unchecked(1970, 1, 1),
            next_last_quarter_moon: Date::from_ymd_unchecked(1970, 1, 1),
        }
    }

    fn new_solar_view() -> Self {
        Self::Solar {
            now: Default::default(),
            day_progress: Default::default(),
            sun_pos: Default::default(),
            sunrise_at: Time::from_seconds_nanos(0, 0).unwrap(),
            sunset_at: Time::from_seconds_nanos(0, 0).unwrap(),
        }
    }
}

impl core::fmt::Display for View {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            View::Lunar {
                now,
                lunar_illumination,
                lunar_phase,
                next_new_moon,
                next_full_moon,
                next_first_quarter_moon,
                next_last_quarter_moon,
            } => {
                write!(
                    f,
                    r#"{}
Lunar phase  {}({:.1} %)
New moon       {}
First quarter  {}
Full moon      {}
Last quarter   {}
"#,
                    now,
                    lunar_phase,
                    lunar_illumination,
                    next_new_moon,
                    next_first_quarter_moon,
                    next_full_moon,
                    next_last_quarter_moon
                )
            }
            View::Solar {
                now,
                day_progress,
                sun_pos,
                sunrise_at,
                sunset_at,
            } => {
                write!(
                    f,
                    r#"{}
Sunrise        {}
Sunset         {}
Solar prog.    {}
  Azimuth     {:>6.2} deg
  Altitude    {:>6.2} deg 
"#,
                    now, sunrise_at, sunset_at, day_progress, sun_pos.azimuth, sun_pos.altitude
                )
            }
        }
    }
}

impl View {
    fn update(
        &mut self,
        sun: &Sun,
        moon: &Moon,
        now_local: DateTime,
        lst_now: (u8, u8, u8),
        ntp_status: NtpStatus,
    ) {
        match self {
            View::Lunar {
                now,
                lunar_phase,
                lunar_illumination,
                next_new_moon,
                next_full_moon,
                next_first_quarter_moon,
                next_last_quarter_moon,
            } => {
                // lunar phase
                *lunar_illumination = moon.illumination();
                *lunar_phase = moon.phase();
                *next_full_moon = moon.upcoming_full_moon().date;
                *next_new_moon = moon.upcoming_new_moon().date;
                *next_first_quarter_moon = moon.upcoming_first_quarter().date;
                *next_last_quarter_moon = moon.upcoming_last_quarter().date;

                // time now
                now.update(now_local, lst_now, ntp_status);
            }
            View::Solar {
                now,
                day_progress,
                sun_pos,
                sunrise_at,
                sunset_at,
            } => {
                // sunrise/sunset status
                let (sunrise, sunset) = (sun.rise_at(), sun.set_at());
                *sunrise_at = *sunrise;
                *sunset_at = *sunset;

                // sun position
                *sun_pos = *sun.pos();
                *day_progress = sun.day_progress(&now.datetime.time);

                // time now
                now.update(now_local, lst_now, ntp_status);
            }
        }
    }
}

pub struct Ui<'a, DI> {
    disp: SSD1306<DI>,
    views: [View; 2],
    view_looper: Circulator,
    text_style: MonoTextStyle<'a, pixelcolor::BinaryColor>,
}

// TODO: event loop
impl<'a, DI> Ui<'a, DI>
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

        let views = [View::new_solar_view(), View::new_lunar_view()];
        let view_circulator = Circulator::new(views.len(), 5);

        let text_style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_5X8)
            .text_color(pixelcolor::BinaryColor::On)
            .build();

        Self {
            disp,
            views,
            view_looper: view_circulator,
            text_style,
        }
    }

    pub async fn initialise(mut self) -> Self {
        self.disp.init().await.unwrap_or_default();
        self
    }

    #[inline]
    pub fn update_state(
        &mut self,
        sun: &Sun,
        moon: &Moon,
        now: DateTime,
        lst: (u8, u8, u8),
        last_ntp_status: NtpStatus,
    ) {
        let cur_view_idx = self.view_looper.next().unwrap();
        if let Some(view) = self.views.get_mut(cur_view_idx) {
            view.update(sun, moon, now, lst, last_ntp_status);
        }
    }

    pub fn draw(&mut self) -> Result<(), AppError> {
        self.disp.clear_buffer();

        if let Some(view) = self.views.get(self.view_looper.peek()) {
            Text::with_baseline(
                &format!("{}", view),
                Point::new(0, 0),
                self.text_style,
                Baseline::Top,
            )
            .draw(&mut self.disp)
            .map_err(|_| AppError::DrawError)?;
        }
        Ok(())
    }

    pub async fn flush(&mut self) {
        self.disp.flush().await.unwrap();
    }
}

struct Circulator {
    cur_idx: usize,
    count: u8,
    period: u8,
    end: usize,
}

impl Circulator {
    pub fn new(end: usize, period: u8) -> Self {
        Self {
            cur_idx: 0,
            end,
            count: 0,
            period,
        }
    }

    pub fn peek(&self) -> usize {
        self.cur_idx
    }
}

impl Iterator for Circulator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count > self.period {
            self.cur_idx = (self.cur_idx + 1) % self.end;
            self.count = 0;
        }

        self.count += 1;
        Some(self.cur_idx)
    }
}
