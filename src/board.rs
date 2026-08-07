use alloc::rc::Rc;
use embassy_embedded_hal::shared_bus;
use embassy_net::StackResources;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::{
    gpio, interrupt::software::SoftwareInterruptControl, ledc, rtc_cntl, time, timer::timg,
};
use esp_println::println;
use esp_radio::wifi;
use static_cell::StaticCell;

const SSID: &str = env!("WIFI_SSID");
const PSWD: &str = env!("WIFI_PSWD");

use crate::events::NtpStatus;
use crate::ntp::fetch_timestamp_ntp;
use crate::{AppError, rand_u64};

extern crate alloc;

/// i2c bus type
pub type I2cType<'a> = i2c::master::I2c<'a, esp_hal::Async>;

/// shared i2c bus
pub type I2cBus<'a> = Mutex<NoopRawMutex, I2cType<'a>>;

/// use for abstracted i2c bus device that implemented with `embedded-hal-async`
pub type I2cBusDeviceAsync<'a> = shared_bus::asynch::i2c::I2cDevice<'a, NoopRawMutex, I2cType<'a>>;

/// use for abstracted i2c bus device that implemented with `embedded-hal`
pub type I2cBusDeviceBlocking<'a> =
    shared_bus::blocking::i2c::I2cDevice<'a, NoopRawMutex, I2cType<'a>>;

pub struct Board {
    /// shared i2c0 bus with mutex
    pub i2c0_bus: &'static I2cBus<'static>,
    /// rtc share by multiple task, therefore, it should be Rc::clone to pass around instead of borrow
    pub rtc: Rc<rtc_cntl::Rtc<'static>>,
    led_pwm: ledc::channel::Channel<'static, ledc::LowSpeed>,
    /// network stack
    pub net_stack: embassy_net::Stack<'static>,
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

        // led pin
        let led = gpio::Output::new(
            perip.GPIO22,
            gpio::Level::High,
            gpio::OutputConfig::default(),
        );

        // led pwm with LEDC
        let mut ledc = ledc::Ledc::new(perip.LEDC);
        ledc.set_global_slow_clock(ledc::LSGlobalClkSource::APBClk); // use slow clock source for ledc

        // config lowspeed timer 0 for ledc clock source,
        // it requires static lifetime because ledc channel config require reference to the lstimer0 instance instead of move
        let lstimer0_ref = {
            static LS_TIMER: StaticCell<ledc::timer::Timer<'_, ledc::LowSpeed>> = StaticCell::new();
            LS_TIMER.init_with(|| {
                let mut lstimer0 = ledc.timer::<ledc::LowSpeed>(ledc::timer::Number::Timer0);
                lstimer0
                    .configure(ledc::timer::config::Config {
                        duty: ledc::timer::config::Duty::Duty12Bit,
                        clock_source: ledc::timer::LSClockSource::APBClk,
                        frequency: time::Rate::from_khz(10),
                    })
                    .unwrap();

                lstimer0
            })
        };

        // config ledc channel 0 to use the `led` pin as pwm signal output and use lstimer0 as clock source
        let mut led_pwm = ledc.channel(ledc::channel::Number::Channel0, led);
        led_pwm
            .configure(ledc::channel::config::Config {
                timer: lstimer0_ref,
                duty_pct: 0,
                drive_mode: gpio::DriveMode::OpenDrain,
            })
            .unwrap();

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

        // TODO: initialise I2C1 and wrap it as RefCellDevice and pass it around to other i2c based sensors

        // setup for embassy
        esp_rtos::start(
            timg::TimerGroup::new(perip.TIMG0).timer0,
            SoftwareInterruptControl::new(perip.SW_INTERRUPT).software_interrupt0,
        );

        // setup wifi and network stack
        let net_stack = wifi_setup(perip.WIFI, spawner);

        // NTP time sync task
        let rtc_clone = Rc::clone(&rtc);
        spawner.spawn(ntp_task(net_stack, rtc_clone).unwrap());

        Self {
            rtc,
            led_pwm,
            net_stack,
            i2c0_bus,
        }
    }

    pub async fn init(&mut self) -> Result<(), AppError> {
        // wait for network ready
        wait_networking_ready(&self.net_stack).await?;

        if let Some(config) = self.net_stack.config_v4() {
            println!("Got IP address: {}", config.address);
        }

        Ok(())
    }

    #[inline]
    pub fn set_led_brightness(&mut self, brightness: u8) {
        let brightness = brightness.clamp(0, 100);

        self.led_pwm.set_duty(brightness).unwrap();
    }
}

#[embassy_executor::task]
async fn wifi_connection_task(mut controller: wifi::WifiController<'static>) {
    'connect: loop {
        match controller.connect_async().await {
            Ok(info) => {
                println!("Wifi connected to {}", info.ssid.as_str());

                // wait until we're no longer connected
                if let Ok(info) = controller.wait_for_disconnect_async().await {
                    println!("Wifi disconnected from {}", info.ssid.as_str());
                }

                // this will reconnect after disconnect
                continue 'connect;
            }
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
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

    loop {
        match fetch_timestamp_ntp(net_stack, rtc.current_time_us()).await {
            Ok(ts) => {
                NtpStatus::OK.notify();
                rtc.set_current_time_us(ts);
            }
            Err(e) => {
                NtpStatus::Err.notify();
                Timer::after_secs(5).await;

                println!("NTP error: {}, retrying..", e);
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
