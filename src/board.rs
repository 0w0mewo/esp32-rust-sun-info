use alloc::rc::Rc;
use embassy_embedded_hal::shared_bus;
use embassy_net::StackResources;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::time::Rate;
use esp_hal::{gpio, interrupt::software::SoftwareInterruptControl, rtc_cntl, time, timer::timg};
use esp_hal::{i2c, rmt};
use esp_println::println;
use esp_radio::wifi;
use smart_leds::{RGB8, SmartLedsWriteAsync, brightness, gamma};
use static_cell::StaticCell;

const SSID: &str = env!("WIFI_SSID");
const PSWD: &str = env!("WIFI_PSWD");

use crate::events::{NtpStatus, StatusLedCommand};
use crate::ntp::fetch_timestamp_ntp;
use crate::ui::{self};
use crate::{AppError, rand_u64};

extern crate alloc;

// shared GPIO input
pub type InputType<'a> = Mutex<NoopRawMutex, gpio::Input<'a>>;

// shared GPIO output
pub type OutputType<'a> = Mutex<NoopRawMutex, gpio::Output<'a>>;

/// i2c bus type
pub type I2cType<'a> = i2c::master::I2c<'a, esp_hal::Async>;

/// shared i2c bus
pub type I2cBus<'a> = Mutex<NoopRawMutex, I2cType<'a>>;

/// use for abstracted i2c bus device that implemented with `embedded-hal-async`
pub type I2cBusDeviceAsync<'a> = shared_bus::asynch::i2c::I2cDevice<'a, NoopRawMutex, I2cType<'a>>;

/// use for abstracted i2c bus device that implemented with `embedded-hal`
pub type I2cBusDeviceBlocking<'a> =
    shared_bus::blocking::i2c::I2cDevice<'a, NoopRawMutex, I2cType<'a>>;

// single RGB led type
pub type RgbLedType<'a> = esp_hal_smartled::RmtSmartLeds<
    'a,
    { esp_hal_smartled::buffer_size::<RGB8>(1) },
    esp_hal::Async,
    RGB8,
    esp_hal_smartled::color_order::Grb,
>;

pub struct Board {
    /// shared i2c0 bus with mutex
    /// Note: use reference to I2cBus instead of Rc<I2cBus> here because the embassy_shared_bus
    /// library required `&'static Mutex` as argument
    pub i2c0_bus: &'static I2cBus<'static>,
    /// rtc share by multiple task, therefore, it should be Rc::clone to pass around instead of borrow
    pub rtc: Rc<rtc_cntl::Rtc<'static>>,
    /// network stack
    pub net_stack: embassy_net::Stack<'static>,
    /// button
    pub button: Rc<InputType<'static>>,
    /// RGB LED
    rgb_led: RgbLedType<'static>,
}

impl Board {
    pub async fn new(spawner: &embassy_executor::Spawner) -> Self {
        let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
        let perip = esp_hal::init(config);

        // heap allocator
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 98768);

        // RTC peripheral
        let mut rtc = rtc_cntl::Rtc::new(perip.LPWR);
        rtc.rwdt.disable();
        let rtc = Rc::new(rtc);

        // led pin, active low
        let status_led = gpio::Output::new(
            perip.GPIO22,
            gpio::Level::Low,
            gpio::OutputConfig::default(),
        );
        spawner.spawn(status_led_task(status_led).unwrap());

        // share access i2c0 bus with static lifetime and async feature
        let i2c0_bus = {
            static I2C_BUS: StaticCell<I2cBus<'static>> = StaticCell::new();
            let i2c_bus = i2c::master::I2c::new(
                perip.I2C0,
                i2c::master::Config::default().with_frequency(time::Rate::from_khz(400)),
            )
            .unwrap()
            .with_scl(perip.GPIO18)
            .with_sda(perip.GPIO23)
            .into_async();

            I2C_BUS.init(Mutex::new(i2c_bus))
        };

        // RGB led
        let rmt = rmt::Rmt::new(perip.RMT, Rate::from_mhz(80))
            .unwrap()
            .into_async();
        let rgb_led = esp_hal_smartled::RmtSmartLeds::new(
            esp_hal_smartled::WS2812B_TIMING,
            rmt.channel0,
            perip.GPIO33,
        )
        .unwrap();

        // button
        let button = Mutex::new(gpio::Input::new(
            perip.GPIO19,
            gpio::InputConfig::default().with_pull(gpio::Pull::Up),
        ));
        let button = Rc::new(button);
        // TODO: initialise I2C1 and wrap it as RefCellDevice and pass it around to other i2c based sensors

        // setup for embassy
        esp_rtos::start(
            timg::TimerGroup::new(perip.TIMG0).timer0,
            SoftwareInterruptControl::new(perip.SW_INTERRUPT).software_interrupt0,
        );

        // setup wifi and network stack
        let net_stack = wifi_setup(perip.WIFI, spawner);

        // NTP time sync task
        spawner.spawn(ntp_task(net_stack, Rc::clone(&rtc)).unwrap());

        Self {
            rtc,
            net_stack,
            i2c0_bus,
            button,
            rgb_led,
        }
    }

    /// wait for network ready
    pub async fn wait_for_network(&mut self) -> Result<(), AppError> {
        // turn on status LED
        StatusLedCommand::On.notify().await;
        wait_networking_ready(&self.net_stack).await?;

        if let Some(config) = self.net_stack.config_v4() {
            ui::UpdateCmd::notify_new_ip_address(config.address).await;
        }

        // turn off status LED when connected
        StatusLedCommand::Off.notify().await;
        Ok(())
    }

    #[inline]
    pub async fn set_rgb_led_color(&mut self, color: RGB8, b: u8) {
        self.rgb_led
            .write(brightness(gamma([color].into_iter()), b))
            .into_future()
            .await
            .unwrap_or_default();
    }
}

#[embassy_executor::task]
async fn status_led_task(mut led: gpio::Output<'static>) {
    loop {
        match StatusLedCommand::wait_for().await {
            StatusLedCommand::On => {
                led.set_low();
            }
            StatusLedCommand::Off => {
                led.set_high();
            }
            StatusLedCommand::Blink(times) => {
                for _ in 0..times {
                    led.toggle();
                    Timer::after_millis(300).await;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn wifi_connection_task(mut controller: wifi::WifiController<'static>) {
    'connect: loop {
        match controller.connect_async().await {
            Ok(info) => {
                ui::UpdateCmd::SetApStatus(info.ssid.as_str().into())
                    .notify()
                    .await;

                // wait until we're no longer connected
                if let Ok(info) = controller.wait_for_disconnect_async().await {
                    ui::UpdateCmd::SetApStatus(alloc::format!(
                        "Disconnected from: \n{}",
                        info.ssid.as_str()
                    ))
                    .notify()
                    .await;
                }

                // this will reconnect after disconnect
                continue 'connect;
            }
            Err(_) => {
                ui::UpdateCmd::SetApStatus("Failed".into()).notify().await;
            }
        }

        Timer::after_secs(5).await
    }
}
#[embassy_executor::task]
async fn network_stack_task(mut runner: embassy_net::Runner<'static, wifi::Interface<'static>>) {
    runner.run().await;
}

#[embassy_executor::task]
async fn ntp_task(net_stack: embassy_net::Stack<'static>, rtc: Rc<rtc_cntl::Rtc<'static>>) {
    wait_networking_ready(&net_stack).await.unwrap();

    // status LED lit when NTP failed
    loop {
        match fetch_timestamp_ntp(net_stack, rtc.current_time_us()).await {
            Ok(ts) => {
                NtpStatus::OK.notify();
                StatusLedCommand::Off.notify().await;
                rtc.set_current_time_us(ts);
            }
            Err(e) => {
                NtpStatus::Err.notify();
                StatusLedCommand::Blink(5).notify().await;
                println!("NTP error: {}, retrying..", e);

                Timer::after_secs(5).await;
                continue;
            }
        }

        // every 15 minutes
        Timer::after_secs(15 * 60).await;
    }
}

/// setup wifi controller and spawn network stack background task
fn wifi_setup(
    wifi_peri: esp_hal::peripherals::WIFI<'static>,
    spawner: &embassy_executor::Spawner,
) -> embassy_net::Stack<'static> {
    // setup wifi controller
    let (wifi_controller, wifi_intfs) = wifi::new(
        wifi_peri,
        wifi::ControllerConfig::default().with_initial_config(wifi::Config::Station(
            wifi::sta::StationConfig::default()
                .with_ssid(SSID)
                .with_password(PSWD.into()),
        )),
    )
    .expect("Failed to initialize Wi-Fi controller");
    let wifi_sta_intf = wifi_intfs.station;

    // embassy network stack init
    let res = {
        static RES: StaticCell<StackResources<3>> = StaticCell::new();
        RES.init(StackResources::new())
    };
    let (net_stack, net_runner) = embassy_net::new(
        wifi_sta_intf,
        embassy_net::Config::dhcpv4(Default::default()),
        res,
        rand_u64(),
    );

    // handle run wifi link connectivity and network stack task in the background
    // Note: wifi link auto connects to AP immidiately after the task spawned
    spawner.spawn(wifi_connection_task(wifi_controller).unwrap());
    spawner.spawn(network_stack_task(net_runner).unwrap());

    net_stack
}

/// waiting for wifi link and IP network stack ready,
/// return `AppError::WifiLinkTimeout` if it failed to connect to the wifi,
/// `AppError::IpAddrTimeout` if it failed to obtain IP address
async fn wait_networking_ready(net_stack: &embassy_net::Stack<'_>) -> Result<(), AppError> {
    embassy_time::with_timeout(Duration::from_secs(120), net_stack.wait_link_up())
        .await
        .map_err(|_| AppError::WifiLinkTimeout)?;

    embassy_time::with_timeout(Duration::from_secs(120), net_stack.wait_config_up())
        .await
        .map_err(|_| AppError::IpAddrTimeout)?;

    // all sets
    Ok(())
}
