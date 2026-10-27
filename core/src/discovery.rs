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
    /// 仅测试 / CI：接受 loopback 源（默认拒绝，真实网络要求同网段私有地址）。
    pub allow_loopback: bool,
    /// 绑定固定端口（默认按 `beacon::BEACON_PORTS` 顺序回退）。
    pub listen_port: Option<u16>,
    /// 固定会话 nonce（默认随机）；测试确定性用。
    pub session_nonce: Option<[u8; 16]>,
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

    pub fn listen_port(&self) -> u16 {
        self.shared.listen_port
    }

    /// 向指定地址发送显式 Query；对端过滤通过后单播回复 Beacon。
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
        shared.listen_port,
        &caps,
        shared.config.discoverable,
    )
}

fn refresh_subnets(subnets: &Mutex<Vec<Subnet>>) {
    let list = if_addrs::get_if_addrs()
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
        .unwrap_or_default();
    if let Ok(mut guard) = subnets.lock() {
        *guard = list;
    }
}

fn same_subnet(a: Ipv4Addr, b: Ipv4Addr, mask: Ipv4Addr) -> bool {
    (a & mask) == (b & mask)
}

fn allowed_source(src: IpAddr, subnets: &[Subnet], allow_loopback: bool) -> bool {
    let IpAddr::V4(ip) = src else {
        return false;
    };
    if ip.is_loopback() {
        return allow_loopback;
    }
    if !(ip.is_private() && !ip.is_broadcast() && !ip.is_multicast() && !ip.is_unspecified()) {
        return false;
    }
    subnets
        .iter()
        .any(|subnet| same_subnet(ip, subnet.ip, subnet.netmask))
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
    let mut targets: Vec<Ipv4Addr> = Vec::new();
    if let Ok(guard) = shared.subnets.lock() {
        targets.extend(guard.iter().filter_map(|subnet| subnet.broadcast));
    }
    targets.push(Ipv4Addr::BROADCAST);
    for target in targets {
        let _ = shared
            .socket
            .send_to(
                &bytes,
                SocketAddr::new(IpAddr::V4(target), shared.listen_port),
            )
            .await;
    }
}

async fn listen_loop(
    shared: Arc<Shared>,
    events: mpsc::Sender<Event>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut buffer = [0u8; beacon::MAX_PACKET];
    let mut cooldowns: HashMap<IpAddr, Instant> = HashMap::new();
    loop {
        let (size, source) = tokio::select! {
            _ = shutdown.changed() => return,
            received = shared.socket.recv_from(&mut buffer) => match received {
                Ok(received) => received,
                Err(_) => continue,
            },
        };
        let allowed = match shared.subnets.lock() {
            Ok(guard) => allowed_source(source.ip(), &guard, shared.config.allow_loopback),
            Err(_) => false,
        };
        if !allowed {
            continue;
        }
        let packet = match beacon::decode_packet(&buffer[..size]) {
            Ok(packet) => packet,
            Err(_) => continue,
        };
        match packet {
            Packet::Query(query) => {
                if query.session_nonce == shared.nonce
                    || query.tag_hash != beacon::tag_hash(&shared.config.tag)
                {
                    continue;
                }
                if !cooldown_allows(&mut cooldowns, source.ip()) {
                    continue;
                }
                if let Ok(bytes) = self_beacon(&shared).encode() {
                    let _ = shared.socket.send_to(&bytes, source).await;
                }
            }
            Packet::Beacon(received) => {
                if received.session_nonce == shared.nonce
                    || received.tag_hash != beacon::tag_hash(&shared.config.tag)
                    || !received.discoverable
                {
                    continue;
                }
                let snapshot = PeerSnapshot {
                    addr: source.ip(),
                    listen_port: received.listen_port,
                    caps: received.caps,
                    session_nonce: received.session_nonce,
                };
                let discovered = match shared.peers.lock() {
                    Ok(mut peers) => {
                        let now = Instant::now();
                        match peers.get_mut(&snapshot.addr) {
                            Some(entry) => {
                                let changed =
                                    entry.snapshot.session_nonce != snapshot.session_nonce;
                                entry.snapshot = snapshot.clone();
                                entry.last_seen = now;
                                changed
                            }
                            None => {
                                peers.insert(
                                    snapshot.addr,
                                    PeerEntry {
                                        snapshot: snapshot.clone(),
                                        last_seen: now,
                                    },
                                );
                                true
                            }
                        }
                    }
                    Err(_) => continue,
                };
                // 只有新地址或 nonce 变化才对外发事件；同 nonce 心跳只刷新存活时间。
                if discovered {
                    let _ = events.send(Event::PeerDiscovered(snapshot)).await;
                }
            }
        }
    }
}

fn cooldown_allows(cooldowns: &mut HashMap<IpAddr, Instant>, source: IpAddr) -> bool {
    let now = Instant::now();
    if let Some(last) = cooldowns.get(&source) {
        if now.duration_since(*last) < Duration::from_millis(beacon::QUERY_REPLY_COOLDOWN_MS) {
            return false;
        }
    }
    if cooldowns.len() > 256 {
        cooldowns.retain(|_, last| now.duration_since(*last) < Duration::from_secs(60));
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
