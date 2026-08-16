#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
// #![deny(clippy::large_stack_frames)]

use alloc::rc::Rc;
use embassy_embedded_hal::shared_bus;
use embassy_executor::Spawner;
use embassy_time::Instant;
use esp_hal::gpio;
use esp32_sun_info as lib;

use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::system;
use esp_println::println;
use fasttime::{DateTime, OffsetDateTime, UtcOffset};
use lib::AstronDatetimeExt;
use lib::MICROSECS_PER_SEC;
use lib::board::{Board, InputType};
use lib::events::NtpStatus;
use lib::solar::PlanetUpdater;
use lib::solar::moon::Moon;
use lib::solar::sun::Sun;
use lib::ui::Ui;
use lib::ui::ui_flush_task;
use lib::ui::{self};
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

    // create an compatible embedded_hal_async::i2c::I2c instance because the ssd1306 driver needs it
    let i2c_dev_ssd1306 = shared_bus::asynch::i2c::I2cDevice::new(board.i2c0_bus);

    // initialise UI, it must run after embassy initialised because it requires await
    let ui = Ui::new(ssd1306::I2CDisplayInterface::new(i2c_dev_ssd1306))
        .initialise()
        .await;
    spawner.spawn(ui_flush_task(ui).unwrap());

    // switch UI views by button
    spawner.spawn(switch_view(board.button.clone()).unwrap());

    // sunrise calc
    let tz_offset = UtcOffset::from_hours_minutes(true, 10, 0).unwrap();
    let mut sun = Sun::default();
    let mut moon = Moon::default();
    let (lat, lon) = (LAT, LON);
    let mut last_ntp_status = NtpStatus::default();

    // unix timestamp for the last astronomical events update
    // Note: set to 0 so that a update always performs at the startup
    let mut last_astron_update = 0;

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

            // frequently update sun and moon position
            sun.update_pos(&now, lat, lon);
            moon.update_pos(&now, lat, lon);

            // reduce unnecessary computation because astronomical events
            // do not change in a short time, (update every 10 minutes)
            if utc_now.unix_timestamp() - last_astron_update > 10 * 60 {
                sun.update_astron(&now, lat, lon);
                moon.update_astron(&now, lat, lon);

                last_astron_update = utc_now.unix_timestamp();
            }

            if let Some(new_ntp_status) = NtpStatus::last() {
                last_ntp_status = new_ntp_status;
            }

            // update datetime status bar
            ui::UpdateCmd::notify_new_datetime(now_local, now_sidereal_local, last_ntp_status)
                .await;

            // update sun and moon states
            ui::UpdateCmd::notify_new_solar_state(now_local, &sun, &moon).await;

            // flush display
            ui::UpdateCmd::redraw().await;

            // RGB LED color as sun color
            // LED brightness as day progress
            let day_percentage = sun
                .day_progress(&now_local.time)
                .to_pwm_duty_cycle_percent();
            let sun_color = sun.color_at(&now_local.time);
            board.set_rgb_led_color(sun_color, day_percentage).await;
        }

        Timer::after(Duration::from_secs(UPDATE_SEC)).await;
    }
}

#[embassy_executor::task]
async fn switch_view(button: Rc<InputType<'static>>) {
    loop {
        // waiting for button pressed
        let mut btn = button.lock().await;
        wait_debounced_button(&mut btn).await;

        // switch view
        ui::UpdateCmd::next_view().await;
    }
}

/// falling edge triggered button
async fn wait_debounced_button<'a>(btn: &mut gpio::Input<'a>) {
    loop {
        let now = Instant::now();

        btn.wait_for_falling_edge().await;
        if now.elapsed() < Duration::from_millis(65) {
            continue;
        }

        // for unknown reason, it also triggered when rising edge and makes
        // the falling edge triggering pointless.
        // Add an extra check on to ensure it was actually triggered by
        // falling edge.
        if btn.is_low() {
            break;
        }
    }
}
