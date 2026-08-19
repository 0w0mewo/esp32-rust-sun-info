use core::net::SocketAddr;

use crate::MICROSECS_PER_SEC;
use embassy_net::{
    dns,
    udp::{PacketMetadata, UdpSocket},
};
use embassy_time::Duration;
use sntpc::NtpContext;
use sntpc_net_embassy::UdpSocketWrapper;

const NTP_SERVER: &str = env!("NTP_SERVER");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error")]
    Network,
    #[error("dns error")]
    Dns,
    #[error("timeout")]
    Timeout,
}

#[derive(Clone, Copy, Default)]
struct Timestamp {
    rtc_timestamp_us: u64,
    pivot: u64,
}

impl Timestamp {
    pub fn new(timestamp_us: u64) -> Self {
        Self {
            rtc_timestamp_us: timestamp_us,
            pivot: 0,
        }
    }
}

impl sntpc::NtpTimestampGenerator for Timestamp {
    fn init(&mut self) {
        self.pivot = self.rtc_timestamp_us;
    }

    fn timestamp_sec(&self) -> u64 {
        self.pivot / MICROSECS_PER_SEC
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        self.pivot.rem_euclid(MICROSECS_PER_SEC) as u32
    }
}

pub async fn fetch_timestamp_ntp(
    net_stack: embassy_net::Stack<'_>,
    current_timestamp_us: u64,
) -> Result<u64, Error> {
    let ntp_server_addrs = net_stack
        .dns_query(NTP_SERVER, dns::DnsQueryType::A)
        .await
        .map_err(|_| Error::Dns)?;
    if ntp_server_addrs.is_empty() {
        return Err(Error::Dns);
    }

    let ntp_server_addr = ntp_server_addrs[0]; // use the first available address only

    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buffer = [0; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buffer = [0; 128];

    let ntpc_socket = {
        let mut socket = UdpSocket::new(
            net_stack,
            &mut rx_meta,
            &mut rx_buffer,
            &mut tx_meta,
            &mut tx_buffer,
        );
        socket.bind(123).unwrap();

        UdpSocketWrapper::new(socket)
    };

    embassy_time::with_timeout(
        Duration::from_secs(20),
        sntpc::get_time(
            SocketAddr::from((ntp_server_addr, 123)),
            &ntpc_socket,
            NtpContext::new(Timestamp::new(current_timestamp_us)),
        ),
    )
    .await
    .map_err(|_| Error::Timeout)
    .and_then(|r| match r {
        Ok(t) => {
            Ok(t.sec() * MICROSECS_PER_SEC + ((t.sec_fraction() as u64 * MICROSECS_PER_SEC) >> 32))
        }

        _ => Err(Error::Network),
    })
}
