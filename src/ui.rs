use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::mono_font::{MonoTextStyle, ascii};
use embedded_graphics::primitives::{Circle, Line, PrimitiveStyle, StyledDrawable};
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::{pixelcolor, prelude::*};
use fasttime::{Date, DateTime, Time};
use ssd1306::{Ssd1306Async, prelude::*};

use crate::board::I2cBusDeviceAsync;
use crate::events::NtpStatus;
use crate::solar::moon::Moon;
use crate::solar::sun::Sun;
use crate::solar::{moon, sun};
use crate::{AppError, AstronDatetimeExt, D2000, HorizontalCoordinate, MIDNIGHT, SSD1306};

extern crate alloc;
use alloc::format;
static UPDATE_CMD_CHAN: channel::Channel<CriticalSectionRawMutex, UpdateCmd, 3> =
    channel::Channel::new();

#[derive(Clone)]
enum UpdateCmd {
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
        next_first_quarter: Date,
        next_last_quarter: Date,
    },
    SetSolar {
        day_progress: sun::DayProgress,
        sunrise_at: Time,
        sunset_at: Time,
        sundawn_at: Time,
        sundusk_at: Time,
    },
    SetPosition {
        sun_pos: HorizontalCoordinate,
        moon_pos: HorizontalCoordinate,
    },
    Draw,
}

#[derive(Clone)]
struct DatetimeStatus {
    datetime: DateTime,
    last_ntp_status: NtpStatus,
    lst: (u8, u8, u8),
}

impl DatetimeStatus {
    pub fn update(&mut self, datetime: DateTime, lst: (u8, u8, u8), last_ntp_status: NtpStatus) {
        self.last_ntp_status = last_ntp_status;
        self.lst = lst;
        self.datetime = datetime;
    }
}

impl Default for DatetimeStatus {
    fn default() -> Self {
        Self {
            lst: (0, 0, 0),
            last_ntp_status: Default::default(),
            datetime: DateTime::from_unix_timestamp(0, 0).unwrap(),
        }
    }
}

impl core::fmt::Display for DatetimeStatus {
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

enum View {
    Sun {
        datetime: DatetimeStatus,
        day_progress: sun::DayProgress,
        sunrise_at: Time,
        sunset_at: Time,
        dawn_at: Time,
        dusk_at: Time,
    },
    Moon {
        datetime: DatetimeStatus,
        lunar_phase: moon::Phase,
        lunar_illumination: f64,
        next_new_moon: Date,
        next_full_moon: Date,
        next_first_quarter_moon: Date,
        next_last_quarter_moon: Date,
    },
    Position {
        datetime: DatetimeStatus,
        sun_pos: HorizontalCoordinate,
        moon_pos: HorizontalCoordinate,
    },
}

impl View {
    fn new_sun_info_view() -> Self {
        Self::Sun {
            day_progress: sun::DayProgress::Night,
            sunrise_at: MIDNIGHT,
            sunset_at: MIDNIGHT,
            dawn_at: MIDNIGHT,
            dusk_at: MIDNIGHT,
            datetime: Default::default(),
        }
    }

    fn new_moon_info_view() -> Self {
        Self::Moon {
            datetime: Default::default(),
            lunar_phase: Default::default(),
            lunar_illumination: Default::default(),
            next_new_moon: D2000,
            next_full_moon: D2000,
            next_first_quarter_moon: D2000,
            next_last_quarter_moon: D2000,
        }
    }

    fn new_position_info_view() -> Self {
        Self::Position {
            datetime: Default::default(),
            sun_pos: Default::default(),
            moon_pos: Default::default(),
        }
    }
}

impl core::fmt::Display for View {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            View::Moon {
                datetime,
                lunar_phase,
                lunar_illumination,
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
                    datetime,
                    lunar_phase,
                    lunar_illumination,
                    next_new_moon,
                    next_first_quarter_moon,
                    next_full_moon,
                    next_last_quarter_moon
                )
            }

            View::Sun {
                datetime,
                day_progress,
                sunrise_at,
                sunset_at,
                dawn_at,
                dusk_at,
            } => {
                write!(
                    f,
                    r#"{}
Solar prog.     {}
Dawn            {}
Sunrise         {}
Sunet           {}
Dusk            {} 
"#,
                    datetime, day_progress, dawn_at, sunrise_at, sunset_at, dusk_at,
                )
            }

            View::Position {
                datetime,
                sun_pos,
                moon_pos,
            } => {
                write!(
                    f,
                    r#"JD {:.2}
LT   {:02}:{:02}:{:02}
LST  {:02}:{:02}:{:02}
Sun pos.
 Az  {:>6.2}
 Alt {:>6.2}
Moon pos.
 Az  {:>6.2} 
 Alt {:>6.2} 
  "#,
                    datetime.datetime.to_julian(),
                    datetime.datetime.time.hour,
                    datetime.datetime.time.minute,
                    datetime.datetime.time.second,
                    datetime.lst.0,
                    datetime.lst.1,
                    datetime.lst.2,
                    sun_pos.azimuth,
                    sun_pos.altitude,
                    moon_pos.azimuth,
                    moon_pos.altitude
                )
            }
        }
    }
}

const PRIMITIVE_STYLE_DEFAULT: PrimitiveStyle<pixelcolor::BinaryColor> =
    PrimitiveStyle::with_stroke(pixelcolor::BinaryColor::On, 1);

pub struct Ui<DI> {
    disp: SSD1306<DI>,
    views: [View; 3],
    view_looper: Circulator,
    screen_center: Point,
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
            View::new_position_info_view(),
            View::new_sun_info_view(),
            View::new_moon_info_view(),
        ];
        let view_circulator = Circulator::new(views.len(), 5);
        let screen_center = disp.bounding_box().center();

        Self {
            disp,
            views,
            view_looper: view_circulator,
            screen_center,
        }
    }

    pub async fn initialise(mut self) -> Self {
        self.disp.init().await.unwrap_or_default();
        self
    }

    fn draw(&mut self) -> Result<(), AppError> {
        self.disp.clear_buffer();

        let cur_view_idx = self.view_looper.next().unwrap();
        if let Some(view) = self.views.get(cur_view_idx) {
            if let View::Position {
                sun_pos, moon_pos, ..
            } = view
            {
                let center = self.screen_center + Point::new(32, 0);

                let compass = Compass::new(center, 64);
                compass
                    .draw(&mut self.disp)
                    .map_err(|_| AppError::DrawError)?;

                // sun and moon azimuths, draw while it's above horizon
                let arm_len = 0.5 * compass.diameter as f64 - 9.0;
                if sun_pos.altitude >= 0.0 {
                    PolarLine::with_label(compass.center, sun_pos.azimuth, arm_len, "O")
                        .draw(&mut self.disp)
                        .map_err(|_| AppError::DrawError)?;
                }
                if moon_pos.altitude >= 0.0 {
                    PolarLine::with_label(compass.center, moon_pos.azimuth, arm_len, "L")
                        .draw(&mut self.disp)
                        .map_err(|_| AppError::DrawError)?;
                }
            }

            // status info starts from top left
            CommonStatusTexts::new(Point::zero(), &format!("{}", view))
                .draw(&mut self.disp)
                .map_err(|_| AppError::DrawError)?;
        }

        Ok(())
    }

    fn update_views(&mut self, cmd: &UpdateCmd) {
        self.views.iter_mut().for_each(|view| match *cmd {
            // datetime
            UpdateCmd::SetDatetime {
                datetime,
                lst,
                last_ntp_status,
            } => match view {
                View::Moon { datetime: dt, .. }
                | View::Sun { datetime: dt, .. }
                | View::Position { datetime: dt, .. } => {
                    dt.update(datetime, lst, last_ntp_status);
                }
            },

            // moon infomations
            UpdateCmd::SetLunar {
                lunar_phase: phase,
                lunar_illumination: illumination,
                next_new_moon: new_moon,
                next_full_moon: full_moon,
                next_first_quarter: first_quarter,
                next_last_quarter: last_quarter,
            } => {
                if let View::Moon {
                    lunar_phase,
                    lunar_illumination,
                    next_new_moon,
                    next_full_moon,
                    next_first_quarter_moon,
                    next_last_quarter_moon,
                    ..
                } = view
                {
                    *lunar_phase = phase;
                    *lunar_illumination = illumination;
                    *next_full_moon = full_moon;
                    *next_new_moon = new_moon;
                    *next_first_quarter_moon = first_quarter;
                    *next_last_quarter_moon = last_quarter;
                }
            }

            // sun infomations
            UpdateCmd::SetSolar {
                day_progress: day_prog,
                sunrise_at: rise_at,
                sunset_at: set_at,
                sundawn_at,
                sundusk_at,
            } => {
                if let View::Sun {
                    day_progress,
                    sunrise_at,
                    sunset_at,
                    dawn_at,
                    dusk_at,
                    ..
                } = view
                {
                    *day_progress = day_prog;
                    *sunrise_at = rise_at;
                    *sunset_at = set_at;
                    *dawn_at = sundawn_at;
                    *dusk_at = sundusk_at;
                }
            }

            // sun and moon position
            UpdateCmd::SetPosition { sun_pos, moon_pos } => {
                if let View::Position {
                    sun_pos: s_pos,
                    moon_pos: m_pos,
                    ..
                } = view
                {
                    *s_pos = sun_pos;
                    *m_pos = moon_pos;
                }
            }

            // ignore other commands
            _ => {}
        });
    }

    async fn flush(&mut self) -> Result<(), AppError> {
        self.draw()?;
        self.disp.flush().await.map_err(|_| AppError::DrawError)?;

        Ok(())
    }
}

pub async fn ui_update(
    datetime: DateTime,
    lst: (u8, u8, u8),
    last_ntp_status: NtpStatus,
    moon: &Moon,
    sun: &Sun,
) {
    // update datetime status bar
    UPDATE_CMD_CHAN
        .send(UpdateCmd::SetDatetime {
            datetime,
            lst,
            last_ntp_status,
        })
        .await;

    // update moon info view
    UPDATE_CMD_CHAN
        .send(UpdateCmd::SetLunar {
            lunar_phase: moon.phase(),
            lunar_illumination: moon.illumination(),
            next_new_moon: moon.upcoming_new_moon().date,
            next_full_moon: moon.upcoming_full_moon().date,
            next_first_quarter: moon.upcoming_first_quarter().date,
            next_last_quarter: moon.upcoming_last_quarter().date,
        })
        .await;

    // update sun info view
    UPDATE_CMD_CHAN
        .send(UpdateCmd::SetSolar {
            day_progress: sun.day_progress(&datetime.time),
            sunrise_at: sun.rise_at(),
            sunset_at: sun.set_at(),
            sundusk_at: sun.dusk_at(),
            sundawn_at: sun.dawn_at(),
        })
        .await;

    // update sun and moon position view
    UPDATE_CMD_CHAN
        .send(UpdateCmd::SetPosition {
            sun_pos: sun.pos(),
            moon_pos: moon.pos(),
        })
        .await;

    // redraw screen
    UPDATE_CMD_CHAN.send(UpdateCmd::Draw).await;
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

/// draw line with polar coordinate
struct PolarLine<'a> {
    center: Point,
    angle: f64,
    radius: f64,
    label: Option<&'a str>,
}
impl<'a> PolarLine<'a> {
    fn new(center: Point, angle: f64, radius: f64) -> Self {
        Self {
            center,
            angle,
            radius,
            label: None,
        }
    }

    fn with_label(center: Point, angle: f64, radius: f64, label: &'a str) -> Self {
        let mut p = Self::new(center, angle, radius);
        p.label.replace(label);
        p
    }
}

impl Drawable for PolarLine<'_> {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let p = polar(self.center, self.angle, self.radius);

        // line
        Line::new(self.center, p).draw_styled(&PRIMITIVE_STYLE_DEFAULT, target)?;

        // label
        if let Some(label) = self.label {
            Text::with_baseline(
                label,
                p,
                MonoTextStyle::new(&ascii::FONT_5X7, pixelcolor::BinaryColor::On),
                Baseline::Middle,
            )
            .draw(target)?;
        } else {
            Circle::with_center(p, 2).draw_styled(&PRIMITIVE_STYLE_DEFAULT, target)?;
        }

        Ok(())
    }
}

/// the status text shared by all views
struct CommonStatusTexts<'a> {
    s: &'a str,
    position: Point,
}

impl<'a> CommonStatusTexts<'a> {
    fn new(position: Point, s: &'a str) -> Self {
        Self { s, position }
    }
}

impl Drawable for CommonStatusTexts<'_> {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        // shrink to a smaller font size if the lines exceeded the screen
        let font = if self.s.lines().count() > 8 {
            &ascii::FONT_5X7
        } else {
            &ascii::FONT_5X8
        };

        Text::with_baseline(
            self.s,
            self.position,
            MonoTextStyle::new(font, pixelcolor::BinaryColor::On),
            Baseline::Top,
        )
        .draw(target)?;

        Ok(())
    }
}

/// compass primitive
struct Compass {
    center: Point,
    diameter: u32,
}

impl Compass {
    fn new(center: Point, diameter: u32) -> Self {
        Self { center, diameter }
    }
}

impl Drawable for Compass {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let face = Circle::with_center(self.center, self.diameter);
        face.draw_styled(&PRIMITIVE_STYLE_DEFAULT, target)?;

        for &(angle, angle_txt) in &[(0.0, "N"), (90.0, "E"), (180.0, "S"), (270.0, "W")] {
            let pos = polar(self.center, angle, 0.5 * self.diameter as f64 - 4.0);
            Text::with_baseline(
                angle_txt,
                pos,
                MonoTextStyle::new(&ascii::FONT_4X6, pixelcolor::BinaryColor::On),
                Baseline::Middle,
            )
            .draw(target)?;
        }

        Ok(())
    }
}

/// convert to polar coordinate
fn polar(center: Point, angle: f64, radius: f64) -> Point {
    let (angle_sin, angle_cos) = libm::sincos(angle.to_radians());

    let (x, y) = (radius * angle_sin, -radius * angle_cos);
    center + Point::new(libm::round(x) as i32, libm::round(y) as i32)
}
