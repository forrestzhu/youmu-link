use super::*;

#[test]
fn mirror_backpressure_needs_two_windows_then_reduces_to_a_finite_floor() {
    let (sender, mut receiver) = mpsc::channel(1);
    let mut windows = 0;
    assert!(!mirror_congested(&mut windows, 120, 2, false));
    sender.try_send(()).unwrap();
    assert!(!mirror_congested(
        &mut windows,
        120,
        0,
        sender.capacity() == 0
    ));
    assert!(mirror_congested(
        &mut windows,
        120,
        0,
        sender.capacity() == 0
    ));
    receiver.try_recv().unwrap();
    assert!(!mirror_congested(
        &mut windows,
        120,
        0,
        sender.capacity() == 0
    ));
    assert!(!mirror_congested(&mut windows, 120, 24, false));
    assert!(mirror_congested(&mut windows, 120, 24, false));
    assert!(!mirror_congested(&mut windows, 120, 23, false));
    assert!(!mirror_congested(&mut windows, 9, 9, false));

    let mut edge = 1920;
    let mut actual = vec![];
    while let Some(next) = lower_mirror_edge(edge, edge / 2) {
        assert!(next < edge && next % 2 == 0);
        actual.push(next);
        edge = next;
    }
    assert_eq!(actual, [1440, 1080, 810, 640]);
    assert_eq!(lower_mirror_edge(1080, 1920), Some(1440));
    assert_eq!(lower_mirror_edge(640, 360), None);
    assert_eq!(lower_mirror_edge(320, 240), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_failure_survives_resume_and_only_explicit_start_renews_consent() {
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    // Permanently occupy this isolated test's gate: even a failing assertion can
    // never enter Capture::start or request recording access.
    shared
        .capture_gate
        .clone()
        .try_acquire_owned()
        .unwrap()
        .forget();
    shared.edit(|s| {
        s.displays = vec![crate::capture::Display {
            id: 991,
            name: "test display".into(),
            width: 1920,
            height: 1080,
        }];
        s.mirror.status = "paused".into();
        s.mirror.display_id = Some(991);
    });
    let generation = shared.active.lock().unwrap().as_ref().unwrap().generation;
    let failure = shared.mirror_failure.lock().unwrap().clone();
    mirror_command(&shared, Command::ResumeMirror, None).unwrap();
    assert!(Arc::ptr_eq(
        &failure,
        &shared.mirror_failure.lock().unwrap()
    ));
    // An initialization failure from the old generation must survive even when
    // resume already awaits its teardown and the old UI update will be ignored.
    record_capture_failure(&failure, "screen capture stopped unexpectedly".into());
    assert_eq!(
        record_capture_failure(&failure, "later startup failure".into()),
        "screen capture stopped unexpectedly"
    );
    assert_eq!(
        mirror_command(&shared, Command::PauseMirror, Some(generation)).unwrap_err(),
        "screen capture stopped unexpectedly"
    );
    assert_eq!(shared.state.lock().unwrap().mirror.status, "error");
    shared.edit(|s| s.mirror.status = "paused".into());
    assert_eq!(
        mirror_command(&shared, Command::ResumeMirror, Some(generation)).unwrap_err(),
        "screen capture stopped unexpectedly"
    );
    assert_eq!(shared.state.lock().unwrap().mirror.status, "error");
    let start = |display_id| Command::StartMirror {
        display_id,
        audio: false,
        quality: "balanced".into(),
    };
    assert!(mirror_command(&shared, start(992), None).is_err());
    assert!(Arc::ptr_eq(
        &failure,
        &shared.mirror_failure.lock().unwrap()
    ));
    mirror_command(&shared, start(991), None).unwrap();
    let renewed = shared.mirror_failure.lock().unwrap().clone();
    assert!(!Arc::ptr_eq(&failure, &renewed));
    assert!(renewed.message().is_none());
    assert!(failure.message().is_some());
    mirror_command(&shared, Command::StopMirror, None).unwrap();
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[test]
fn mirror_start_announcement_rejects_old_generations_and_full_control_queues() {
    let state = Mutex::new(Snapshot::new("test".into()));
    let current = AtomicU64::new(2);
    let (sender, mut receiver) = mpsc::channel(1);
    assert!(!announce_mirror(&state, &current, 1, &sender, 11));
    assert!(receiver.try_recv().is_err());
    assert!(announce_mirror(&state, &current, 2, &sender, 22));
    current.store(3, Ordering::Release);
    assert!(!announce_mirror(&state, &current, 2, &sender, 11));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Message::MirrorState { stream_id: 22, .. }
    ));
    sender.try_send(Message::Ping { v: VERSION }).unwrap();
    assert!(!announce_mirror(&state, &current, 3, &sender, 33));
    assert_eq!(state.lock().unwrap().mirror.status, "error");
}

async fn wait_state(backend: &Backend, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if predicate(&backend.state.lock().unwrap()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "backend state transition timed out"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn command_ack(backend: &Backend, command: Command) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    backend
        .commands
        .send(Request {
            command,
            reply: Some(tx),
            generation: None,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), rx)
        .await
        .unwrap()
        .unwrap()
}

#[test]
fn authenticated_eof_disconnects_without_an_alert_but_auth_and_protocol_failures_still_surface() {
    for failure in [
        "authenticated-eof",
        "unauthenticated-eof",
        "invalid-version",
    ] {
        let temp =
            std::env::temp_dir().join(format!("ymlink-peer-close-{:016x}", rand::random::<u64>()));
        let backend = Backend::start_inner(
            temp.clone(),
            "Close Mac".into(),
            Some(Identity::generate().unwrap()),
            true,
        )
        .unwrap();
        let client = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        client.block_on(async {
            wait_state(&backend, |s| s.listen_port != 0).await;
            let port = backend.state.lock().unwrap().listen_port;
            let identity = Identity::generate().unwrap();
            let mut session = SecureSession::handshake(
                TcpStream::connect(("127.0.0.1", port)).await.unwrap(),
                true,
                &identity,
                Hello::new(&identity, "Quest close", "quest", 32009),
            )
            .await
            .unwrap();
            if failure == "unauthenticated-eof" {
                // A peer that enters Known has requested a real connection;
                // failures after this point retain the existing feedback.
                assert!(session.needs_pairing(&TrustStore::memory()).await.unwrap());
                drop(session);
                wait_state(&backend, |s| s.error_id > 0).await;
                let state = backend.state.lock().unwrap();
                assert!(state.error.is_some());
                assert!(!state.peers.iter().any(|p| p.trusted));
                return;
            }
            let mut trust = TrustStore::memory();
            assert!(session.needs_pairing(&trust).await.unwrap());
            session.request_pairing(None).await.unwrap();
            wait_state(&backend, |s| {
                s.pairing.as_ref().is_some_and(|p| p.code.is_some())
            })
            .await;
            let code = backend
                .state
                .lock()
                .unwrap()
                .pairing
                .as_ref()
                .unwrap()
                .code
                .clone()
                .unwrap();
            session
                .authorize(&mut trust, Pairing::Enter { code })
                .await
                .unwrap();
            wait_state(&backend, |s| s.status == "connected").await;
            let before = {
                let mut state = backend.state.lock().unwrap();
                assert!(state.error.is_none());
                state.transfers.push(Transfer {
                    id: "in-flight".into(),
                    name: "synthetic.bin".into(),
                    transferred: 16,
                    total: 32,
                    status: "transferring".into(),
                });
                state.error_id
            };
            if failure == "invalid-version" {
                let (_read, mut write) = session.into_split().unwrap();
                write.send(&Message::Ping { v: VERSION - 1 }).await.unwrap();
                wait_state(&backend, |s| s.error_id > before).await;
                assert!(
                    backend.state.lock().unwrap().error.is_some(),
                    "an authenticated protocol violation must still surface"
                );
            } else {
                // LinkClient.disconnect/close ends the TCP stream without Bye.
                // This is an ordinary peer lifecycle, not a failed operation.
                drop(session);
                wait_state(&backend, |s| {
                    s.status == "ready"
                        && s.peers
                            .iter()
                            .any(|p| p.id == identity.device_id() && p.status == "disconnected")
                })
                .await;
                let state = backend.state.lock().unwrap();
                assert_eq!(
                    state.error_id, before,
                    "ordinary authenticated EOF must not publish a modal error"
                );
                assert!(state.error.is_none());
            }
            assert_eq!(
                backend.state.lock().unwrap().transfers[0].status,
                "waiting",
                "interrupted work must remain recoverable"
            );
        });
        drop(client);
        drop(backend);
        std::fs::remove_dir_all(temp).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hello_only_identification_leaves_idle_state_and_trust_unchanged() {
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    server.await.unwrap().unwrap();
    *shared.active.lock().unwrap() = None;
    shared.edit(|state| state.listen_port = 32000);
    let before = serde_json::to_value(&*shared.state.lock().unwrap()).unwrap();
    let mirror_generation = shared.mirror_generation.load(Ordering::Acquire);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stream = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (remote_stream, address) = listener.accept().await.unwrap();
    let task = tokio::spawn(connection(
        shared.clone(),
        remote_stream,
        address,
        false,
        None,
        false,
    ));
    let identity = Identity::generate().unwrap();
    let session = SecureSession::handshake(
        stream,
        true,
        &identity,
        Hello::new(&identity, "Identification only", "quest", 32009),
    )
    .await
    .unwrap();
    assert_eq!(session.remote().name, "test");
    drop(session);
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(&*shared.state.lock().unwrap()).unwrap(),
        before
    );
    assert!(shared.active.lock().unwrap().is_none());
    assert_eq!(
        shared.mirror_generation.load(Ordering::Acquire),
        mirror_generation
    );
    assert!(!shared
        .trust
        .lock()
        .await
        .peers()
        .iter()
        .any(|peer| peer.device_id == identity.device_id()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn known_wait_cancellation_and_changed_pins_cannot_admit_a_late_connection() {
    for change in ["cancel-inbound", "pin-outbound", "forget-inbound"] {
        let (shared, mut writer, _incoming, _read, server, _commands) =
            isolated_file_server().await;
        writer.send(&Message::Bye { v: VERSION }).await.unwrap();
        server.await.unwrap().unwrap();
        *shared.active.lock().unwrap() = None;
        shared.edit(|state| state.listen_port = 32000);
        let peer = Identity::generate().unwrap();
        let hello = Hello::new(&peer, "Known wait", "quest", 32009);
        if change == "forget-inbound" {
            shared
                .trust
                .lock()
                .await
                .remember(&hello, &peer.public_key())
                .unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (accepted, _) = listener.accept().await.unwrap();
        let initiator = change == "pin-outbound";
        let (local, remote) = if initiator {
            (client, accepted)
        } else {
            (accepted, client)
        };
        let actual = local.peer_addr().unwrap();
        // Peek a duplicated handle without consuming bytes: after Hello,
        // readability proves the desktop has sent Known from its old snapshot.
        let raw = remote.into_std().unwrap();
        let peek = TcpStream::from_std(raw.try_clone().unwrap()).unwrap();
        let remote = TcpStream::from_std(raw).unwrap();
        let task = tokio::spawn(connection(
            shared.clone(),
            local,
            actual,
            initiator,
            None,
            true,
        ));
        let mut session = SecureSession::handshake(remote, !initiator, &peer, hello)
            .await
            .unwrap();
        let mut byte = [0];
        assert!(
            tokio::time::timeout(Duration::from_secs(1), peek.peek(&mut byte))
                .await
                .unwrap()
                .unwrap()
                > 0
        );
        assert!(
            shared.trust.try_lock().is_ok(),
            "Known IO must not hold the trust lock"
        );
        match change {
            "cancel-inbound" => {
                shared
                    .dial_cancel
                    .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
            }
            "pin-outbound" => {
                let other = Identity::generate().unwrap();
                shared
                    .trust
                    .lock()
                    .await
                    .remember_endpoint(
                        &Hello::new(&other, "New endpoint owner", "windows", actual.port()),
                        &other.public_key(),
                        Some(actual.to_string()),
                    )
                    .unwrap();
            }
            _ => {
                shared.trust.lock().await.forget(&peer.device_id()).unwrap();
            }
        }
        let _ = session.needs_pairing(&TrustStore::memory()).await;
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect(change)
            .unwrap();
        assert!(shared.active.lock().unwrap().is_none(), "{change}");
        assert!(shared.state.lock().unwrap().pairing.is_none(), "{change}");
        assert_eq!(
            shared.state.lock().unwrap().error.is_some(),
            change == "pin-outbound",
            "only a real manual pin failure needs recovery feedback"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_known_mismatch_pauses_redial_until_manual_pairing_succeeds() {
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    server.await.unwrap().unwrap();
    *shared.active.lock().unwrap() = None;
    shared.edit(|state| state.listen_port = 32000);
    let peer = Identity::generate().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hello = Hello::new(&peer, "Trust lost on Quest", "quest", address.port());
    shared
        .trust
        .lock()
        .await
        .remember_endpoint(&hello, &peer.public_key(), Some(address.to_string()))
        .unwrap();
    shared.edit(|state| {
        publish_authenticated_peer(
            state,
            Peer {
                id: peer.device_id(),
                name: hello.name.clone(),
                platform: "quest".into(),
                address: address.to_string(),
                status: "disconnected".into(),
                trusted: true,
            },
        )
    });
    let wait_dial = || async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while shared.dialing.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    };
    dial(shared.clone(), address, Some(peer.device_id()), false);
    let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut session = SecureSession::handshake(stream, false, &peer, hello.clone())
        .await
        .unwrap();
    assert!(session.needs_pairing(&TrustStore::memory()).await.unwrap());
    assert!(
        session.request_pairing(None).await.is_err(),
        "automatic retry cannot open a pairing window"
    );
    wait_dial().await;
    for _ in 0..2 {
        dial(shared.clone(), address, Some(peer.device_id()), false);
        assert!(
            tokio::time::timeout(Duration::from_millis(150), listener.accept())
                .await
                .is_err(),
            "a peer needing explicit pairing must not receive another automatic TCP dial"
        );
    }
    assert!(shared.state.lock().unwrap().pairing.is_none());

    // An explicit connection still permits the normal four-digit exchange.
    dial(shared.clone(), address, Some(peer.device_id()), true);
    let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut session = SecureSession::handshake(stream, false, &peer, hello.clone())
        .await
        .unwrap();
    let mut remote_trust = TrustStore::memory();
    assert!(session.needs_pairing(&remote_trust).await.unwrap());
    session.request_pairing(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while shared.state.lock().unwrap().pairing.is_none() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let code = shared
        .state
        .lock()
        .unwrap()
        .pairing
        .as_ref()
        .unwrap()
        .code
        .clone()
        .unwrap();
    session
        .authorize(&mut remote_trust, Pairing::Enter { code })
        .await
        .unwrap();
    let (_reader, mut sender) = session.into_split().unwrap();
    sender.send(&Message::Bye { v: VERSION }).await.unwrap();
    wait_dial().await;

    // Successful authorization re-enables automatic Known recovery.
    dial(shared.clone(), address, Some(peer.device_id()), false);
    let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut session = SecureSession::handshake(stream, false, &peer, hello)
        .await
        .unwrap();
    assert!(!session.needs_pairing(&remote_trust).await.unwrap());
    session
        .authorize(&mut remote_trust, Pairing::ReconnectOnly)
        .await
        .unwrap();
    let (_reader, mut sender) = session.into_split().unwrap();
    sender.send(&Message::Bye { v: VERSION }).await.unwrap();
    wait_dial().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnecting_before_noise_prevents_automatic_reconnect_for_both_selectors() {
    use tokio::io::AsyncReadExt;
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    server.await.unwrap().unwrap();
    *shared.active.lock().unwrap() = None;
    shared.edit(|s| s.listen_port = 32000);
    let (events, _events) = mpsc::channel(8);
    let discovery = Discovery::start(
        discovery::Config {
            listen_port: Some(0),
            allow_loopback: true,
            ..Default::default()
        },
        events,
    )
    .await
    .unwrap();
    let peer = Identity::generate().unwrap();
    let id = peer.device_id();
    for address_only in [false, true] {
        for selected in [Some(id.clone()), None] {
            let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let address = listener.local_addr().unwrap();
            shared.disconnected.lock().unwrap().remove(&id);
            shared.edit(|s| {
                s.peers = vec![Peer {
                    id: id.clone(),
                    address: address.to_string(),
                    name: "Remembered PC".into(),
                    platform: "windows".into(),
                    status: "disconnected".into(),
                    trusted: true,
                }]
            });
            handle_command(
                shared.clone(),
                &discovery,
                Command::Connect {
                    address: address_only.then(|| address.to_string()),
                    peer_id: (!address_only).then(|| id.clone()),
                },
                None,
            )
            .await
            .unwrap();
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
                .await
                .unwrap()
                .unwrap();
            assert!(shared.active.lock().unwrap().is_none());
            assert!(shared.pending_dial_matches(&id));
            handle_command(
                shared.clone(),
                &discovery,
                Command::Disconnect { peer_id: selected },
                None,
            )
            .await
            .unwrap();
            let mut bytes = Vec::new();
            tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut bytes))
                .await
                .unwrap()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                while shared.dialing.load(Ordering::Acquire) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(
                shared.disconnected.lock().unwrap().contains(&id),
                "explicit disconnect must exclude the pending peer from the reconnect tick"
            );
            assert!(shared.dial_target.lock().unwrap().is_none());
            assert!(shared.active.lock().unwrap().is_none());
            assert!(shared.state.lock().unwrap().pairing.is_none());
        }
    }
}

#[test]
fn forgetting_a_device_cancels_its_pending_noise_handshake() {
    let temp =
        std::env::temp_dir().join(format!("ymlink-forget-dial-{:016x}", rand::random::<u64>()));
    let backend = Backend::start_inner(
        temp.clone(),
        "Forget Mac".into(),
        Some(Identity::generate().unwrap()),
        true,
    )
    .unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    client.block_on(async {
        use tokio::io::AsyncReadExt;
        wait_state(&backend, |s| s.listen_port != 0).await;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let id = Identity::generate().unwrap().device_id();
        backend.state.lock().unwrap().peers.push(Peer {
            id: id.clone(),
            address: address.to_string(),
            name: "Remembered PC".into(),
            platform: "windows".into(),
            status: "disconnected".into(),
            trusted: true,
        });
        command_ack(
            &backend,
            Command::Connect {
                address: None,
                peer_id: Some(id.clone()),
            },
        )
        .await
        .unwrap();
        let (mut stream, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap();
        // The remote deliberately never answers Noise: active peer identity is
        // unavailable, so cancellation must match the claimed outbound target.
        command_ack(&backend, Command::Forget { peer_id: id })
            .await
            .unwrap();
        let mut bytes = Vec::new();
        tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut bytes))
            .await
            .expect("forgotten dial must close before handshake timeout")
            .unwrap();
        let state = backend.state.lock().unwrap();
        assert!(state.pairing.is_none());
        assert!(!state.peers.iter().any(|p| p.trusted));
        assert_ne!(state.status, "connected");
    });
    drop(client);
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn rejection_before_ready_keeps_a_route_for_an_explicit_fresh_retry() {
    let temp =
        std::env::temp_dir().join(format!("ymlink-first-limit-{:016x}", rand::random::<u64>()));
    let backend = Backend::start_inner(
        temp.clone(),
        "Limited Mac".into(),
        Some(Identity::generate().unwrap()),
        true,
    )
    .unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    client.block_on(async {
        wait_state(&backend, |s| s.listen_port != 0).await;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        command_ack(
            &backend,
            Command::Connect {
                address: Some(address.to_string()),
                peer_id: None,
            },
        )
        .await
        .unwrap();
        let identity = Identity::generate().unwrap();
        let server = async {
            let trust = TrustStore::memory();
            for attempt in 0..2 {
                let (stream, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
                let mut session = SecureSession::handshake(
                    stream,
                    false,
                    &identity,
                    Hello::new(&identity, "Limited PC", "windows", address.port()),
                )
                .await
                .unwrap();
                assert!(session.needs_pairing(&trust).await.unwrap());
                assert_eq!(
                    session.receive_pairing_request().await.unwrap(),
                    None,
                    "terminal rejection must not invent a resumable window"
                );
                if attempt == 0 {
                    session
                        .reject_pairing(session::AuthErrorCode::RateLimited, 500)
                        .await
                        .unwrap();
                } else {
                    session
                        .offer_pairing([8; 16], Instant::now() + Duration::from_secs(30))
                        .await
                        .unwrap();
                    let mut trust = TrustStore::memory();
                    assert!(matches!(
                        session
                            .authorize(
                                &mut trust,
                                Pairing::Display {
                                    code: "0427".into(),
                                    expires_at: Instant::now() + Duration::from_secs(30)
                                }
                            )
                            .await,
                        Err(session::Error::Cancelled)
                    ));
                }
            }
        };
        let ui = async {
            wait_state(&backend, |s| {
                s.pairing.as_ref().is_some_and(|p| p.stage == "limited")
            })
            .await;
            let p = backend.state.lock().unwrap().pairing.clone().unwrap();
            assert!(p.window_id.is_empty());
            assert!(p.cooldown_deadline.is_some());
            assert!(command_ack(&backend, Command::RetryPairing {})
                .await
                .is_err());
            tokio::time::sleep(
                p.cooldown_deadline
                    .unwrap()
                    .saturating_duration_since(Instant::now())
                    + Duration::from_millis(5),
            )
            .await;
            command_ack(&backend, Command::RetryPairing {})
                .await
                .unwrap();
            wait_state(&backend, |s| {
                s.pairing.as_ref().is_some_and(|p| p.stage == "ready")
            })
            .await;
            assert_eq!(
                backend
                    .state
                    .lock()
                    .unwrap()
                    .pairing
                    .as_ref()
                    .unwrap()
                    .window_id,
                hex_window(&[8; 16])
            );
            command_ack(&backend, Command::CancelPairing).await.unwrap();
            wait_state(&backend, |s| s.pairing.is_none()).await;
        };
        tokio::join!(server, ui);
    });
    drop(client);
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn wrong_code_preserves_the_display_window_and_five_attempts_limit_new_connections() {
    let temp = std::env::temp_dir().join(format!("ymlink-retry-{:016x}", rand::random::<u64>()));
    let backend = Backend::start_inner(
        temp.clone(),
        "Retry Mac".into(),
        Some(Identity::generate().unwrap()),
        true,
    )
    .unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    client.block_on(async {
        wait_state(&backend, |s| s.listen_port != 0).await;
        let port = backend.state.lock().unwrap().listen_port;
        let identity = Identity::generate().unwrap();
        let mut trust = TrustStore::memory();
        let mut resume = None;
        let mut original = None;
        for attempt in 0..6 {
            let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let mut session = SecureSession::handshake(
                stream,
                true,
                &identity,
                Hello::new(&identity, "Quest retry", "quest", 32009),
            )
            .await
            .unwrap();
            assert!(session.needs_pairing(&trust).await.unwrap());
            let ready = session.request_pairing(resume).await;
            if attempt == 5 {
                assert!(matches!(
                    ready,
                    Err(session::Error::RateLimited(1..=60_000))
                ));
                wait_state(&backend, |s| {
                    s.pairing.as_ref().is_some_and(|p| p.stage == "limited")
                })
                .await;
                assert!(backend.state.lock().unwrap().error.is_none());
                break;
            }
            let ready = ready.unwrap();
            resume = Some(ready.window_id);
            wait_state(&backend, |s| {
                s.pairing.as_ref().is_some_and(|p| p.stage == "ready")
            })
            .await;
            let current = backend.state.lock().unwrap().pairing.clone().unwrap();
            if let Some((id, code, deadline)) = &original {
                assert_eq!(&current.window_id, id);
                assert_eq!(&current.code, code);
                assert_eq!(&current.deadline, deadline);
            } else {
                original = Some((
                    current.window_id.clone(),
                    current.code.clone(),
                    current.deadline,
                ));
            }
            let wrong = if current.code.as_deref() == Some("0000") {
                "0001"
            } else {
                "0000"
            };
            assert!(matches!(
                session
                    .authorize(&mut trust, Pairing::Enter { code: wrong.into() })
                    .await,
                Err(session::Error::PairingRejected)
            ));
            drop(session);
            wait_state(&backend, |s| {
                s.pairing.as_ref().is_some_and(|p| p.stage == "incorrect")
            })
            .await;
            assert!(backend.state.lock().unwrap().error.is_none());
            assert!(trust.peers().is_empty());
            assert!(!backend
                .state
                .lock()
                .unwrap()
                .peers
                .iter()
                .any(|p| p.trusted));
        }
        backend.command(r#"{"action":"cancel_pairing"}"#).unwrap();
        wait_state(&backend, |s| s.pairing.is_none()).await;
    });
    drop(client);
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn desktop_input_retries_on_a_new_noise_session_without_replaying_the_wrong_code() {
    desktop_input_retry(false);
}

#[test]
fn resumed_input_cannot_authorize_after_the_original_logical_deadline() {
    desktop_input_retry(true);
}

fn desktop_input_retry(expire: bool) {
    let temp =
        std::env::temp_dir().join(format!("ymlink-enter-retry-{:016x}", rand::random::<u64>()));
    let backend = Backend::start_inner(
        temp.clone(),
        "Input Mac".into(),
        Some(Identity::generate().unwrap()),
        true,
    )
    .unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    client.block_on(async {
        wait_state(&backend, |s| s.listen_port != 0).await;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        backend
            .command(&json!({"action":"connect","address":address.to_string()}).to_string())
            .unwrap();
        let (done_tx, done_rx) = oneshot::channel();
        let server = async {
            let identity = Identity::generate().unwrap();
            let mut trust = TrustStore::memory();
            let deadline = Instant::now()
                + if expire {
                    Duration::from_millis(1200)
                } else {
                    Duration::from_secs(30)
                };
            for attempt in 0..2 {
                let (stream, _) = tokio::time::timeout(Duration::from_secs(4), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
                let mut session = SecureSession::handshake(
                    stream,
                    false,
                    &identity,
                    Hello::new(&identity, "Display PC", "windows", address.port()),
                )
                .await
                .unwrap();
                assert!(session.needs_pairing(&trust).await.unwrap());
                assert_eq!(
                    session.receive_pairing_request().await.unwrap(),
                    if attempt == 0 { None } else { Some([7; 16]) }
                );
                let offered = if expire && attempt == 1 {
                    Instant::now() + Duration::from_secs(30)
                } else {
                    deadline
                };
                session.offer_pairing([7; 16], offered).await.unwrap();
                let result = session
                    .authorize(
                        &mut trust,
                        Pairing::Display {
                            code: "0427".into(),
                            expires_at: offered,
                        },
                    )
                    .await;
                if attempt == 0 {
                    assert!(matches!(result, Err(session::Error::PairingRejected)));
                    assert!(trust.peers().is_empty());
                } else {
                    if expire {
                        assert!(
                            result.is_err(),
                            "a longer resumed ready must not extend the logical window"
                        );
                        assert!(trust.peers().is_empty());
                        break;
                    }
                    result.unwrap();
                    let _ = done_rx.await;
                    let (_, mut writer) = session.into_split().unwrap();
                    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
                    break;
                }
            }
        };
        let ui = async {
            wait_state(&backend, |s| {
                s.pairing.as_ref().is_some_and(|p| p.stage == "ready")
            })
            .await;
            let first = backend.state.lock().unwrap().pairing.clone().unwrap();
            submit_code(&backend, "0000");
            wait_state(&backend, |s| {
                s.pairing
                    .as_ref()
                    .is_some_and(|p| p.error.as_deref() == Some("incorrect"))
            })
            .await;
            wait_state(&backend, |s| {
                s.pairing
                    .as_ref()
                    .is_some_and(|p| p.stage == "ready" && p.attempt != first.attempt)
            })
            .await;
            let second = backend.state.lock().unwrap().pairing.clone().unwrap();
            assert_eq!(first.window_id, second.window_id);
            assert!(
                second.deadline <= first.deadline,
                "resuming must never extend the original deadline"
            );
            assert!(backend.state.lock().unwrap().error.is_none());
            if expire {
                tokio::time::sleep(
                    first.deadline.saturating_duration_since(Instant::now())
                        + Duration::from_millis(40),
                )
                .await;
                wait_state(&backend, |s| {
                    s.pairing.as_ref().is_some_and(|p| p.stage == "expired")
                })
                .await;
                assert!(!backend
                    .state
                    .lock()
                    .unwrap()
                    .peers
                    .iter()
                    .any(|p| p.trusted));
                command_ack(&backend, Command::CancelPairing).await.unwrap();
                return;
            }
            command_ack(
                &backend,
                Command::Pair {
                    code: "0000".into(),
                    window_id: first.window_id,
                    attempt: first.attempt,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                backend
                    .state
                    .lock()
                    .unwrap()
                    .pairing
                    .as_ref()
                    .unwrap()
                    .stage,
                "ready",
                "late UI input from the old TCP attempt must not submit the new one"
            );
            submit_code(&backend, "0427");
            wait_state(&backend, |s| s.status == "connected").await;
            let _ = done_tx.send(());
        };
        tokio::join!(server, ui);
    });
    drop(client);
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}

fn submit_code(backend: &Backend, code: &str) {
    let pairing = backend
        .state
        .lock()
        .unwrap()
        .pairing
        .clone()
        .expect("pending input");
    backend.command(&json!({"action":"pair","code":code,"window_id":pairing.window_id,"attempt":pairing.attempt}).to_string()).unwrap();
}

async fn next(
    receiver: &mut mpsc::Receiver<Message>,
    writer: &mut session::SecureWriter,
) -> Message {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
            .await
            .expect("response deadline")
            .expect("connection open");
        if let Message::Ping { v: VERSION } = message {
            writer.send(&Message::Pong { v: VERSION }).await.unwrap();
        } else {
            return message;
        }
    }
}

fn receiver(mut reader: session::SecureReader) -> (mpsc::Receiver<Message>, AbortTask) {
    let (send, receive) = mpsc::channel(32);
    let task = AbortTask(tokio::spawn(async move {
        while let Ok(message) = reader.recv::<Message>().await {
            if send.send(message).await.is_err() {
                break;
            }
        }
    }));
    (receive, task)
}

async fn control(
    receiver: &mut mpsc::Receiver<Message>,
    writer: &mut session::SecureWriter,
    id: u64,
    command: MirrorAction,
) -> (bool, MirrorInfo) {
    writer
        .send(&Message::MirrorControl {
            v: VERSION,
            id,
            command,
        })
        .await
        .unwrap();
    match next(receiver, writer).await {
        Message::MirrorReply {
            id: response,
            ok,
            state,
            ..
        } if response == id => (ok, state),
        _ => panic!("expected mirror control reply"),
    }
}

#[test]
fn native_backend_pairs_serves_ranges_controls_transfer_reconnects_and_cancels() {
    let temp = std::env::temp_dir().join(format!("ymlink-runtime-{:016x}", rand::random::<u64>()));
    let root = temp.join("media");
    std::fs::create_dir_all(&root).unwrap();
    let content: Vec<u8> = (0..50_000).map(|i| (i % 251) as u8).collect();
    std::fs::write(root.join("sample.bin"), &content).unwrap();
    let backend = Backend::start_inner(
        temp.join("state"),
        "Test Mac".into(),
        Some(Identity::generate().unwrap()),
        true,
    )
    .unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    client.block_on(async {
        wait_state(&backend,|s|s.listen_port!=0).await;
        backend.command(&json!({"action":"add_share","path":root.to_str().unwrap()}).to_string()).unwrap();
        wait_state(&backend,|s|s.shares.len()==1).await;
        let port=backend.state.lock().unwrap().listen_port;
        let identity=Identity::generate().unwrap();let mut trust=TrustStore::memory();
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        let mut session=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Quest test","headset-test",32009)).await.unwrap();
        assert_eq!(session.remote().platform, std::env::consts::OS);
        assert!(session.needs_pairing(&trust).await.unwrap());
        session.request_pairing(None).await.unwrap();
        wait_state(&backend,|s|s.pairing.is_some()).await;
        let code=backend.state.lock().unwrap().pairing.as_ref().unwrap().code.clone().unwrap();
        session.authorize(&mut trust,Pairing::Enter{code}).await.unwrap();
        let (read,mut write)=session.into_split().unwrap();let (mut incoming,_read)=receiver(read);
        wait_state(&backend,|s|s.status=="connected").await;
        write.send(&Message::MirrorQuery{v:VERSION,id:100}).await.unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::MirrorReply{id:100,ok:true,state:MirrorInfo{status,..},..}if status=="stopped"));
        let (ok,view)=control(&mut incoming,&mut write,101,MirrorAction::Resume).await;
        assert!(!ok);assert_eq!(view.status,"stopped","the peer cannot start the first recording");
        let (ok,_)=control(&mut incoming,&mut write,102,MirrorAction::Pause).await;assert!(!ok);
        let (ok,_)=control(&mut incoming,&mut write,103,MirrorAction::Stop).await;assert!(ok);
        // Policy-only paused state: no Capture::start call or OS permission request.
        {
            let mut state=backend.state.lock().unwrap();
            state.displays=vec![
                crate::capture::Display{id:991,name:"test display 1".into(),width:1920,height:1080},
                crate::capture::Display{id:992,name:"test display 2".into(),width:1280,height:720},
            ];
            state.mirror.status="paused".into();state.mirror.display_id=Some(991);state.mirror.audio=false;state.mirror.quality="smooth".into();
        }
        let (ok,view)=control(&mut incoming,&mut write,104,MirrorAction::SelectDisplay{display_id:992}).await;
        assert!(ok);assert_eq!(view.status,"paused");assert_eq!(view.display_id,Some(992));assert!(!view.audio);assert_eq!(view.quality,"smooth");
        let (ok,view)=control(&mut incoming,&mut write,105,MirrorAction::Pause).await;assert!(ok);assert_eq!(view.status,"paused");
        let (ok,view)=control(&mut incoming,&mut write,106,MirrorAction::SelectDisplay{display_id:993}).await;assert!(!ok);assert_eq!(view.display_id,Some(992));
        let (ok,view)=control(&mut incoming,&mut write,107,MirrorAction::Stop).await;assert!(ok);assert_eq!(view.status,"stopped");
        let (ok,view)=control(&mut incoming,&mut write,108,MirrorAction::Resume).await;assert!(!ok);assert_eq!(view.status,"stopped");
        write.send(&Message::FileRequest{v:VERSION,id:1,request:FileRequest::Shares}).await.unwrap();
        let share=match next(&mut incoming,&mut write).await {Message::FileResponse{id:1,response:FileResponse::Shares{shares},..}=>shares[0].id.clone(),_=>panic!("expected shares")};
        write.send(&Message::FileRequest{v:VERSION,id:2,request:FileRequest::Stat{share_id:share.clone(),path:"sample.bin".into()}}).await.unwrap();
        let version=match next(&mut incoming,&mut write).await {Message::FileResponse{id:2,response:FileResponse::Stat{entry},..}=>{assert_eq!(entry.size,50_000);entry.version.unwrap()},_=>panic!("expected file metadata")};
        write.send(&Message::FileRequest{v:VERSION,id:3,request:FileRequest::Read{share_id:share.clone(),path:"sample.bin".into(),version:version.clone(),offset:7,length:20}}).await.unwrap();
        match next(&mut incoming,&mut write).await {Message::FileResponse{id:3,response:FileResponse::Read{data,offset:7,..},..}=>assert_eq!(data,content[7..27]),_=>panic!("expected range")};
        write.send(&Message::TransferOpen{v:VERSION,id:"download1".into(),share_id:share,path:"sample.bin".into(),version,offset:0}).await.unwrap();
        let offset=match next(&mut incoming,&mut write).await {Message::TransferChunk{offset:0,data,..}=>{assert_eq!(data,content[..data.len()]);data.len()as u64},_=>panic!("expected chunk")};
        backend.command(r#"{"action":"pause_transfer","id":"download1"}"#).unwrap();
        wait_state(&backend,|s|s.transfers[0].status=="paused").await;
        write.send(&Message::TransferAck{v:VERSION,id:"download1".into(),offset}).await.unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::TransferAction{action,..}if action=="pause"));
        assert!(tokio::time::timeout(Duration::from_millis(60),incoming.recv()).await.is_err(),"paused sender must not send another chunk");
        backend.command(r#"{"action":"resume_transfer","id":"download1"}"#).unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::TransferAction{action,..}if action=="resume"));
        let acknowledged=match next(&mut incoming,&mut write).await {Message::TransferChunk{offset:next,data,..}=>{assert_eq!(next,offset);assert_eq!(data,content[next as usize..next as usize+data.len()]);next+data.len()as u64},_=>panic!("expected resumed chunk")};
        backend.command(r#"{"action":"cancel_transfer","id":"download1"}"#).unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::TransferAction{action,..}if action=="cancel"));
        write.send(&Message::TransferAck{v:VERSION,id:"download1".into(),offset:acknowledged}).await.unwrap();
        write.send(&Message::FileRequest{v:VERSION,id:4,request:FileRequest::Shares}).await.unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::FileResponse{id:4,..}),"late acknowledgement after cancellation must keep the connection usable");
        write.send(&Message::Bye{v:VERSION}).await.unwrap();drop(write);
        wait_state(&backend,|s|s.status!="connected").await;
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        let mut reconnected=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Quest test","headset-test",32009)).await.unwrap();
        assert!(!reconnected.needs_pairing(&trust).await.unwrap());
        reconnected.authorize(&mut trust,Pairing::ReconnectOnly).await.unwrap();
        wait_state(&backend,|s|s.status=="connected").await;
        backend.command(&json!({"action":"disconnect","peer_id":identity.device_id()}).to_string()).unwrap();
        wait_state(&backend,|s|s.status!="connected").await;
        drop(reconnected);
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        if let Ok(mut blocked)=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Quest test","headset-test",32009)).await {
            assert!(blocked.needs_pairing(&trust).await.is_err(),"manual disconnect must reject the peer's automatic inbound retry");
        }
        backend.command(&json!({"action":"forget","peer_id":identity.device_id()}).to_string()).unwrap();
        wait_state(&backend,|s|!s.peers.iter().any(|p|p.id==identity.device_id())).await;
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        let mut unpaired=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Other Quest","headset-test",32010)).await.unwrap();
        assert!(unpaired.needs_pairing(&TrustStore::memory()).await.unwrap());
        unpaired.request_pairing(None).await.unwrap();
        wait_state(&backend,|s|s.pairing.is_some()).await;
        backend.command(r#"{"action":"cancel_pairing"}"#).unwrap();
        wait_state(&backend,|s|s.pairing.is_none()).await;
    });
    drop(client);
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn authorized_folders_survive_backend_restart_and_revocation_stays_revoked() {
    let temp = std::env::temp_dir().join(format!("ymlink-restart-{:016x}", rand::random::<u64>()));
    let root = temp.join("media");
    std::fs::create_dir_all(&root).unwrap();
    let path = temp.join("state");
    let identity = Identity::generate().unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let start = || {
        Backend::start_inner(
            path.clone(),
            "Restart test".into(),
            Some(identity.clone()),
            true,
        )
        .unwrap()
    };
    let backend = start();
    let original = client.block_on(async {
        wait_state(&backend, |s| s.listen_port != 0).await;
        backend
            .command(&json!({"action":"add_share","path":root.to_str().unwrap()}).to_string())
            .unwrap();
        wait_state(&backend, |s| s.shares.len() == 1).await;
        backend.state.lock().unwrap().shares[0].clone()
    });
    drop(backend);
    let backend = start();
    client.block_on(async {
        wait_state(&backend, |s| s.listen_port != 0).await;
        let restored = backend.state.lock().unwrap().shares.clone();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, original.id);
        assert_eq!(restored[0].path, original.path);
        backend
            .command(&json!({"action":"remove_share","id":original.id}).to_string())
            .unwrap();
        wait_state(&backend, |s| s.shares.is_empty()).await;
    });
    drop(backend);
    let backend = start();
    client.block_on(async {
        wait_state(&backend, |s| s.listen_port != 0).await;
        assert!(backend.state.lock().unwrap().shares.is_empty());
    });
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn native_runtime_pairs_windows_macos_and_headset_roles_in_both_directions() {
    // Desktop responders display the code and desktop initiators enter it;
    // against a Quest peer the local desktop always displays the code.
    for (platform, initiator, mode) in [
        ("windows", true, "enter"),
        ("windows", false, "display"),
        ("macos", true, "enter"),
        ("macos", false, "display"),
        ("headset-test", true, "display"),
        ("headset-test", false, "display"),
    ] {
        let temp =
            std::env::temp_dir().join(format!("ymlink-roles-{:016x}", rand::random::<u64>()));
        let backend = Backend::start_inner(
            temp.clone(),
            "Test desktop".into(),
            Some(Identity::generate().unwrap()),
            true,
        )
        .unwrap();
        let client = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        client.block_on(async {
            wait_state(&backend, |s| s.listen_port != 0).await;
            let port = backend.state.lock().unwrap().listen_port;
            let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let remote_port = listener.local_addr().unwrap().port();
            let stream = if initiator {
                backend
                    .command(
                        &json!({"action":"connect","address":format!("127.0.0.1:{remote_port}")})
                            .to_string(),
                    )
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(3), listener.accept())
                    .await
                    .unwrap()
                    .unwrap()
                    .0
            } else {
                TcpStream::connect(("127.0.0.1", port)).await.unwrap()
            };
            let identity = Identity::generate().unwrap();
            let mut session = SecureSession::handshake(
                stream,
                !initiator,
                &identity,
                Hello::new(&identity, "Remote test", platform, remote_port),
            )
            .await
            .unwrap();
            assert_eq!(session.remote().platform, std::env::consts::OS);
            let mut trust = TrustStore::memory();
            assert!(session.needs_pairing(&trust).await.unwrap());
            if mode == "enter" {
                assert!(session.receive_pairing_request().await.unwrap().is_none());
                session
                    .offer_pairing([4; 16], Instant::now() + Duration::from_secs(30))
                    .await
                    .unwrap();
            } else {
                session.request_pairing(None).await.unwrap();
            }
            wait_state(&backend, |s| s.pairing.is_some()).await;
            let pairing = backend.state.lock().unwrap().pairing.clone().unwrap();
            assert_eq!(
                pairing.mode, mode,
                "remote {platform}, local initiator {initiator}"
            );
            let remote_role = if mode == "enter" {
                assert!(pairing.code.is_none());
                submit_code(&backend, "0427");
                Pairing::Display {
                    code: "0427".into(),
                    expires_at: Instant::now() + Duration::from_secs(30),
                }
            } else {
                Pairing::Enter {
                    code: pairing.code.unwrap(),
                }
            };
            session.authorize(&mut trust, remote_role).await.unwrap();
            wait_state(&backend, |s| s.status == "connected").await;
            let (_, mut writer) = session.into_split().unwrap();
            writer.send(&Message::Bye { v: VERSION }).await.unwrap();
            wait_state(&backend, |s| s.status != "connected").await;
        });
        drop(client);
        drop(backend);
        std::fs::remove_dir_all(temp).unwrap();
    }
}

async fn isolated_file_server() -> (
    Arc<Shared>,
    session::SecureWriter,
    mpsc::Receiver<Message>,
    AbortTask,
    tokio::task::JoinHandle<session::Result<()>>,
    AbortTask,
) {
    let identity = Identity::generate().unwrap();
    let client_identity = Identity::generate().unwrap();
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_send_buffer_size(4096).unwrap();
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let listener = socket.listen(8).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (client, server) = tokio::join!(
        async {
            let socket = tokio::net::TcpSocket::new_v4().unwrap();
            socket.set_recv_buffer_size(4096).unwrap();
            let stream = socket
                .connect(format!("127.0.0.1:{port}").parse().unwrap())
                .await
                .unwrap();
            SecureSession::handshake(
                stream,
                true,
                &client_identity,
                Hello::new(&client_identity, "Quest test", "headset-test", port),
            )
            .await
            .unwrap()
        },
        async {
            let (stream, _) = listener.accept().await.unwrap();
            SecureSession::handshake(
                stream,
                false,
                &identity,
                Hello::new(&identity, "Mac test", "macos", port),
            )
            .await
            .unwrap()
        }
    );
    let (mut client, mut server) = (client, server);
    let mut client_trust = TrustStore::memory();
    let mut server_trust = TrustStore::memory();
    client_trust
        .remember(client.remote(), &client.remote_key())
        .unwrap();
    server_trust
        .remember(server.remote(), &server.remote_key())
        .unwrap();
    let (a, b) = tokio::join!(
        client.authorize(&mut client_trust, Pairing::ReconnectOnly),
        server.authorize(&mut server_trust, Pairing::ReconnectOnly)
    );
    a.unwrap();
    b.unwrap();
    let state = Arc::new(Mutex::new(Snapshot::new("test".into())));
    let observed = state.clone();
    let (commands, mut requests) = mpsc::channel::<Request>(8);
    let responder = AbortTask(tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            observed
                .lock()
                .unwrap()
                .set_error("test mirror command processed");
            if let Some(reply) = request.reply {
                let _ = reply.send(Err("policy-only test; no recording is started".into()));
            }
        }
    }));
    let shared = Arc::new(Shared {
        state,
        identity,
        trust: AsyncMutex::new(server_trust),
        catalog: Mutex::new(FileCatalog::new()),
        active: Mutex::new(None),
        generation: AtomicU64::new(1),
        dialing: AtomicBool::new(false),
        dial_cancel: watch::channel(0).0,
        dial_target: Mutex::new(None),
        pairing_attempts: Mutex::new(Budget::default()),
        pairing_windows: Mutex::new(Windows::default()),
        config_path: PathBuf::new(),
        mirror_cancel: Mutex::new(None),
        disconnected: Mutex::new(HashSet::new()),
        reconnect_paused: Mutex::new(HashSet::new()),
        mirror_generation: AtomicU64::new(1),
        allow_loopback: true,
        share_changes: Mutex::new(()),
        commands,
        capture_gate: Arc::new(Semaphore::new(1)),
        mirror_failure: Mutex::new(Arc::new(crate::capture::Failure::default())),
        file_gate: Arc::new(Semaphore::new(MAX_FILE_WORK)),
    });
    let (outbox, outgoing) = mpsc::channel(32);
    let (media_send, media) = mpsc::channel(8);
    let (cancel, _) = watch::channel(false);
    *shared.active.lock().unwrap() = Some(Active {
        generation: 1,
        peer_id: client_identity.device_id(),
        preferred: true,
        cancel,
        outbox,
        media: media_send,
        pair_input: None,
        authorized: true,
    });
    let server = tokio::spawn(serve(shared.clone(), server, outgoing, media, 1));
    let (reader, writer) = client.into_split().unwrap();
    let (incoming, read_task) = receiver(reader);
    (shared, writer, incoming, read_task, server, responder)
}

struct HeldCatalog {
    release: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for HeldCatalog {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

async fn hold_catalog(shared: Arc<Shared>) -> HeldCatalog {
    let (release, released) = std::sync::mpsc::channel();
    let (ready, held) = oneshot::channel();
    let thread = std::thread::spawn(move || {
        let _guard = shared.catalog.lock().unwrap();
        let _ = ready.send(());
        let _ = released.recv_timeout(Duration::from_secs(10));
    });
    held.await.unwrap();
    HeldCatalog {
        release: Some(release),
        thread: Some(thread),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_file_work_keeps_remote_mirror_controls_responsive() {
    let (shared, mut writer, mut incoming, _read, server, _commands) = isolated_file_server().await;
    let held = hold_catalog(shared).await;
    writer
        .send(&Message::FileRequest {
            v: VERSION,
            id: 1,
            request: FileRequest::Shares,
        })
        .await
        .unwrap();
    writer
        .send(&Message::MirrorQuery { v: VERSION, id: 2 })
        .await
        .unwrap();
    writer
        .send(&Message::MirrorControl {
            v: VERSION,
            id: 3,
            command: MirrorAction::Stop,
        })
        .await
        .unwrap();
    for expected in [2, 3] {
        let message =
            tokio::time::timeout(Duration::from_millis(750), next(&mut incoming, &mut writer))
                .await
                .expect("slow file work blocked remote mirror controls");
        assert!(matches!(message, Message::MirrorReply { id, ok: false, .. } if id == expected));
    }
    drop(held);
    assert!(matches!(
        next(&mut incoming, &mut writer).await,
        Message::FileResponse {
            id: 1,
            response: FileResponse::Shares { .. },
            ..
        }
    ));
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_work_is_bounded_and_cancelled_requests_do_not_emit_chunks() {
    let (shared, mut writer, mut incoming, _read, server, _commands) = isolated_file_server().await;
    let directory =
        std::env::temp_dir().join(format!("ymlink-serve-files-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("old"), b"old contents").unwrap();
    std::fs::write(directory.join("new"), b"new contents").unwrap();
    let versions = {
        let mut catalog = shared.catalog.lock().unwrap();
        catalog
            .add("share".into(), "test".into(), &directory)
            .unwrap();
        ["old", "new"].map(|path| {
            match catalog.handle(FileRequest::Stat {
                share_id: "share".into(),
                path: path.into(),
            }) {
                FileResponse::Stat { entry } => entry.version.unwrap(),
                _ => panic!("expected version"),
            }
        })
    };
    let held = hold_catalog(shared.clone()).await;
    writer
        .send(&Message::TransferOpen {
            v: VERSION,
            id: "reuse".into(),
            share_id: "share".into(),
            path: "old".into(),
            version: versions[0].clone(),
            offset: 0,
        })
        .await
        .unwrap();
    writer
        .send(&Message::TransferAction {
            v: VERSION,
            id: "reuse".into(),
            action: "cancel".into(),
        })
        .await
        .unwrap();
    writer
        .send(&Message::TransferOpen {
            v: VERSION,
            id: "fresh".into(),
            share_id: "share".into(),
            path: "new".into(),
            version: versions[1].clone(),
            offset: 0,
        })
        .await
        .unwrap();
    for id in 10..13 {
        writer
            .send(&Message::FileRequest {
                v: VERSION,
                id,
                request: FileRequest::Shares,
            })
            .await
            .unwrap();
    }
    let busy = tokio::time::timeout(Duration::from_millis(750), next(&mut incoming, &mut writer))
        .await
        .expect("full file work must be rejected without waiting for disk");
    assert!(matches!(
        busy,
        Message::FileResponse {
            id: 12,
            response: FileResponse::Error {
                code: ymlink_core::files::FileError::Unavailable
            },
            ..
        }
    ));
    assert_eq!(
        shared
            .state
            .lock()
            .unwrap()
            .transfers
            .iter()
            .find(|t| t.id == "fresh")
            .unwrap()
            .status,
        "transferring"
    );
    drop(held);
    let mut responses = 0;
    let mut chunks = 0;
    while responses != 2 || chunks != 1 {
        match next(&mut incoming, &mut writer).await {
            Message::FileResponse {
                id: 10 | 11,
                response: FileResponse::Shares { .. },
                ..
            } => responses += 1,
            Message::TransferChunk {
                id,
                version,
                data,
                offset: 0,
                eof: true,
                ..
            } => {
                assert_eq!(id, "fresh");
                assert_eq!(version, versions[1]);
                assert_eq!(data, b"new contents");
                chunks += 1;
                writer
                    .send(&Message::TransferAck {
                        v: VERSION,
                        id,
                        offset: data.len() as u64,
                    })
                    .await
                    .unwrap();
            }
            _ => panic!("cancelled file work emitted a chunk"),
        }
    }
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    shared.catalog.lock().unwrap().remove("share");
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_file_waiters_keeps_io_slots_until_disk_work_finishes() {
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    let held = hold_catalog(shared.clone()).await;
    let mut old = FileWork::new();
    for id in 0..MAX_FILE_WORK {
        file_request(
            &shared,
            &mut old,
            FileTarget::Request(id as u64),
            FileRequest::Shares,
        );
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        while shared.file_gate.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    old.abort_all();
    while old.join_next().await.is_some() {}
    assert_eq!(
        shared.file_gate.available_permits(),
        0,
        "cancelled physical IO still owns all four slots"
    );
    let mut new = FileWork::new();
    file_request(
        &shared,
        &mut new,
        FileTarget::Request(9),
        FileRequest::Shares,
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(30), new.join_next())
            .await
            .is_err()
    );
    drop(held);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), new.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .unwrap(),
        (FileTarget::Request(9), FileResponse::Shares { .. })
    ));
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_socket_writer_does_not_block_remote_stop_dispatch() {
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    let media = shared
        .active
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .media
        .clone();
    let mut stalled = false;
    for packet_id in 0..512 {
        let message = Message::Media {
            v: VERSION,
            stream_id: 1,
            packet_id,
            part: 0,
            parts: 1,
            media_kind: "video".into(),
            codec: "h264-avcc".into(),
            pts_us: 0,
            keyframe: true,
            width: 64,
            height: 64,
            data: vec![255; 8192],
        };
        if tokio::time::timeout(Duration::from_millis(100), media.send(message))
            .await
            .is_err()
        {
            stalled = true;
            break;
        }
    }
    assert!(
        stalled,
        "small TCP buffers and an unread receive queue must exercise writer backpressure"
    );
    writer
        .send(&Message::MirrorControl {
            v: VERSION,
            id: 90,
            command: MirrorAction::Stop,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_millis(750), async {
        while shared.state.lock().unwrap().error.as_deref() != Some("test mirror command processed")
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("socket backpressure blocked remote stop dispatch");
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retired_transfer_ack_is_ignored_but_reopening_its_id_is_rejected() {
    let (shared, mut writer, mut incoming, _read, mut server, _commands) =
        isolated_file_server().await;
    let directory =
        std::env::temp_dir().join(format!("ymlink-transfer-id-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&directory).unwrap();
    let content = b"actual file bytes";
    std::fs::write(directory.join("sample"), content).unwrap();
    let version = {
        let mut catalog = shared.catalog.lock().unwrap();
        catalog
            .add("share".into(), "test".into(), &directory)
            .unwrap();
        match catalog.handle(FileRequest::Stat {
            share_id: "share".into(),
            path: "sample".into(),
        }) {
            FileResponse::Stat { entry } => entry.version.unwrap(),
            _ => panic!("expected file version"),
        }
    };
    let open = |id: &str| Message::TransferOpen {
        v: VERSION,
        id: id.into(),
        share_id: "share".into(),
        path: "sample".into(),
        version: version.clone(),
        offset: 0,
    };
    writer.send(&open("retired")).await.unwrap();
    assert!(
        matches!(next(&mut incoming, &mut writer).await, Message::TransferChunk { data, eof: true, .. } if data == content)
    );
    writer
        .send(&Message::TransferAction {
            v: VERSION,
            id: "retired".into(),
            action: "cancel".into(),
        })
        .await
        .unwrap();
    writer
        .send(&Message::TransferAck {
            v: VERSION,
            id: "retired".into(),
            offset: content.len() as u64,
        })
        .await
        .unwrap();
    writer
        .send(&Message::FileRequest {
            v: VERSION,
            id: 90,
            request: FileRequest::Shares,
        })
        .await
        .unwrap();
    assert!(matches!(
        next(&mut incoming, &mut writer).await,
        Message::FileResponse { id: 90, .. }
    ));
    writer
        .send(&Message::TransferOpen {
            v: VERSION,
            id: "failed".into(),
            share_id: "share".into(),
            path: "missing".into(),
            version: version.clone(),
            offset: 0,
        })
        .await
        .unwrap();
    assert!(
        matches!(next(&mut incoming, &mut writer).await, Message::TransferError { id, .. } if id == "failed")
    );
    shared
        .active
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .outbox
        .try_send(Message::TransferAction {
            v: VERSION,
            id: "retired".into(),
            action: "pause".into(),
        })
        .unwrap();
    shared
        .active
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .outbox
        .try_send(Message::TransferAction {
            v: VERSION,
            id: "failed".into(),
            action: "resume".into(),
        })
        .unwrap();
    writer.send(&open("fresh")).await.unwrap();
    assert!(
        matches!(next(&mut incoming, &mut writer).await, Message::TransferChunk { id, data, eof: true, .. } if id == "fresh" && data == content)
    );
    writer
        .send(&Message::TransferAck {
            v: VERSION,
            id: "fresh".into(),
            offset: content.len() as u64,
        })
        .await
        .unwrap();
    writer
        .send(&Message::FileRequest {
            v: VERSION,
            id: 91,
            request: FileRequest::Shares,
        })
        .await
        .unwrap();
    assert!(matches!(
        next(&mut incoming, &mut writer).await,
        Message::FileResponse { id: 91, .. }
    ));
    assert_eq!(
        shared.state.lock().unwrap().error.as_deref(),
        Some("transfer no longer active")
    );
    assert_eq!(
        shared
            .state
            .lock()
            .unwrap()
            .transfers
            .iter()
            .find(|t| t.id == "failed")
            .unwrap()
            .status,
        "failed"
    );
    writer.send(&open("retired")).await.unwrap();
    let rejected = tokio::time::timeout(Duration::from_secs(1), &mut server).await;
    if rejected.is_err() {
        server.abort();
    }
    shared.catalog.lock().unwrap().remove("share");
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        matches!(rejected, Ok(Ok(Err(session::Error::Protocol(_))))),
        "retired id must never bind an old Ack or queued control to a new transfer"
    );
}

#[test]
fn transfer_id_history_has_a_hard_session_limit() {
    let mut ids = HashSet::new();
    for index in 0..MAX_TRANSFER_IDS {
        remember_transfer_id(&mut ids, &format!("transfer-{index}")).unwrap();
    }
    assert_eq!(ids.len(), MAX_TRANSFER_IDS);
    assert!(remember_transfer_id(&mut ids, "transfer-0").is_err());
    assert!(remember_transfer_id(&mut ids, "one-too-many").is_err());
    assert_eq!(ids.len(), MAX_TRANSFER_IDS);
}

#[test]
fn repeated_snapshot_errors_have_distinct_event_ids() {
    let mut state = Snapshot::new("test".into());
    assert_eq!(state.error_id, 0);
    state.set_error("folder is already shared");
    assert_eq!(state.error_id, 1);
    state.set_error("folder is already shared");
    assert_eq!(state.error_id, 2);
    state.error = None;
    state.set_error("folder is already shared");
    assert_eq!(state.error_id, 3);
    assert_eq!(serde_json::to_value(&state).unwrap()["error_id"], 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_dial_checks_the_actual_endpoint_before_untrusted_advertised_port() {
    let (shared, mut writer, _incoming, _read, server, _commands) = isolated_file_server().await;
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    server.await.unwrap().unwrap();
    *shared.active.lock().unwrap() = None;
    shared.edit(|state| {
        state.listen_port = 32000;
        state.error = None;
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let pinned = Identity::generate().unwrap();
    let pinned_hello = Hello::new(&pinned, "pinned Mac", "macos", address.port());
    shared
        .trust
        .lock()
        .await
        .remember_endpoint(
            &pinned_hello,
            &pinned.public_key(),
            Some(address.to_string()),
        )
        .unwrap();
    let changed = Identity::generate().unwrap();
    let advertised_port = if address.port() == 65535 {
        1
    } else {
        address.port() + 1
    };
    let client = TcpStream::connect(address).await.unwrap();
    let app = shared.clone();
    let connection_task = tokio::spawn(async move {
        connection(app, client, address, true, None, true).await;
    });
    let (socket, _) = listener.accept().await.unwrap();
    let _remote = SecureSession::handshake(
        socket,
        false,
        &changed,
        Hello::new(&changed, "replacement", "headset-test", advertised_port),
    )
    .await
    .unwrap();
    let rejected = tokio::time::timeout(Duration::from_millis(750), async {
        loop {
            if shared.state.lock().unwrap().error.as_deref()
                == Some("paired identity key changed; forget the old device before pairing again")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    connection_task.abort();
    let _ = connection_task.await;
    assert!(
        rejected.is_ok(),
        "changed identity at the dialed endpoint was not rejected before pairing"
    );
    assert!(shared.state.lock().unwrap().pairing.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_history_never_evicts_a_paused_transfer() {
    let (shared, mut writer, mut incoming, _read, server, _commands) = isolated_file_server().await;
    let directory =
        std::env::temp_dir().join(format!("ymlink-history-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("long"), vec![7; 32_768]).unwrap();
    std::fs::write(directory.join("short"), b"x").unwrap();
    let versions = {
        let mut catalog = shared.catalog.lock().unwrap();
        catalog
            .add("share".into(), "test".into(), &directory)
            .unwrap();
        ["long", "short"].map(|path| {
            match catalog.handle(FileRequest::Stat {
                share_id: "share".into(),
                path: path.into(),
            }) {
                FileResponse::Stat { entry } => entry.version.unwrap(),
                _ => panic!("expected test file metadata"),
            }
        })
    };
    let open = |id: String, path: &str, version: &str| Message::TransferOpen {
        v: VERSION,
        id,
        share_id: "share".into(),
        path: path.into(),
        version: version.into(),
        offset: 0,
    };
    writer
        .send(&open("long".into(), "long", &versions[0]))
        .await
        .unwrap();
    assert!(
        matches!(next(&mut incoming, &mut writer).await, Message::TransferChunk { id, offset: 0, data, .. } if id == "long" && data.len() == 16_384)
    );
    writer
        .send(&Message::TransferAction {
            v: VERSION,
            id: "long".into(),
            action: "pause".into(),
        })
        .await
        .unwrap();
    writer
        .send(&Message::TransferAck {
            v: VERSION,
            id: "long".into(),
            offset: 16_384,
        })
        .await
        .unwrap();
    for index in 0..128 {
        let id = format!("short-{index}");
        writer
            .send(&open(id.clone(), "short", &versions[1]))
            .await
            .unwrap();
        assert!(
            matches!(next(&mut incoming, &mut writer).await, Message::TransferChunk { id: received, data, eof: true, .. } if received == id && data == b"x")
        );
        writer
            .send(&Message::TransferAck {
                v: VERSION,
                id,
                offset: 1,
            })
            .await
            .unwrap();
    }
    let retained = shared
        .state
        .lock()
        .unwrap()
        .transfers
        .iter()
        .any(|t| t.id == "long" && t.status == "paused");
    if retained {
        writer
            .send(&Message::TransferAction {
                v: VERSION,
                id: "long".into(),
                action: "resume".into(),
            })
            .await
            .unwrap();
        assert!(
            matches!(next(&mut incoming, &mut writer).await, Message::TransferChunk { id, offset: 16_384, data, eof: true, .. } if id == "long" && data == vec![7; 16_384])
        );
        writer
            .send(&Message::TransferAck {
                v: VERSION,
                id: "long".into(),
                offset: 32_768,
            })
            .await
            .unwrap();
    }
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    shared.catalog.lock().unwrap().remove("share");
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        retained,
        "completed history evicted the paused transfer and stalled its live job"
    );
    assert!(shared.state.lock().unwrap().transfers.len() <= 128);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unfinished_history_refuses_new_jobs_without_dropping_records_or_connection() {
    let (shared, mut writer, mut incoming, _read, server, _commands) = isolated_file_server().await;
    shared.edit(|s| {
        s.transfers = (0..128)
            .map(|index| Transfer {
                id: format!("waiting-{index}"),
                name: "sample".into(),
                transferred: 0,
                total: 1,
                status: "waiting".into(),
            })
            .collect()
    });
    let open = |id: &str| Message::TransferOpen {
        v: VERSION,
        id: id.into(),
        share_id: "missing".into(),
        path: "sample".into(),
        version: "version".into(),
        offset: 0,
    };
    writer.send(&open("new")).await.unwrap();
    assert!(
        matches!(next(&mut incoming, &mut writer).await, Message::TransferError { id, code, .. } if id == "new" && code == "too many unfinished transfers; cancel an interrupted transfer")
    );
    assert_eq!(shared.state.lock().unwrap().transfers.len(), 128);
    transfer_command(&shared, "waiting-0".into(), "cancel")
        .await
        .unwrap();
    writer.send(&open("after-cancel")).await.unwrap();
    assert!(
        matches!(next(&mut incoming, &mut writer).await, Message::TransferError { id, code, .. } if id == "after-cancel" && code != "too many unfinished transfers; cancel an interrupted transfer")
    );
    assert!(shared
        .state
        .lock()
        .unwrap()
        .transfers
        .iter()
        .any(|t| t.id == "waiting-127"));
    writer
        .send(&Message::FileRequest {
            v: VERSION,
            id: 900,
            request: FileRequest::Shares,
        })
        .await
        .unwrap();
    assert!(matches!(
        next(&mut incoming, &mut writer).await,
        Message::FileResponse { id: 900, .. }
    ));
    writer.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preferred_connection_handoff_preserves_interrupted_transfer_controls() {
    async fn attach(
        shared: Arc<Shared>,
        peer: &Identity,
        initiator: bool,
    ) -> (
        session::SecureReader,
        session::SecureWriter,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (accepted, _) = listener.accept().await.unwrap();
        let (local, remote) = if initiator {
            (client, accepted)
        } else {
            (accepted, client)
        };
        let address = local.peer_addr().unwrap();
        let task = tokio::spawn(connection(
            shared.clone(),
            local,
            address,
            initiator,
            None,
            false,
        ));
        let mut session = SecureSession::handshake(
            remote,
            !initiator,
            peer,
            Hello::new(peer, "Quest test", "headset-test", 32009),
        )
        .await
        .unwrap();
        let mut trust = TrustStore::memory();
        trust
            .remember(session.remote(), &session.remote_key())
            .unwrap();
        session
            .authorize(&mut trust, Pairing::ReconnectOnly)
            .await
            .unwrap();
        let preferred = initiator == (shared.identity.device_id() < peer.device_id());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if shared
                    .active
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|active| active.authorized && active.preferred == preferred)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let (read, write) = session.into_split().unwrap();
        (read, write, task)
    }

    let (shared, mut fixture_writer, _incoming, _read, fixture_server, _commands) =
        isolated_file_server().await;
    fixture_writer
        .send(&Message::Bye { v: VERSION })
        .await
        .unwrap();
    fixture_server.await.unwrap().unwrap();
    *shared.active.lock().unwrap() = None;
    shared.edit(|state| state.listen_port = 32000);
    let peer = Identity::generate().unwrap();
    shared
        .trust
        .lock()
        .await
        .remember(
            &Hello::new(&peer, "Quest test", "headset-test", 32009),
            &peer.public_key(),
        )
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("ymlink-handoff-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("long"), vec![7u8; 32_768]).unwrap();
    let version = {
        let mut catalog = shared.catalog.lock().unwrap();
        catalog
            .add("share".into(), "test".into(), &directory)
            .unwrap();
        match catalog.handle(FileRequest::Stat {
            share_id: "share".into(),
            path: "long".into(),
        }) {
            FileResponse::Stat { entry } => entry.version.unwrap(),
            _ => panic!("expected test file metadata"),
        }
    };
    let preferred_initiator = shared.identity.device_id() < peer.device_id();
    let (read, mut write, old_connection) =
        attach(shared.clone(), &peer, !preferred_initiator).await;
    let (mut incoming, _reader) = receiver(read);
    write
        .send(&Message::TransferOpen {
            v: VERSION,
            id: "long".into(),
            share_id: "share".into(),
            path: "long".into(),
            version,
            offset: 0,
        })
        .await
        .unwrap();
    assert!(matches!(
        next(&mut incoming, &mut write).await,
        Message::TransferChunk {
            offset: 0,
            data,
            eof: false,
            ..
        } if data.len() == 16_384
    ));
    write
        .send(&Message::TransferAction {
            v: VERSION,
            id: "long".into(),
            action: "pause".into(),
        })
        .await
        .unwrap();
    write
        .send(&Message::TransferAck {
            v: VERSION,
            id: "long".into(),
            offset: 16_384,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if shared
                .state
                .lock()
                .unwrap()
                .transfers
                .iter()
                .any(|transfer| {
                    transfer.id == "long"
                        && transfer.status == "paused"
                        && transfer.transferred == 16_384
                })
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    // A Hello-only identification from the same identity in the preferred
    // direction must not steal this active connection or its real paused work.
    let active_generation = shared.active.lock().unwrap().as_ref().unwrap().generation;
    let mirror_generation = shared.mirror_generation.load(Ordering::Acquire);
    let before_probe = serde_json::to_value(&*shared.state.lock().unwrap()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (accepted, _) = listener.accept().await.unwrap();
    let (local, remote) = if preferred_initiator {
        (client, accepted)
    } else {
        (accepted, client)
    };
    let address = local.peer_addr().unwrap();
    let probe_task = tokio::spawn(connection(
        shared.clone(),
        local,
        address,
        preferred_initiator,
        None,
        false,
    ));
    let probe = SecureSession::handshake(
        remote,
        !preferred_initiator,
        &peer,
        Hello::new(&peer, "Quest test", "headset-test", 32009),
    )
    .await
    .unwrap();
    drop(probe);
    tokio::time::timeout(Duration::from_secs(1), probe_task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        shared.active.lock().unwrap().as_ref().unwrap().generation,
        active_generation
    );
    assert_eq!(
        shared.mirror_generation.load(Ordering::Acquire),
        mirror_generation
    );
    assert_eq!(
        serde_json::to_value(&*shared.state.lock().unwrap()).unwrap(),
        before_probe
    );
    assert!(!old_connection.is_finished());

    // Complete the competing preferred connection only after the original
    // connection has real paused work. No capture or OS permissions are used.
    let (_new_read, mut new_write, new_connection) =
        attach(shared.clone(), &peer, preferred_initiator).await;
    tokio::time::timeout(Duration::from_secs(2), old_connection)
        .await
        .unwrap()
        .unwrap();
    let interrupted = shared.state.lock().unwrap().transfers[0].status.clone();
    transfer_command(&shared, "long".into(), "cancel")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let state = shared.state.lock().unwrap();
            if state.transfers[0].status == "cancelled"
                || state.error.as_deref() == Some("transfer no longer active")
            {
                break;
            }
            drop(state);
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let cancelled = shared.state.lock().unwrap().transfers[0].status.clone();
    new_write.send(&Message::Bye { v: VERSION }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), new_connection)
        .await
        .unwrap()
        .unwrap();
    shared.catalog.lock().unwrap().remove("share");
    std::fs::remove_dir_all(directory).unwrap();
    assert_eq!(interrupted, "waiting");
    assert_eq!(cancelled, "cancelled");
}

fn anonymous_peer(ip: &str, port: u16) -> Peer {
    let address = format!("{ip}:{port}");
    Peer {
        id: address.clone(),
        address,
        name: String::new(),
        platform: String::new(),
        status: "discovered".into(),
        trusted: false,
    }
}

#[test]
fn discovery_port_rotation_replaces_one_anonymous_candidate_instead_of_filling_the_list() {
    let mut state = Snapshot::new("test".into());
    for port in 30000..30140 {
        record_discovered_peer(
            &mut state,
            format!("192.0.2.8:{port}").parse().expect("address"),
            None,
        );
        assert_eq!(state.peers.len(), 1);
        assert_eq!(state.peers[0].address, format!("192.0.2.8:{port}"));
        assert!(!state.peers[0].trusted);
    }
    record_discovered_peer(
        &mut state,
        "192.0.2.9:31000".parse().expect("address"),
        None,
    );
    assert_eq!(state.peers.len(), 2);
}

#[test]
fn discovery_keeps_authenticated_identity_and_latest_reconnect_route_without_stale_anonymous_rows()
{
    let mut state = Snapshot::new("test".into());
    let mut connected = anonymous_peer("192.0.2.8", 30000);
    connected.id = "authenticated-key-id".into();
    connected.trusted = true;
    connected.status = "connected".into();
    state.peers = vec![
        connected,
        anonymous_peer("192.0.2.8", 30001),
        anonymous_peer("192.0.2.9", 30002),
    ];
    record_discovered_peer(
        &mut state,
        "192.0.2.8:30003".parse().expect("address"),
        Some("authenticated-key-id"),
    );
    assert_eq!(state.peers.len(), 2);
    assert_eq!(state.peers[0].address, "192.0.2.8:30003");
    assert_eq!(state.peers[0].status, "connected");
    assert!(state.peers[0].trusted);
    assert_eq!(state.peers[0].id, "authenticated-key-id");
    // After the connection closes there may be no new discovery event: the
    // next dial must still use the latest advertised route.
    state.peers[0].status = "disconnected".into();
    assert_eq!(state.peers[0].address, "192.0.2.8:30003");
}

#[test]
fn authentication_promotes_identity_and_cleans_all_old_ports_without_removing_other_trust() {
    let mut state = Snapshot::new("test".into());
    let mut old = anonymous_peer("192.0.2.8", 29999);
    old.id = "other-key-id".into();
    old.trusted = true;
    state.peers = vec![
        anonymous_peer("192.0.2.8", 30001),
        anonymous_peer("192.0.2.8", 30002),
        old,
    ];
    let mut peer = anonymous_peer("192.0.2.8", 30003);
    peer.id = "new-key-id".into();
    peer.trusted = true;
    peer.status = "connected".into();
    publish_authenticated_peer(&mut state, peer);
    assert_eq!(state.peers.len(), 2);
    assert!(state.peers.iter().all(|p| p.trusted));
    assert!(state.peers.iter().any(|p| p.id == "other-key-id"));
}

#[test]
fn discovery_refreshes_a_disconnected_trusted_route_without_giving_anonymous_peers_trust() {
    let mut state = Snapshot::new("test".into());
    let mut known = anonymous_peer("192.0.2.8", 30000);
    known.id = "known-key".into();
    known.trusted = true;
    known.status = "disconnected".into();
    state.peers.push(known);
    record_discovered_peer(
        &mut state,
        "192.0.2.8:30001".parse().expect("address"),
        Some("known-key"),
    );
    assert_eq!(state.peers[0].address, "192.0.2.8:30001");
    assert_eq!(state.peers[0].status, "discovered");
    assert_eq!(state.peers.len(), 1);
    record_discovered_peer(
        &mut state,
        "192.0.2.9:30002".parse().expect("address"),
        None,
    );
    assert!(!state.peers[1].trusted);
}
