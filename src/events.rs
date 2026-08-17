use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};

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

static LED_CMD: Channel<CriticalSectionRawMutex, StatusLedCommand, 2> = Channel::new();

#[derive(Clone, Copy, Default)]
pub enum StatusLedCommand {
    On,
    #[default]
    Off,
    Blink(u8),
}

impl StatusLedCommand {
    pub async fn notify(&self) {
        LED_CMD.send(*self).await;
    }

    pub async fn wait_for() -> StatusLedCommand {
        LED_CMD.receive().await
    }
}
