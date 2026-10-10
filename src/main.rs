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
use embassy_time::Ticker;
use embassy_time::Timer;
use esp_hal::gpio;
use esp32_sun_info as lib;
use lib::config::*;

use embassy_time::Duration;
use esp_backtrace as _;
use esp_hal::system;
use esp_println::println;
use lib::MICROSECS_PER_SEC;
use lib::board::{Board, InputType};
use lib::datetime::MORNING;
use lib::datetime::UtOffsetExt;
use lib::events::NtpStatus;
use lib::solar::PlanetUpdater;
use lib::solar::moon::Moon;
use lib::solar::sun::Sun;
use lib::ui::Ui;
use lib::ui::ui_flush_task;
use lib::ui::{self};
use time::SignedDuration;
use time::Time;
use time::UtcDateTime;

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // initialise board perhiphal resources
    let mut board = Board::new(&spawner).await;

    // create an compatible embedded_hal_async::i2c::I2c instance because the ssd1306 driver needs it
    let i2c_dev_ssd1306 = shared_bus::asynch::i2c::I2cDevice::new(board.i2c0_bus);

    // initialise UI, it must run after embassy initialised because it requires await
    let ui = Ui::new(ssd1306::I2CDisplayInterface::new(i2c_dev_ssd1306))
        .initialise()
        .await;
    spawner.spawn(ui_flush_task(ui).unwrap());

    // switch UI views by button
    spawner.spawn(switch_view(board.button.clone()).unwrap());

    // reflush the screen while UI is ready
    ui::UpdateCmd::redraw().await;

    board.wait_for_network().await.unwrap_or_else(|e| {
        println!("Startup failed: {}, reseting..", e);
        system::software_reset();
    });

    // sun and moon calc
    let mut sun = Sun::default();
    let mut moon = Moon::default();
    let (lat, lon) = (LAT, LON);
    let mut last_ntp_status = NtpStatus::default();
    let mut first_run = true;

    // ticker to reduce unnecessary computation because astronomical events
    // do not change in a short time, (update every 35 minutes)
    let mut astron_update_ticker = Ticker::every(Duration::from_secs(35 * 60));

    // the default UI view is the status page, switch to other view here after everything is ready
    ui::UpdateCmd::next_view().await;

    // update astronomical events
    let astron_update = async |sun: &mut Sun, moon: &mut Moon, now: &UtcDateTime| {
        // local time with DST offset
        let local_now = now.to_offset(now.dst_offset());

        // local midnight in UTC
        let midnight = local_now.replace_time(Time::MIDNIGHT).to_utc();
        sun.update_riseset(&midnight, lat, lon);

        // local midnight of tonight in UTC
        let midnight_tonight = {
            let sunrise_time = sun
                .rise()
                .map(|event_info| event_info.event_datetime_local().time())
                .unwrap_or(MORNING);

            if local_now.time() >= sunrise_time {
                // forward to 00:00 of next day
                &local_now
                    .saturating_add(SignedDuration::hours(24 - sunrise_time.hour() as i64))
                    .replace_time(Time::MIDNIGHT)
                    .to_utc()
            } else {
                &midnight
            }
        };
        moon.update_riseset(midnight_tonight, lat, lon);
        moon.update_phase(now);

        // notify UI update
        ui::UpdateCmd::notify_new_solar_state(now, sun).await;
        ui::UpdateCmd::notify_new_lunar_state(moon).await;

        // update seasons
        ui::UpdateCmd::notify_season_start(local_now.year(), lat).await;
    };

    loop {
        let rtc_now = board.rtc.current_time_us();

        if let Ok(utc_now) =
            UtcDateTime::from_unix_timestamp(rtc_now.div_euclid(MICROSECS_PER_SEC) as i64)
        {
            if let Some(new_ntp_status) = NtpStatus::last() {
                last_ntp_status = new_ntp_status;
            }

            if let NtpStatus::OK = last_ntp_status
                && first_run
            {
                // make sure the moon and sun are updated at the first NTP synced
                astron_update(&mut sun, &mut moon, &utc_now).await;

                first_run = false;
            }

            // wait for update tick
            match embassy_futures::select::select(
                astron_update_ticker.next(),
                Timer::after_secs(UPDATE_SEC),
            )
            .await
            {
                // infrequently update sun and moon atronomical events
                embassy_futures::select::Either::First(_) => {
                    astron_update(&mut sun, &mut moon, &utc_now).await;
                }
                // frequently update sun and moon position
                embassy_futures::select::Either::Second(_) => {
                    sun.update_pos(&utc_now, lat, lon);
                    moon.update_pos(&utc_now, lat, lon);
                }
            }

            // update datetime status bar
            ui::UpdateCmd::notify_new_datetime(&utc_now, last_ntp_status).await;

            // update rise/set and current position
            ui::UpdateCmd::notify_new_object_state(&sun).await;
            ui::UpdateCmd::notify_new_object_state(&moon).await;

            // flush display
            ui::UpdateCmd::redraw().await;

            // RGB LED color as sun color
            // LED brightness as day progress
            let day_prog = sun.day_progress(&utc_now);
            let brigtness = day_prog.to_pwm_duty_cycle_percent().max(5);
            let sun_color = sun.color_at(day_prog);
            board.set_rgb_led_color(sun_color, brigtness).await;
        }
    }
}

#[embassy_executor::task]
async fn switch_view(button: Rc<InputType<'static>>) {
    loop {
        // waiting for button pressed
        {
            let mut btn = button.lock().await;
            wait_debounced_button(&mut btn).await;
        }

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
