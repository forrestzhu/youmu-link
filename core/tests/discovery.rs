//! 发现层 loopback 集成测试（TESTING L2）：Query 单播回复、tag 隔离、
//! 冷却与去重、自回环抑制、外部 beacon 发现、候选过期、端口回退。
//!
//! 测试端口避开 `beacon::BEACON_PORTS`，只有端口回退用例占用真实序列；
//! loopback 显式经 `allow_loopback` 放行（生产默认拒绝）。

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;
use tokio::sync::mpsc;
use ymlink_core::beacon::{self, Beacon};
use ymlink_core::discovery::{Config, Discovery, Event};

const LONG_HEARTBEAT: Duration = Duration::from_secs(3600);

fn test_config(tag: &str, port: u16, nonce: u8) -> Config {
    Config {
        tag: tag.to_string(),
        allow_loopback: true,
        heartbeat: LONG_HEARTBEAT,
        sweep_interval: Duration::from_millis(50),
        listen_port: Some(port),
        session_nonce: Some([nonce; 16]),
        ..Config::default()
    }
}

async fn spawn(tag: &str, port: u16, nonce: u8) -> (Discovery, mpsc::Receiver<Event>) {
    let (events, receiver) = mpsc::channel(64);
    let discovery = Discovery::start(test_config(tag, port, nonce), events)
        .await
        .expect("discovery starts");
    (discovery, receiver)
}

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}

#[tokio::test]
async fn query_replies_with_beacon_and_discovers_peer() {
    let (a, mut events_a) = spawn("youmu-link", 57610, 1).await;
    let (b, _events_b) = spawn("youmu-link", 57611, 2).await;
    a.send_query(loopback(b.listen_port()))
        .await
        .expect("query sent");
    let event = tokio::time::timeout(Duration::from_secs(2), events_a.recv())
        .await
        .expect("peer discovered in time")
        .expect("channel open");
    match event {
        Event::PeerDiscovered(peer) => {
            assert_eq!(peer.addr, loopback(b.listen_port()).ip());
            assert_eq!(peer.listen_port, 57611);
            assert_eq!(peer.session_nonce, vec![2u8; 16]);
        }
        other => panic!("unexpected event: {other:?}"),
    }
    a.stop();
    b.stop();
}

#[tokio::test]
async fn different_tag_is_silent() {
    let (a, mut events_a) = spawn("youmu-link", 57612, 3).await;
    let (b, mut events_b) = spawn("other-tag", 57613, 4).await;
    a.send_query(loopback(b.listen_port()))
        .await
        .expect("query sent");
    assert!(
        tokio::time::timeout(Duration::from_millis(300), events_a.recv())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(300), events_b.recv())
            .await
            .is_err()
    );
    a.stop();
    b.stop();
}

#[tokio::test]
async fn repeat_query_is_coalesced() {
    let (a, mut events_a) = spawn("youmu-link", 57614, 5).await;
    let (b, _events_b) = spawn("youmu-link", 57615, 6).await;
    a.send_query(loopback(b.listen_port())).await.unwrap();
    a.send_query(loopback(b.listen_port())).await.unwrap();
    let first = tokio::time::timeout(Duration::from_secs(2), events_a.recv())
        .await
        .expect("first discovery")
        .expect("channel open");
    assert!(matches!(first, Event::PeerDiscovered(_)));
    assert!(
        tokio::time::timeout(Duration::from_millis(300), events_a.recv())
            .await
            .is_err(),
        "duplicate discovery suppressed"
    );
    a.stop();
    b.stop();
}

#[tokio::test]
async fn own_beacon_is_ignored() {
    let (a, mut events_a) = spawn("youmu-link", 57616, 7).await;
    a.send_beacon_to(loopback(a.listen_port())).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(300), events_a.recv())
            .await
            .is_err()
    );
    a.stop();
}

#[tokio::test]
async fn foreign_beacon_becomes_candidate() {
    let (a, mut events_a) = spawn("youmu-link", 57617, 8).await;
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("raw sender");
    let beacon = Beacon::new("youmu-link", [9u8; 16], 47610, &["discovery"], true);
    let bytes = beacon.encode().unwrap();
    sender
        .send_to(&bytes, loopback(a.listen_port()))
        .expect("beacon sent");
    let event = tokio::time::timeout(Duration::from_secs(2), events_a.recv())
        .await
        .expect("foreign beacon discovered")
        .expect("channel open");
    match event {
        Event::PeerDiscovered(peer) => {
            assert_eq!(peer.session_nonce, vec![9u8; 16]);
            assert_eq!(peer.listen_port, 47610);
        }
        other => panic!("unexpected event: {other:?}"),
    }
    a.stop();
}

#[tokio::test]
async fn peer_expires_without_refresh() {
    let config = Config {
        peer_expiry: Duration::from_millis(120),
        ..test_config("youmu-link", 57618, 10)
    };
    let (events, mut receiver) = mpsc::channel(64);
    let a = Discovery::start(config, events)
        .await
        .expect("discovery starts");
    let (b, _events_b) = spawn("youmu-link", 57619, 11).await;
    a.send_query(loopback(b.listen_port())).await.unwrap();
    let first = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("discovery")
        .expect("channel open");
    assert!(matches!(first, Event::PeerDiscovered(_)));
    let second = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("expiry")
        .expect("channel open");
    assert_eq!(second, Event::PeerExpired(loopback(0).ip()));
    a.stop();
    b.stop();
}

#[tokio::test]
async fn beacon_ports_fall_back_in_order() {
    let blocker1 = UdpSocket::bind(("0.0.0.0", beacon::BEACON_PORTS[0])).expect("block first port");
    let blocker2 =
        UdpSocket::bind(("0.0.0.0", beacon::BEACON_PORTS[1])).expect("block second port");
    let (events, _receiver) = mpsc::channel(64);
    let config = Config {
        allow_loopback: true,
        heartbeat: LONG_HEARTBEAT,
        ..Config::default()
    };
    let discovery = Discovery::start(config, events)
        .await
        .expect("fallback bind");
    assert_eq!(discovery.listen_port(), beacon::BEACON_PORTS[2]);
    discovery.rescan().await.unwrap();
    for socket in [&blocker1, &blocker2] {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = [0u8; beacon::MAX_PACKET + 1];
        let (size, _) = socket
            .recv_from(&mut bytes)
            .expect("fallback port receives probe or announcement");
        assert!(beacon::decode_packet(&bytes[..size]).is_ok());
    }
    discovery.stop();
    drop(blocker1);
    drop(blocker2);
}

#[tokio::test]
async fn tcp_port_is_advertised_independently_of_udp_binding() {
    let config = Config {
        advertised_port: Some(32001),
        ..test_config("youmu-link", 57620, 12)
    };
    let (events, _receiver) = mpsc::channel(64);
    let a = Discovery::start(config, events).await.unwrap();
    let (b, mut receiver) = spawn("youmu-link", 57621, 13).await;
    a.send_beacon_to(loopback(b.listen_port())).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(event,Event::PeerDiscovered(peer) if peer.listen_port==32001));
    assert_eq!(a.listen_port(), 57620);
    a.stop();
    b.stop();
}

#[tokio::test]
async fn oversized_datagram_with_valid_prefix_is_ignored() {
    let (a, mut receiver) = spawn("youmu-link", 57622, 14).await;
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut packet = Beacon::new("youmu-link", [15; 16], 32001, &["control"], true)
        .encode()
        .unwrap();
    packet.resize(beacon::MAX_PACKET + 100, 0);
    sender.send_to(&packet, loopback(a.listen_port())).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(200), receiver.recv())
            .await
            .is_err()
    );
    a.stop();
}
