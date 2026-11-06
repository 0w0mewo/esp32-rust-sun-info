#![no_std]

pub mod board;
pub mod config;
pub mod datetime;
pub mod events;
pub mod math;
pub mod ntp;
pub mod solar;
pub mod ui;

pub type SSD1306<DI> = ssd1306::Ssd1306Async<
    DI,
    ssd1306::size::DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsModeAsync<ssd1306::size::DisplaySize128x64>,
>;

use esp_hal::rng;

use crate::datetime::DAYS_PER_JULIAN_CENTURY;

pub const MICROSECS_PER_SEC: u64 = 1_000_000;
const SECONDS_PER_DAY: f64 = 24.0 * 3600.0;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("fail to connect to wifi")]
    WifiLinkTimeout,
    #[error("fail to obtain IP address")]
    IpAddrTimeout,
    #[error("draw error")]
    DrawError,
    #[error("network error")]
    NetworkError,
}

pub fn rand_u64() -> u64 {
    let rand = rng::Rng::new();
    rand.random() as u64 | ((rand.random() as u64) << 32)
}
