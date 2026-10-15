use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};
use smart_leds::RGB8;

#[derive(Debug, Clone, Copy, Default)]
pub enum NtpStatus {
    OK,
    Err,
    #[default]
    Unknown,
}

static LAST_NTP_STATUS: Signal<CriticalSectionRawMutex, NtpStatus> = Signal::new();

impl NtpStatus {
    /// get last NTP status from global signal pipe
    pub fn last() -> Option<Self> {
        LAST_NTP_STATUS.try_take()
    }

    /// send the current NTP status to global signal pipe
    pub fn notify(&self) {
        LAST_NTP_STATUS.signal(*self);
    }
}

impl core::fmt::Display for NtpStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            NtpStatus::OK => "OK",
            NtpStatus::Err => "ERR",
            NtpStatus::Unknown => "IDK",
        };

        write!(f, "{}", s)
    }
}

static LED_CMD: Channel<CriticalSectionRawMutex, LedCommand, 2> = Channel::new();

#[derive(Clone, Copy, Default)]
pub enum LedCommand {
    /// turn on blue LED
    On,
    /// turn off blue LED
    #[default]
    Off,
    /// blink blue LED the given times in a short period
    Blink(u8),
    /// toggle blue LED
    Toggle,
    /// set the color and brightness of the RGB LED, (Not the blue status LED)
    RgbaColor { color: RGB8, brightness: u8 },
}

impl LedCommand {
    pub async fn notify(&self) {
        LED_CMD.send(*self).await;
    }

    pub async fn wait_for() -> LedCommand {
        LED_CMD.receive().await
    }
}
