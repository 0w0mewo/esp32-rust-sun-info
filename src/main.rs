#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
// #![deny(clippy::large_stack_frames)]

use embassy_embedded_hal::shared_bus;
use embassy_executor::Spawner;
use esp32_sun_info as lib;

use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::system;
use esp_println::println;
use fasttime::{DateTime, OffsetDateTime, UtcOffset};
use lib::AstronDatetimeExt;
use lib::MICROSECS_PER_SEC;
use lib::board::Board;
use lib::events::NtpStatus;
use lib::moon::Moon;
use lib::sun::Sun;
use lib::ui::Ui;
extern crate alloc;

const UPDATE_SEC: u64 = 3;
const LAT: f64 = -33.8651;
const LON: f64 = 151.2099;

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // initialise board perhiphal resources
    let mut board = Board::new(&spawner).await;
    board.init().await.unwrap_or_else(|e| {
        println!("Startup failed: {}, reseting..", e);
        system::software_reset();
    });

    // initialise UI, it must run after embassy initialised because it requires await
    let mut ui = Ui::new(ssd1306::I2CDisplayInterface::new(
        shared_bus::asynch::i2c::I2cDevice::new(board.i2c0_bus),
    ))
    .initialise()
    .await;

    // sunrise calc
    let tz_offset = UtcOffset::from_hours_minutes(true, 10, 0).unwrap();
    let mut sun = Sun::default();
    let mut moon = Moon::default();
    let (lat, lon) = (LAT, LON);
    let mut last_ntp_status = NtpStatus::default();

    loop {
        let rtc_now = board.rtc.current_time_us();

        // update info on display
        if let Ok(utc_now) = DateTime::from_unix_timestamp(
            rtc_now.div_euclid(MICROSECS_PER_SEC) as i64,
            rtc_now.rem_euclid(MICROSECS_PER_SEC) as i32,
        ) {
            let now = OffsetDateTime::from_utc(utc_now, tz_offset);
            let now_local = now.to_local().unwrap();
            let now_sidereal_local = utc_now.to_sidereal_time_hms(lon);

            sun.update(&now, lat, lon);
            moon.update(&utc_now);

            if let Some(new_ntp_status) = NtpStatus::last() {
                last_ntp_status = new_ntp_status;
            }

            // ui update
            ui.update_state(&sun, &moon, now_local, now_sidereal_local, last_ntp_status);
            ui.draw().unwrap();
            ui.flush().await;

            // LED brightness
            let day_percentage = sun
                .day_progress(&now_local.time)
                .to_pwm_duty_cycle_percent();
            board.set_led_brightness(day_percentage);
        }

        Timer::after(Duration::from_secs(UPDATE_SEC)).await;
    }
}
