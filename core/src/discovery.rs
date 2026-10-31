//! 发现运行时：UDP beacon 广播 / 监听、接收过滤、Query 单播回复与候选表（PROTOCOL §2.3）。
//!
//! 身份零暴露：beacon 只携带 tag 指纹 / nonce / 端口 / 能力位；候选表按源地址
//! 维护，事件驱动 UI。业务 TCP listener 与配对在 M2 接入。

use crate::beacon::{self, Beacon, Packet, Query};
use if_addrs::{IfAddr, Ifv4Addr};
use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone)]
pub struct Config {
    pub tag: String,
    pub caps: Vec<String>,
    pub discoverable: bool,
    pub heartbeat: Duration,
    pub peer_expiry: Duration,
    pub sweep_interval: Duration,
    /// 仅测试 / CI：接受 loopback 源（默认拒绝，真实网络要求实际接口同子网单播地址）。
    pub allow_loopback: bool,
    /// 绑定固定端口（默认按 `beacon::BEACON_PORTS` 顺序回退）。
    pub listen_port: Option<u16>,
    /// 固定会话 nonce（默认随机）；测试确定性用。
    pub session_nonce: Option<[u8; 16]>,
    /// Business TCP port announced in beacons; UDP binding stays independent.
    pub advertised_port: Option<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tag: String::from("youmu-link"),
            caps: vec![String::from("discovery")],
            discoverable: true,
            heartbeat: Duration::from_millis(beacon::HEARTBEAT_INTERVAL_MS),
            peer_expiry: Duration::from_millis(beacon::PEER_EXPIRY_MS),
            sweep_interval: Duration::from_secs(1),
            allow_loopback: false,
            listen_port: None,
            session_nonce: None,
            advertised_port: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSnapshot {
    pub addr: IpAddr,
    pub listen_port: u16,
    pub caps: Vec<String>,
    pub session_nonce: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 新地址进入候选表，或已知地址 nonce 变化（对端重启）。
    PeerDiscovered(PeerSnapshot),
    /// 候选超过 `peer_expiry` 无新 beacon。
    PeerExpired(IpAddr),
}

#[derive(Debug)]
pub enum Error {
    Bind(String),
    Socket(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Bind(detail) => write!(f, "cannot bind discovery socket: {detail}"),
            Error::Socket(detail) => write!(f, "socket operation failed: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Subnet {
    ip: Ipv4Addr,
    netmask: Ipv4Addr,
    broadcast: Option<Ipv4Addr>,
}

#[derive(Debug)]
struct PeerEntry {
    snapshot: PeerSnapshot,
    last_seen: Instant,
}

struct Shared {
    socket: Arc<UdpSocket>,
    config: Arc<Config>,
    nonce: [u8; 16],
    listen_port: u16,
    subnets: Arc<Mutex<Vec<Subnet>>>,
    peers: Arc<Mutex<HashMap<IpAddr, PeerEntry>>>,
}

pub struct Discovery {
    shared: Arc<Shared>,
    shutdown: watch::Sender<bool>,
}

impl Discovery {
    /// 绑定 socket 并启动广播 / 监听 / 过期清扫任务；任务随 `stop()` 或 runtime 关闭结束。
    ///
    /// # Errors
    ///
    /// Returns [`Error::Bind`] when the requested port, or every fallback port,
    /// cannot be bound or its socket cannot be configured.
    pub async fn start(config: Config, events: mpsc::Sender<Event>) -> Result<Self> {
        let (socket, listen_port) = match config.listen_port {
            Some(port) => {
                let socket = bind_socket(port)
                    .await
                    .map_err(|e| Error::Bind(format!("{port}: {e}")))?;
                (socket, port)
            }
            None => bind_with_fallback().await?,
        };
        let nonce = config.session_nonce.unwrap_or_else(rand::random);
        let shared = Arc::new(Shared {
            socket: Arc::new(socket),
            config: Arc::new(config),
            nonce,
            listen_port,
            subnets: Arc::new(Mutex::new(Vec::new())),
            peers: Arc::new(Mutex::new(HashMap::new())),
        });
        refresh_subnets(&shared.subnets);
        let (shutdown, shutdown_rx) = watch::channel(false);
        tokio::spawn(heartbeat_loop(Arc::clone(&shared), shutdown_rx.clone()));
        tokio::spawn(listen_loop(
            Arc::clone(&shared),
            events.clone(),
            shutdown_rx.clone(),
        ));
        tokio::spawn(sweep_loop(Arc::clone(&shared), events, shutdown_rx));
        Ok(Self { shared, shutdown })
    }

    #[must_use]
    pub fn listen_port(&self) -> u16 {
        self.shared.listen_port
    }

    /// Query every interface and fallback port; fail only when every probe fails.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Socket`] when the query could not be sent to any target.
    pub async fn rescan(&self) -> Result<()> {
        refresh_subnets(&self.shared.subnets);
        let mut targets = broadcast_targets(&self.shared);
        if self.shared.config.allow_loopback {
            targets.push(Ipv4Addr::LOCALHOST);
        }
        self.query_targets(&targets).await?;
        broadcast_beacon(&self.shared).await;
        Ok(())
    }

    async fn query_targets(&self, targets: &[Ipv4Addr]) -> Result<()> {
        let mut sent = false;
        let mut last_error = None;
        for target in targets {
            for port in beacon::BEACON_PORTS {
                match self
                    .send_query(SocketAddr::new(IpAddr::V4(*target), port))
                    .await
                {
                    Ok(()) => sent = true,
                    Err(error) => last_error = Some(error),
                }
            }
        }
        if sent {
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| Error::Socket("no discovery probe targets".into())))
        }
    }

    /// 向指定地址发送显式 Query；对端过滤通过后单播回复 Beacon。
    ///
    /// # Errors
    ///
    /// Returns [`Error::Socket`] when the query frame is invalid or cannot be sent.
    pub async fn send_query(&self, to: SocketAddr) -> Result<()> {
        let query = Query::new(&self.shared.config.tag, self.shared.nonce);
        let bytes = query.encode().map_err(|e| Error::Socket(e.to_string()))?;
        self.shared
            .socket
            .send_to(&bytes, to)
            .await
            .map_err(|e| Error::Socket(e.to_string()))?;
        Ok(())
    }

    /// 单播发送一份本机 Beacon（心跳广播之外的定向发送）。
    ///
    /// # Errors
    ///
    /// Returns [`Error::Socket`] when the beacon frame is invalid or cannot be sent.
    pub async fn send_beacon_to(&self, to: SocketAddr) -> Result<()> {
        let beacon = self_beacon(&self.shared);
        let bytes = beacon.encode().map_err(|e| Error::Socket(e.to_string()))?;
        self.shared
            .socket
            .send_to(&bytes, to)
            .await
            .map_err(|e| Error::Socket(e.to_string()))?;
        Ok(())
    }

    /// 停止广播 / 监听 / 清扫任务。
    pub fn stop(self) {
        let _ = self.shutdown.send(true);
    }
}

async fn bind_socket(port: u16) -> std::io::Result<UdpSocket> {
    let socket = UdpSocket::bind(("0.0.0.0", port)).await?;
    socket.set_broadcast(true)?;
    Ok(socket)
}

async fn bind_with_fallback() -> Result<(UdpSocket, u16)> {
    let mut last = String::from("no beacon port available");
    for port in beacon::BEACON_PORTS {
        match bind_socket(port).await {
            Ok(socket) => return Ok((socket, port)),
            Err(e) => last = format!("{port}: {e}"),
        }
    }
    Err(Error::Bind(last))
}

fn self_beacon(shared: &Shared) -> Beacon {
    let caps: Vec<&str> = shared.config.caps.iter().map(String::as_str).collect();
    Beacon::new(
        &shared.config.tag,
        shared.nonce,
        shared.config.advertised_port.unwrap_or(shared.listen_port),
        &caps,
        shared.config.discoverable,
    )
}

fn refresh_subnets(subnets: &Mutex<Vec<Subnet>>) {
    let list = interface_subnets();
    if let Ok(mut guard) = subnets.lock() {
        *guard = list;
    }
}

fn interface_subnets() -> Vec<Subnet> {
    if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .iter()
                .filter_map(|interface| match interface.addr {
                    IfAddr::V4(Ifv4Addr {
                        ip,
                        netmask,
                        broadcast,
                        ..
                    }) => Some(Subnet {
                        ip,
                        netmask,
                        broadcast,
                    }),
                    IfAddr::V6(_) => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Use current interface masks rather than RFC1918 classification: enterprise
/// LANs can use public address ranges, and direct links can use 169.254/16.
#[must_use]
pub fn is_lan_source(source: IpAddr, allow_loopback: bool) -> bool {
    allowed_source(source, &interface_subnets(), allow_loopback)
}

fn unicast_v4(ip: Ipv4Addr) -> bool {
    // Exclude 0/8, loopback, multicast, reserved 240/4 and limited broadcast.
    (1..224).contains(&ip.octets()[0]) && !ip.is_loopback()
}

fn subnet_bounds(subnet: &Subnet) -> Option<(Ipv4Addr, Ipv4Addr)> {
    let mask = u32::from(subnet.netmask);
    if !unicast_v4(subnet.ip) || mask == 0 || mask.leading_ones() + mask.trailing_zeros() != 32 {
        return None;
    }
    let network = subnet.ip & subnet.netmask;
    let broadcast = network | !subnet.netmask;
    if subnet.ip == network
        || subnet.ip == broadcast
        || subnet
            .broadcast
            .is_some_and(|reported| reported != broadcast)
    {
        return None;
    }
    Some((network, broadcast))
}

fn allowed_source(src: IpAddr, subnets: &[Subnet], allow_loopback: bool) -> bool {
    let IpAddr::V4(ip) = src else {
        return false;
    };
    if ip.is_loopback() {
        return allow_loopback;
    }
    // A second local runtime has a different nonce; none of this host's
    // interfaces represent a remote device, even across overlapping subnets.
    if !unicast_v4(ip) || subnets.iter().any(|subnet| subnet.ip == ip) {
        return false;
    }
    subnets.iter().any(|subnet| {
        subnet_bounds(subnet).is_some_and(|(network, broadcast)| {
            ip & subnet.netmask == network && ip != network && ip != broadcast
        })
    })
}

async fn heartbeat_loop(shared: Arc<Shared>, mut shutdown: watch::Receiver<bool>) {
    let mut interval = tokio::time::interval(shared.config.heartbeat);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            _ = interval.tick() => {
                refresh_subnets(&shared.subnets);
                broadcast_beacon(&shared).await;
            }
        }
    }
}

async fn broadcast_beacon(shared: &Shared) {
    let Ok(bytes) = self_beacon(shared).encode() else {
        return;
    };
    for target in broadcast_targets(shared) {
        for port in beacon::BEACON_PORTS {
            let _ = shared
                .socket
                .send_to(&bytes, SocketAddr::new(IpAddr::V4(target), port))
                .await;
        }
    }
}

fn broadcast_targets(shared: &Shared) -> Vec<Ipv4Addr> {
    let mut targets: Vec<Ipv4Addr> = Vec::new();
    if let Ok(guard) = shared.subnets.lock() {
        // Keep every local IPv4 for self exclusion, but only probe valid LANs.
        targets.extend(
            guard
                .iter()
                .filter(|subnet| subnet_bounds(subnet).is_some())
                .filter_map(|subnet| subnet.broadcast),
        );
    }
    targets.push(Ipv4Addr::BROADCAST);
    targets.sort_unstable();
    targets.dedup();
    targets
}

async fn listen_loop(
    shared: Arc<Shared>,
    events: mpsc::Sender<Event>,
    mut shutdown: watch::Receiver<bool>,
) {
    // One extra byte distinguishes a 512-byte packet from a truncated datagram.
    let mut buffer = [0u8; beacon::MAX_PACKET + 1];
    let mut cooldowns: HashMap<IpAddr, Instant> = HashMap::new();
    loop {
        let (size, source) = tokio::select! {
            _ = shutdown.changed() => return,
            received = shared.socket.recv_from(&mut buffer) => match received {
                Ok(received) => received,
                Err(_) => continue,
            },
        };
        let Some(packet) = accepted_packet(
            &shared,
            &buffer[..size],
            source.ip(),
            &mut cooldowns,
            interface_subnets,
        ) else {
            continue;
        };
        match packet {
            Packet::Query(_) => {
                if let Ok(bytes) = self_beacon(&shared).encode() {
                    let _ = shared.socket.send_to(&bytes, source).await;
                }
            }
            Packet::Beacon(received) => {
                let snapshot = PeerSnapshot {
                    addr: source.ip(),
                    listen_port: received.listen_port,
                    caps: received.caps,
                    session_nonce: received.session_nonce,
                };
                let (discovered, evicted) = match shared.peers.lock() {
                    Ok(mut peers) => record_peer(&mut peers, snapshot.clone()),
                    Err(_) => continue,
                };
                if let Some(ip) = evicted {
                    let _ = events.send(Event::PeerExpired(ip)).await;
                }
                // 只有新地址或 nonce 变化才对外发事件；同 nonce 心跳只刷新存活时间。
                if discovered {
                    let _ = events.send(Event::PeerDiscovered(snapshot)).await;
                }
            }
        }
    }
}

fn accepted_packet(
    shared: &Shared,
    bytes: &[u8],
    source: IpAddr,
    cooldowns: &mut HashMap<IpAddr, Instant>,
    interfaces: impl FnOnce() -> Vec<Subnet>,
) -> Option<Packet> {
    let allowed = match shared.subnets.lock() {
        Ok(guard) => allowed_source(source, &guard, shared.config.allow_loopback),
        Err(_) => false,
    };
    if !allowed {
        return None;
    }
    let packet = beacon::decode_packet(bytes).ok()?;
    let matches = match &packet {
        Packet::Query(query) => {
            query.session_nonce != shared.nonce
                && query.tag_hash == beacon::tag_hash(&shared.config.tag)
                && cooldown_allows(cooldowns, source)
        }
        Packet::Beacon(received) => {
            received.session_nonce != shared.nonce
                && received.tag_hash == beacon::tag_hash(&shared.config.tag)
                && received.discoverable
        }
    };
    // The heartbeat cache can predate a newly added local address. Recheck
    // current interfaces only for valid packets, after the query cooldown.
    if !matches || !allowed_source(source, &interfaces(), shared.config.allow_loopback) {
        return None;
    }
    Some(packet)
}

fn record_peer(
    peers: &mut HashMap<IpAddr, PeerEntry>,
    snapshot: PeerSnapshot,
) -> (bool, Option<IpAddr>) {
    let now = Instant::now();
    if let Some(entry) = peers.get_mut(&snapshot.addr) {
        let changed = entry.snapshot.session_nonce != snapshot.session_nonce
            || entry.snapshot.listen_port != snapshot.listen_port;
        entry.snapshot = snapshot;
        entry.last_seen = now;
        return (changed, None);
    }
    let evicted = if peers.len() >= 128 {
        peers
            .iter()
            .min_by_key(|(_, entry)| entry.last_seen)
            .map(|(ip, _)| *ip)
    } else {
        None
    };
    if let Some(ip) = evicted {
        peers.remove(&ip);
    }
    peers.insert(
        snapshot.addr,
        PeerEntry {
            snapshot,
            last_seen: now,
        },
    );
    (true, evicted)
}

fn cooldown_allows(cooldowns: &mut HashMap<IpAddr, Instant>, source: IpAddr) -> bool {
    let now = Instant::now();
    if let Some(last) = cooldowns.get(&source) {
        if now.duration_since(*last) < Duration::from_millis(beacon::QUERY_REPLY_COOLDOWN_MS) {
            return false;
        }
    }
    if cooldowns.len() >= 256 && !cooldowns.contains_key(&source) {
        if let Some(oldest) = cooldowns
            .iter()
            .min_by_key(|(_, last)| **last)
            .map(|(ip, _)| *ip)
        {
            cooldowns.remove(&oldest);
        }
    }
    cooldowns.insert(source, now);
    true
}

async fn sweep_loop(
    shared: Arc<Shared>,
    events: mpsc::Sender<Event>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(shared.config.sweep_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            _ = interval.tick() => {}
        }
        let expired: Vec<IpAddr> = match shared.peers.lock() {
            Ok(mut peers) => {
                let now = Instant::now();
                let expired: Vec<IpAddr> = peers
                    .iter()
                    .filter(|(_, entry)| {
                        now.duration_since(entry.last_seen) > shared.config.peer_expiry
                    })
                    .map(|(addr, _)| *addr)
                    .collect();
                for addr in &expired {
                    peers.remove(addr);
                }
                expired
            }
            Err(_) => continue,
        };
        for addr in expired {
            let _ = events.send(Event::PeerExpired(addr)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn discovery_without_broadcast() -> Discovery {
        // A wildcard-bound socket can still broadcast on macOS with SO_BROADCAST
        // disabled. Bind to loopback so rejected probes never use a LAN route.
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        socket.set_broadcast(false).unwrap();
        let listen_port = socket.local_addr().unwrap().port();
        let (shutdown, _) = watch::channel(false);
        Discovery {
            shared: Arc::new(Shared {
                socket: Arc::new(socket),
                config: Arc::new(Config::default()),
                nonce: [42; 16],
                listen_port,
                subnets: Arc::new(Mutex::new(Vec::new())),
                peers: Arc::new(Mutex::new(HashMap::new())),
            }),
            shutdown,
        }
    }

    #[tokio::test]
    async fn query_targets_survive_unreachable_broadcast_and_probe_every_port() {
        let mut receivers = Vec::new();
        for port in beacon::BEACON_PORTS {
            receivers.push(UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).await.unwrap());
        }
        let discovery = discovery_without_broadcast().await;
        assert!(matches!(
            discovery
                .send_query(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::BROADCAST),
                    beacon::BEACON_PORTS[0],
                ))
                .await,
            Err(Error::Socket(_))
        ));
        let result = discovery
            .query_targets(&[
                Ipv4Addr::BROADCAST,
                Ipv4Addr::LOCALHOST,
                Ipv4Addr::BROADCAST,
            ])
            .await;
        assert!(
            result.is_ok(),
            "reachable probes must survive route errors: {result:?}"
        );
        for receiver in receivers {
            let mut bytes = [0; beacon::MAX_PACKET + 1];
            let (size, source) =
                tokio::time::timeout(Duration::from_secs(1), receiver.recv_from(&mut bytes))
                    .await
                    .expect("each fallback port must receive a query")
                    .unwrap();
            assert_eq!(source.port(), discovery.listen_port());
            assert!(matches!(
                beacon::decode_packet(&bytes[..size]).unwrap(),
                Packet::Query(_)
            ));
        }
        discovery.stop();
    }

    #[tokio::test]
    async fn query_targets_report_failure_when_no_probe_succeeds() {
        let discovery = discovery_without_broadcast().await;
        assert!(matches!(
            discovery.query_targets(&[Ipv4Addr::BROADCAST]).await,
            Err(Error::Socket(_))
        ));
        discovery.stop();
    }

    fn subnet(ip: &str, mask: &str) -> Subnet {
        Subnet {
            ip: ip.parse().unwrap(),
            netmask: mask.parse().unwrap(),
            broadcast: None,
        }
    }

    #[tokio::test]
    async fn fresh_interfaces_reject_hot_added_local_addresses_before_packet_admission() {
        let discovery = discovery_without_broadcast().await;
        let cached = vec![subnet("10.1.0.2", "255.255.0.0")];
        *discovery.shared.subnets.lock().unwrap() = cached.clone();
        let mut current = cached;
        current.push(subnet("10.1.1.4", "255.255.255.255"));
        let query = Query::new("youmu-link", [7; 16]).encode().unwrap();
        let beacon = Beacon::new("youmu-link", [7; 16], 32001, &["control"], true)
            .encode()
            .unwrap();
        for bytes in [&query, &beacon] {
            for (source, admitted) in [("10.1.1.4", false), ("10.1.1.9", true)] {
                let packet = accepted_packet(
                    &discovery.shared,
                    bytes,
                    source.parse().unwrap(),
                    &mut HashMap::new(),
                    || current.clone(),
                );
                assert_eq!(packet.is_some(), admitted, "source {source}");
            }
            assert!(
                accepted_packet(
                    &discovery.shared,
                    bytes,
                    "10.1.1.9".parse().unwrap(),
                    &mut HashMap::new(),
                    Vec::new,
                )
                .is_none(),
                "unavailable current interfaces must fail closed"
            );
        }
        discovery.stop();
    }

    #[tokio::test]
    async fn rejected_packets_and_cooled_queries_do_not_enumerate_interfaces() {
        let discovery = discovery_without_broadcast().await;
        *discovery.shared.subnets.lock().unwrap() = vec![subnet("10.1.0.2", "255.255.0.0")];
        let source: IpAddr = "10.1.1.9".parse().unwrap();
        let query = Query::new("youmu-link", [7; 16]).encode().unwrap();
        let mut oversized = query.clone();
        oversized.resize(beacon::MAX_PACKET + 1, 0);
        let rejected = vec![
            vec![0],
            oversized,
            Query::new("other-tag", [7; 16]).encode().unwrap(),
            Query::new("youmu-link", [42; 16]).encode().unwrap(),
            Beacon::new("other-tag", [7; 16], 32001, &[], true)
                .encode()
                .unwrap(),
            Beacon::new("youmu-link", [42; 16], 32001, &[], true)
                .encode()
                .unwrap(),
            Beacon::new("youmu-link", [7; 16], 32001, &[], false)
                .encode()
                .unwrap(),
        ];
        for bytes in rejected {
            assert!(accepted_packet(
                &discovery.shared,
                &bytes,
                source,
                &mut HashMap::new(),
                || panic!("rejected packet must not enumerate interfaces"),
            )
            .is_none());
        }
        for source in ["10.1.0.2", "10.2.1.9", "127.0.0.1", "224.0.0.1"] {
            assert!(accepted_packet(
                &discovery.shared,
                &query,
                source.parse().unwrap(),
                &mut HashMap::new(),
                || panic!("cached source rejection must not enumerate interfaces"),
            )
            .is_none());
        }
        let mut cooldowns = HashMap::from([(source, Instant::now())]);
        assert!(accepted_packet(
            &discovery.shared,
            &query,
            source,
            &mut cooldowns,
            || panic!("cooled query must not enumerate interfaces"),
        )
        .is_none());
        discovery.stop();
    }

    #[test]
    fn local_interface_addresses_are_not_remote_peers() {
        let interfaces = [
            subnet("172.22.64.1", "255.255.240.0"),
            subnet("172.22.192.1", "255.255.240.0"),
            subnet("192.168.1.3", "255.255.255.0"),
            subnet("192.168.31.67", "255.255.255.0"),
        ];
        for interface in interfaces {
            for allow_loopback in [false, true] {
                assert!(
                    !allowed_source(IpAddr::V4(interface.ip), &interfaces, allow_loopback),
                    "local interface {} must never become a remote candidate",
                    interface.ip
                );
            }
        }
        assert!(allowed_source(
            "192.168.31.9".parse().unwrap(),
            &interfaces,
            false
        ));
        assert!(!allowed_source(
            "127.0.0.1".parse().unwrap(),
            &interfaces,
            false
        ));
        assert!(allowed_source(
            "127.0.0.1".parse().unwrap(),
            &interfaces,
            true
        ));
    }

    #[test]
    fn local_address_cannot_be_admitted_by_an_overlapping_interface() {
        let interfaces = [
            subnet("10.1.0.2", "255.255.0.0"),
            subnet("10.1.1.2", "255.255.255.0"),
            subnet("10.1.1.4", "255.255.255.255"),
        ];
        for source in ["10.1.1.2", "10.1.1.4"] {
            assert!(!allowed_source(source.parse().unwrap(), &interfaces, false));
        }
        assert!(allowed_source(
            "10.1.1.3".parse().unwrap(),
            &interfaces,
            false
        ));
    }

    #[tokio::test]
    async fn invalid_interface_entries_are_not_broadcast_targets() {
        let discovery = discovery_without_broadcast().await;
        *discovery.shared.subnets.lock().unwrap() = vec![
            Subnet {
                broadcast: Some("10.1.0.255".parse().unwrap()),
                ..subnet("10.1.0.2", "255.255.255.0")
            },
            Subnet {
                broadcast: Some("10.1.1.4".parse().unwrap()),
                ..subnet("10.1.1.4", "255.255.255.255")
            },
            Subnet {
                broadcast: Some("203.0.113.8".parse().unwrap()),
                ..subnet("10.1.2.2", "255.255.255.0")
            },
        ];
        assert_eq!(
            broadcast_targets(&discovery.shared),
            vec!["10.1.0.255".parse().unwrap(), Ipv4Addr::BROADCAST]
        );
        discovery.stop();
    }

    #[test]
    fn enterprise_and_link_local_unicast_follow_interface_subnets() {
        let enterprise = [subnet("198.51.100.1", "255.255.255.0")];
        assert!(allowed_source(
            "198.51.100.23".parse().unwrap(),
            &enterprise,
            false
        ));
        assert!(!allowed_source(
            "203.0.113.8".parse().unwrap(),
            &enterprise,
            false
        ));
        let direct = [subnet("169.254.10.1", "255.255.0.0")];
        assert!(allowed_source(
            "169.254.99.8".parse().unwrap(),
            &direct,
            false
        ));
    }

    #[test]
    fn network_broadcast_and_special_sources_are_never_production_peers() {
        let interfaces = [subnet("198.51.100.1", "255.255.255.0")];
        for source in [
            "198.51.100.0",
            "198.51.100.255",
            "255.255.255.255",
            "224.0.0.1",
            "240.0.0.1",
            "0.0.0.0",
            "0.1.2.3",
            "127.0.0.1",
            "::1",
        ] {
            assert!(
                !allowed_source(source.parse().unwrap(), &interfaces, false),
                "invalid source {source}"
            );
        }
        assert!(allowed_source("127.0.0.1".parse().unwrap(), &[], true));
        assert!(!allowed_source("::1".parse().unwrap(), &[], true));
        let private = [subnet("10.0.0.1", "255.255.255.0")];
        assert!(!allowed_source(
            "10.0.0.255".parse().unwrap(),
            &private,
            false
        ));
    }

    #[test]
    fn invalid_interfaces_and_zero_or_noncontiguous_masks_cannot_admit_peers() {
        for interface in [
            subnet("10.0.0.1", "0.0.0.0"),
            subnet("10.0.0.1", "255.0.255.0"),
            subnet("10.0.0.0", "255.255.255.0"),
            subnet("10.0.0.255", "255.255.255.0"),
            subnet("0.0.0.0", "255.255.255.0"),
            subnet("127.0.0.1", "255.255.255.0"),
            subnet("224.0.0.1", "255.255.255.0"),
            subnet("240.0.0.1", "255.255.255.0"),
            Subnet {
                broadcast: Some("203.0.113.8".parse().unwrap()),
                ..subnet("10.0.0.1", "255.255.255.0")
            },
        ] {
            for source in ["10.0.0.23", "10.23.0.5", "240.0.0.2"] {
                assert!(
                    !allowed_source(source.parse().unwrap(), &[interface], false),
                    "invalid interface {interface:?} admitted {source}"
                );
            }
        }
        assert!(!allowed_source("10.0.0.23".parse().unwrap(), &[], false));
    }

    #[test]
    fn candidate_lru_and_query_cooldowns_have_hard_limits() {
        let mut peers = HashMap::new();
        let mut cooldowns = HashMap::new();
        let first = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for n in 1..=300u16 {
            let addr = IpAddr::V4(Ipv4Addr::new(10, 0, (n / 256) as u8, (n % 256) as u8));
            let (new, _) = record_peer(
                &mut peers,
                PeerSnapshot {
                    addr,
                    listen_port: 32001,
                    caps: vec![],
                    session_nonce: vec![1; 16],
                },
            );
            assert!(new);
            assert!(peers.len() <= 128);
            assert!(cooldown_allows(&mut cooldowns, addr));
            assert!(cooldowns.len() <= 256);
        }
        assert!(!peers.contains_key(&first));
        let latest = IpAddr::V4(Ipv4Addr::new(10, 0, 1, 44));
        assert!(!cooldown_allows(&mut cooldowns, latest));
        let snapshot = peers[&latest].snapshot.clone();
        assert!(!record_peer(&mut peers, snapshot.clone()).0);
        let changed = PeerSnapshot {
            listen_port: 32002,
            ..snapshot
        };
        assert!(record_peer(&mut peers, changed).0);
    }
}
