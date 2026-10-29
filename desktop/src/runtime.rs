use crate::model::{Command, PairingView, Peer, Share, Snapshot, Transfer};
use crate::pairing::{Budget, EnterTarget, EnterWindow, Windows};
use crate::wire::{Message, MirrorAction, MirrorInfo, VERSION};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch, Mutex as AsyncMutex, Semaphore};
use ymlink_core::discovery::{self, Discovery};
use ymlink_core::files::{FileCatalog, FileRequest, FileResponse, MAX_READ_BYTES};
use ymlink_core::identity::{Identity, TrustStore};
use ymlink_core::session::{self, AuthErrorCode, Hello, Pairing, SecureSession};

pub struct Backend {
    runtime: Option<tokio::runtime::Runtime>,
    commands: mpsc::Sender<Request>,
    state: Arc<Mutex<Snapshot>>,
}

impl Backend {
    /// Starts the backend runtime for the given state directory and device name.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the device name is invalid or the
    /// async runtime cannot start; startup failures inside the worker surface
    /// through the snapshot instead.
    pub fn start(path: PathBuf, name: String) -> Result<Self, String> {
        Self::start_inner(path, name, None, false)
    }

    fn start_inner(
        path: PathBuf,
        name: String,
        identity: Option<Identity>,
        allow_loopback: bool,
    ) -> Result<Self, String> {
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err("invalid device name".into());
        }
        let state = Arc::new(Mutex::new(Snapshot::new(name)));
        let (commands, receiver) = mpsc::channel(64);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| "cannot start backend runtime")?;
        let state_task = Arc::clone(&state);
        let worker_commands = commands.clone();
        runtime.spawn(async move {
            if let Err(error) = run(
                path,
                Arc::clone(&state_task),
                receiver,
                identity,
                allow_loopback,
                worker_commands,
            )
            .await
            {
                let mut state = state_task
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.status = "error".into();
                state.set_error(error);
            }
        });
        Ok(Self {
            runtime: Some(runtime),
            commands,
            state,
        })
    }

    /// Submits a JSON command to the worker without waiting for it to finish.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the input is too large, is not a
    /// valid command, or the backend queue is full or closed.
    pub fn command(&self, input: &str) -> Result<(), String> {
        if input.len() > 16 * 1024 {
            return Err("command exceeds limit".into());
        }
        let command: Command = serde_json::from_str(input).map_err(|_| "invalid command")?;
        self.commands
            .try_send(Request {
                command,
                reply: None,
                generation: None,
            })
            .map_err(|_| "backend busy or unavailable".into())
    }

    pub fn snapshot(&self) -> String {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pairing) = &mut state.pairing {
            pairing.expires_in = pairing
                .deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u64;
            if let Some(deadline) = pairing.cooldown_deadline {
                pairing.retry_after_ms = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(60_000) as u32;
            }
        }
        serde_json::to_string(&*state)
            .unwrap_or_else(|_| json!({"status":"error","error":"cannot encode state"}).to_string())
    }
}

struct Request {
    command: Command,
    reply: Option<oneshot::Sender<Result<(), String>>>,
    generation: Option<u64>,
}

#[derive(Clone, Copy)]
enum MirrorEnd {
    Stopped,
    Paused,
}
impl MirrorEnd {
    fn status(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Paused => "paused",
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // Bounded shutdown also cancels listener, reader and reconnect tasks.
            runtime.shutdown_timeout(Duration::from_secs(3));
        }
    }
}

struct Active {
    generation: u64,
    peer_id: String,
    preferred: bool,
    cancel: watch::Sender<bool>,
    outbox: mpsc::Sender<Message>,
    media: mpsc::Sender<Message>,
    pair_input: Option<oneshot::Sender<String>>,
    authorized: bool,
}

struct DialTarget {
    address: SocketAddr,
    expected: Option<String>,
}

struct Shared {
    state: Arc<Mutex<Snapshot>>,
    identity: Identity,
    trust: AsyncMutex<TrustStore>,
    catalog: Mutex<FileCatalog>,
    active: Mutex<Option<Active>>,
    generation: AtomicU64,
    dialing: AtomicBool,
    dial_cancel: watch::Sender<u64>,
    dial_target: Mutex<Option<DialTarget>>,
    pairing_attempts: Mutex<Budget>,
    pairing_windows: Mutex<Windows>,
    config_path: PathBuf,
    mirror_cancel: Mutex<Option<watch::Sender<Option<MirrorEnd>>>>,
    disconnected: Mutex<HashSet<String>>,
    mirror_generation: AtomicU64,
    allow_loopback: bool,
    share_changes: Mutex<()>,
    commands: mpsc::Sender<Request>,
    capture_gate: Arc<Semaphore>,
    mirror_failure: Mutex<Arc<crate::capture::Failure>>,
    file_gate: Arc<Semaphore>,
}

impl Shared {
    fn pending_dial_matches(&self, id: &str) -> bool {
        self.dial_target
            .lock()
            .expect("lock poisoned")
            .as_ref()
            .is_some_and(|target| {
                target.expected.as_ref().map_or_else(
                    || {
                        self.state
                            .lock()
                            .expect("lock poisoned")
                            .peers
                            .iter()
                            .any(|p| {
                                p.id == id
                                    && p.address.parse::<SocketAddr>().ok() == Some(target.address)
                            })
                    },
                    |expected| expected == id,
                )
            })
    }
    fn edit(&self, edit: impl FnOnce(&mut Snapshot)) {
        edit(
            &mut self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
    fn error(&self, error: impl Into<String>) {
        self.edit(|s| s.set_error(error));
    }
    fn dial_error(&self, address: SocketAddr, error: String, epoch: Option<u64>) {
        let active = self.active.lock().expect("lock poisoned");
        if epoch.is_some_and(|epoch| epoch != *self.dial_cancel.borrow()) {
            return;
        }
        if active.is_some() {
            return;
        }
        let mut windows = self.pairing_windows.lock().expect("lock poisoned");
        if let Some(target) = windows
            .target
            .as_ref()
            .filter(|w| w.address == address)
            .cloned()
        {
            windows.invalidate_peer(&ymlink_core::identity::device_id(&target.peer_key));
            self.edit(|s| {
                if let Some(view) = s.pairing.as_mut().filter(|p| p.mode == "enter") {
                    view.stage = "disconnected".into();
                    view.error = Some(
                        if error.contains("incompatible application version") {
                            "version"
                        } else {
                            "disconnected"
                        }
                        .into(),
                    );
                    view.code = None;
                } else {
                    s.set_error(error);
                }
            });
        } else {
            self.error(error);
        }
    }
    fn stop_mirror(&self) {
        self.end_mirror(MirrorEnd::Stopped);
    }
    fn end_mirror(&self, reason: MirrorEnd) {
        let mut state = self.state.lock().expect("lock poisoned");
        self.end_mirror_locked(&mut state, reason);
    }
    fn end_mirror_locked(&self, state: &mut Snapshot, reason: MirrorEnd) {
        self.mirror_generation.fetch_add(1, Ordering::AcqRel);
        if let Some(cancel) = self.mirror_cancel.lock().expect("lock poisoned").take() {
            let _ = cancel.send(Some(reason));
        }
        state.mirror.status = reason.status().into();
        state.mirror.fps = None;
        state.mirror.bitrate_mbps = None;
        state.mirror.error = None;
    }
    fn outgoing(&self) -> Option<mpsc::Sender<Message>> {
        self.active
            .lock()
            .expect("lock poisoned")
            .as_ref()
            .filter(|a| a.authorized && !*a.cancel.borrow())
            .map(|a| a.outbox.clone())
    }
    fn mirror_edit(&self, generation: u64, edit: impl FnOnce(&mut Snapshot)) {
        let mut state = self.state.lock().expect("lock poisoned");
        if self.mirror_generation.load(Ordering::Acquire) == generation {
            edit(&mut state);
        }
    }
    fn publish(&self, generation: u64, authorized: bool, edit: impl FnOnce(&mut Snapshot)) -> bool {
        let mut active = self.active.lock().expect("lock poisoned");
        let Some(active) = active
            .as_mut()
            .filter(|a| a.generation == generation && !*a.cancel.borrow())
        else {
            return false;
        };
        if authorized {
            active.authorized = true;
            self.pairing_windows
                .lock()
                .expect("lock poisoned")
                .clear_peer(&active.peer_id);
        }
        self.edit(edit);
        true
    }
    fn pairing_window<T>(
        &self,
        generation: u64,
        edit: impl FnOnce(&mut Windows) -> T,
    ) -> session::Result<T> {
        let active = self.active.lock().expect("lock poisoned");
        if !active
            .as_ref()
            .is_some_and(|a| a.generation == generation && !*a.cancel.borrow())
        {
            return Err(session::Error::Cancelled);
        }
        Ok(edit(
            &mut self.pairing_windows.lock().expect("lock poisoned"),
        ))
    }
    fn pairing_admission(&self, ip: IpAddr) -> Result<(), u32> {
        self.pairing_attempts
            .lock()
            .expect("lock poisoned")
            .admit(ip, Instant::now())
    }
}

async fn run(
    path: PathBuf,
    state: Arc<Mutex<Snapshot>>,
    mut commands: mpsc::Receiver<Request>,
    identity: Option<Identity>,
    allow_loopback: bool,
    command_sender: mpsc::Sender<Request>,
) -> Result<(), String> {
    private_directory(&path)?;
    let identity = match identity {
        Some(identity) => identity,
        None => Identity::load_or_create(&path).map_err(|e| e.to_string())?,
    };
    let trust = TrustStore::load(&path.join("trust.json")).map_err(|e| e.to_string())?;
    let mut catalog = FileCatalog::new();
    let config_path = path.join("shares.json");
    let shares: Vec<Share> = match std::fs::read(&config_path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "invalid shared folder configuration")?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(_) => return Err("cannot read shared folder configuration".into()),
    };
    if shares.len() > 64 {
        return Err("too many shared folders".into());
    }
    {
        let mut state = state.lock().expect("lock poisoned");
        for share in shares {
            if catalog
                .add(share.id.clone(), share.name.clone(), Path::new(&share.path))
                .is_ok()
            {
                state.shares.push(share);
            } else {
                state.set_error(
                    "a shared folder is unavailable; add it again after reconnecting the disk"
                        .to_string(),
                );
            }
        }
        for peer in trust.peers() {
            state.peers.push(Peer {
                id: peer.device_id,
                address: peer.endpoint.unwrap_or_default(),
                name: peer.name,
                platform: peer.platform,
                status: "disconnected".into(),
                trusted: true,
            });
        }
        state.displays = crate::capture::displays().unwrap_or_default();
        state.mirror.permission = if crate::capture::permission_granted() {
            "granted"
        } else {
            "unknown"
        }
        .into();
    }
    let listener = TcpListener::bind(("0.0.0.0", 0))
        .await
        .map_err(|_| "cannot listen for LAN connections")?;
    let port = listener
        .local_addr()
        .map_err(|_| "cannot inspect LAN listener")?
        .port();
    let (events_tx, mut events) = mpsc::channel(128);
    let discovery = Discovery::start(
        discovery::Config {
            advertised_port: Some(port),
            // Unit backends dial explicit TCP endpoints; avoid exhausting the
            // three production discovery ports when these tests run in parallel.
            listen_port: (cfg!(test) && allow_loopback).then_some(0),
            caps: vec!["control".into(), "files".into(), "mirror".into()],
            allow_loopback,
            ..Default::default()
        },
        events_tx,
    )
    .await
    .map_err(|e| e.to_string())?;
    {
        let mut state = state.lock().expect("lock poisoned");
        state.listen_port = port;
        state.discovery_port = discovery.listen_port();
        state.status = "searching".into();
    }
    let shared = Arc::new(Shared {
        state,
        identity,
        trust: AsyncMutex::new(trust),
        catalog: Mutex::new(catalog),
        active: Mutex::new(None),
        generation: AtomicU64::new(1),
        dialing: AtomicBool::new(false),
        dial_cancel: watch::channel(0).0,
        dial_target: Mutex::new(None),
        pairing_attempts: Mutex::new(Budget::default()),
        pairing_windows: Mutex::new(Windows::default()),
        config_path,
        mirror_cancel: Mutex::new(None),
        disconnected: Mutex::new(HashSet::new()),
        mirror_generation: AtomicU64::new(1),
        allow_loopback,
        share_changes: Mutex::new(()),
        commands: command_sender,
        capture_gate: Arc::new(Semaphore::new(1)),
        mirror_failure: Mutex::new(Arc::new(crate::capture::Failure::default())),
        file_gate: Arc::new(Semaphore::new(MAX_FILE_WORK)),
    });
    let slots = Arc::new(Semaphore::new(4));
    let _ = discovery.rescan().await;
    let mut reconnect = tokio::time::interval(Duration::from_secs(5));
    let mut reconnect_cursor = 0usize;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, address)) = accepted else { continue };
                if !lan_address(address.ip()) && (!shared.allow_loopback || !address.ip().is_loopback()) { continue; }
                let Ok(permit) = Arc::clone(&slots).try_acquire_owned() else { continue };
                let shared = Arc::clone(&shared);
                tokio::spawn(async move { let _permit = permit; connection(shared,stream,address,false,None,false).await; });
            }
            event = events.recv() => {
                if let Some(discovery::Event::PeerDiscovered(peer)) = event {
                    let address = SocketAddr::new(peer.addr, peer.listen_port);
                    let trusted = shared.trust.lock().await.peers().into_iter().find(|p| p.endpoint.as_ref().and_then(|e| e.parse::<SocketAddr>().ok()).is_some_and(|e| e.ip() == peer.addr));
                    shared.edit(|s| {
                        record_discovered_peer(s, address, trusted.as_ref().map(|p| p.device_id.as_str()));
                        if s.status == "searching" { s.status = "ready".into(); }
                    });
                } else if let Some(discovery::Event::PeerExpired(ip)) = event {
                    shared.edit(|s| { s.peers.retain(|p| p.trusted || p.address.parse::<SocketAddr>().map_or(true, |a| a.ip() != ip)); });
                }
            }
            Some(request) = commands.recv() => {
                let expired=request.reply.as_ref().is_some_and(tokio::sync::oneshot::Sender::is_closed);
                let authorized=request.generation.is_none_or(|generation|shared.active.lock().expect("lock poisoned").as_ref().is_some_and(|a|a.generation==generation && a.authorized && !*a.cancel.borrow()));
                let result=if expired{Err("request expired".into())}else if !authorized{Err("connection no longer active".into())}else{handle_command(Arc::clone(&shared), &discovery, request.command,request.generation).await};
                if let Some(reply)=request.reply {let _=reply.send(result);}else if let Err(error)=result{shared.error(error);}
            }
            _ = reconnect.tick() => {
                if shared.active.lock().expect("lock poisoned").is_none() && !shared.dialing.load(Ordering::Acquire) {
                    let state=shared.state.lock().expect("lock poisoned");
                    let has_trust=state.peers.iter().any(|p|p.trusted);
                    let candidates:Vec<_>=state.peers.iter().filter(|p| state.pairing.is_none() && !p.address.is_empty() && !shared.disconnected.lock().expect("lock poisoned").contains(&p.id) && (p.trusted || (has_trust && p.status=="discovered"))).cloned().collect();
                    drop(state);
                    if !candidates.is_empty() {
                        let peer = &candidates[reconnect_cursor % candidates.len()];
                        reconnect_cursor = reconnect_cursor.wrapping_add(1);
                        if let Ok(address) = peer.address.parse() { dial(Arc::clone(&shared), address,peer.trusted.then(||peer.id.clone()),false); }
                    }
                }
            }
        }
    }
}

fn lan_address(ip: IpAddr) -> bool {
    discovery::is_lan_source(ip, false)
}

// Anonymous discovery is routing information; authentication remains in the encrypted session.
fn same_discovery_host(peer: &Peer, address: SocketAddr) -> bool {
    peer.address
        .parse::<SocketAddr>()
        .is_ok_and(|old| old.ip() == address.ip())
}

fn record_discovered_peer(s: &mut Snapshot, address: SocketAddr, trusted: Option<&str>) {
    // The core discovery table also has one anonymous candidate per IP. A new
    // ephemeral listen port replaces the old route, without assigning identity.
    s.peers
        .retain(|p| p.trusted || !same_discovery_host(p, address));
    if let Some(known) = trusted {
        if let Some(item) = s.peers.iter_mut().find(|p| p.id == known) {
            // This is a future dial route, not the live session or a persisted
            // identity pin. Keep it fresh even while the old connection lives.
            item.address = address.to_string();
            if item.status != "connected" {
                item.status = "discovered".into();
            }
        }
    } else if s.peers.len() < 128 {
        s.peers.push(Peer {
            id: format!("discovery:{}", address.ip()),
            address: address.to_string(),
            name: String::new(),
            platform: String::new(),
            status: "discovered".into(),
            trusted: false,
        });
    }
}

fn publish_authenticated_peer(s: &mut Snapshot, peer: Peer) {
    let endpoint = peer.address.parse::<SocketAddr>().ok();
    s.peers.retain(|p| {
        p.id != peer.id && (p.trusted || !endpoint.is_some_and(|a| same_discovery_host(p, a)))
    });
    s.peers.push(peer);
}

fn dial(shared: Arc<Shared>, address: SocketAddr, expected: Option<String>, manual: bool) {
    if shared.dialing.swap(true, Ordering::AcqRel) {
        return;
    }
    let mut cancelled = shared.dial_cancel.subscribe();
    let epoch = *cancelled.borrow();
    *shared.dial_target.lock().expect("lock poisoned") = Some(DialTarget {
        address,
        expected: expected.clone(),
    });
    tokio::spawn(async move {
        let connected = tokio::select! {
            biased;
            _=cancelled.changed()=>None,
            result=tokio::time::timeout(Duration::from_secs(5),TcpStream::connect(address))=>Some(result),
        };
        match connected {
            Some(Ok(Ok(stream))) => {
                connection_guarded(
                    Arc::clone(&shared),
                    stream,
                    address,
                    true,
                    expected.clone(),
                    manual,
                    Some((epoch, cancelled)),
                )
                .await;
            }
            Some(_) if manual => {
                shared.dial_error(
                    address,
                    "cannot connect; confirm the other device is on the same Wi-Fi".into(),
                    Some(epoch),
                );
            }
            _ => {}
        }
        shared.dial_target.lock().expect("lock poisoned").take();
        shared.dialing.store(false, Ordering::Release);
    });
}

async fn connection(
    shared: Arc<Shared>,
    stream: TcpStream,
    address: SocketAddr,
    initiator: bool,
    expected: Option<String>,
    manual: bool,
) {
    connection_guarded(shared, stream, address, initiator, expected, manual, None).await;
}

async fn connection_guarded(
    shared: Arc<Shared>,
    stream: TcpStream,
    address: SocketAddr,
    initiator: bool,
    expected: Option<String>,
    manual: bool,
    mut dial_guard: Option<(u64, watch::Receiver<u64>)>,
) {
    let (name, port) = {
        let s = shared.state.lock().expect("lock poisoned");
        (s.device_name.clone(), s.listen_port)
    };
    let local = Hello::new(&shared.identity, name, std::env::consts::OS, port);
    let handshake = SecureSession::handshake(stream, initiator, &shared.identity, local);
    let result = if let Some((epoch, cancelled)) = &mut dial_guard {
        if *epoch != *cancelled.borrow() {
            return;
        }
        tokio::select! { biased; _=cancelled.changed()=>return, result=handshake=>result }
    } else {
        handshake.await
    };
    let session = match result {
        Ok(session) => session,
        Err(error) => {
            if initiator && manual {
                shared.dial_error(
                    address,
                    error.to_string(),
                    dial_guard.as_ref().map(|(epoch, _)| *epoch),
                );
            } else if matches!(&error, session::Error::Protocol(detail) if detail.contains("incompatible application version"))
            {
                let active = shared.active.lock().expect("lock poisoned");
                if active.is_none() {
                    shared.error(error.to_string());
                }
            }
            return;
        }
    };
    let remote = session.remote().clone();
    let endpoint = SocketAddr::new(address.ip(), remote.listen_port).to_string();
    {
        let trust = shared.trust.lock().await;
        if expected.as_ref().is_some_and(|id| *id != remote.device_id)
            || (initiator
                && trust
                    .check_endpoint(&remote, &session.remote_key(), &address.to_string())
                    .is_err())
            || trust
                .check_endpoint(&remote, &session.remote_key(), &endpoint)
                .is_err()
        {
            shared.error("paired identity key changed; forget the old device before pairing again");
            return;
        }
        // Anonymous discovery is only a hint. Probe a moved peer's encrypted
        // identity, but never start unsolicited pairing on an outbound retry.
        if initiator && !manual && !trust.check(&remote, &session.remote_key()).unwrap_or(false) {
            return;
        }
    }
    let (outbox, receiver) = mpsc::channel(32);
    let (media, media_receiver) = mpsc::channel(8);
    let (cancel, cancelled) = watch::channel(false);
    let generation;
    let preferred = initiator == (shared.identity.device_id() < remote.device_id);
    {
        let mut active = shared.active.lock().expect("lock poisoned");
        if dial_guard
            .as_ref()
            .is_some_and(|(epoch, guard)| *epoch != *guard.borrow())
        {
            return;
        }
        if manual {
            shared
                .disconnected
                .lock()
                .expect("lock poisoned")
                .remove(&remote.device_id);
        } else if shared
            .disconnected
            .lock()
            .expect("lock poisoned")
            .contains(&remote.device_id)
        {
            return;
        }
        if let Some(old) = active.as_ref() {
            if old.peer_id != remote.device_id || old.preferred || !preferred {
                return;
            }
            let _ = old.cancel.send(true);
            // Handoff owns the active gate: old serve writers cannot publish
            // after these records become interrupted and the new owner starts.
            shared.edit(Snapshot::interrupt_transfers);
        }
        generation = shared.generation.fetch_add(1, Ordering::AcqRel);
        *active = Some(Active {
            generation,
            peer_id: remote.device_id.clone(),
            preferred,
            cancel,
            outbox,
            media,
            pair_input: None,
            authorized: false,
        });
    }
    shared.stop_mirror();
    let result = authorize_and_serve(
        Arc::clone(&shared),
        generation,
        session,
        &remote,
        &endpoint,
        initiator,
        receiver,
        media_receiver,
        !initiator || manual,
        cancelled,
    )
    .await;
    let mut active = shared.active.lock().expect("lock poisoned");
    let mut retry = None;
    let mut clear_window = false;
    if active.as_ref().is_some_and(|a| a.generation == generation) {
        let was_authorized = active.take().is_some_and(|a| a.authorized);
        let same_window = shared
            .pairing_windows
            .lock()
            .expect("lock poisoned")
            .contains_key(
                &remote
                    .public_key
                    .as_slice()
                    .try_into()
                    .expect("validated Hello key"),
            );
        shared.stop_mirror();
        shared.edit(|s| {
            s.status = "ready".into();
            for peer in &mut s.peers {
                if peer.id == remote.device_id {
                    peer.status = "disconnected".into();
                }
            }
            s.interrupt_transfers();
            if !was_authorized
                && same_window
                && s.pairing.is_none()
                && matches!(
                    result,
                    Err(session::Error::RateLimited(_) | session::Error::PairingExpired)
                )
            {
                let mut view =
                    pairing_view(&remote, "enter", [0; 16], Instant::now(), generation, None);
                view.window_id.clear();
                s.pairing = Some(view);
            }
            let pending = !was_authorized && same_window && s.pairing.is_some();
            if let Err(error) = &result {
                if pending {
                    let view = s.pairing.as_mut().expect("pending view");
                    match error {
                        session::Error::PairingRejected if view.deadline > Instant::now() => {
                            view.stage = "incorrect".into();
                            view.error = Some("incorrect".into());
                            view.error_revision = generation;
                            if view.mode == "enter" {
                                retry = Some(view.window_id.clone());
                            }
                        }
                        session::Error::RateLimited(ms) => {
                            view.stage = "limited".into();
                            view.error = Some("rate_limited".into());
                            view.retry_after_ms = *ms;
                            view.cooldown_deadline =
                                Some(Instant::now() + Duration::from_millis(u64::from(*ms)));
                        }
                        session::Error::PairingExpired | session::Error::PairingRejected => {
                            view.stage = "expired".into();
                            view.error = Some("expired".into());
                            view.code = None;
                            clear_window = true;
                        }
                        session::Error::Cancelled => {
                            s.pairing = None;
                            clear_window = true;
                        }
                        _ => {
                            view.stage = "disconnected".into();
                            view.error = Some("disconnected".into());
                            view.code = None;
                            clear_window = true;
                        }
                    }
                } else if !matches!(error, session::Error::Cancelled) {
                    s.set_error(error.to_string());
                }
            } else {
                s.pairing = None;
            }
        });
        if matches!(result, Err(session::Error::Cancelled)) {
            shared
                .pairing_windows
                .lock()
                .expect("lock poisoned")
                .clear_peer(&remote.device_id);
        } else if clear_window {
            shared
                .pairing_windows
                .lock()
                .expect("lock poisoned")
                .invalidate_peer(&remote.device_id);
        }
    }
    drop(active);
    if let Some(id) = retry {
        queue_pairing_retry(shared, generation, id);
    }
}

fn queue_pairing_retry(shared: Arc<Shared>, generation: u64, window_id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(520)).await;
        // A failed outbound connection still owns `dialing` until its caller
        // returns. Never overlap it, or retry after cancel / another owner.
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            {
                let active = shared.active.lock().expect("lock poisoned");
                if active.is_some()
                    || shared.generation.load(Ordering::Acquire) != generation.wrapping_add(1)
                {
                    return;
                }
                let windows = shared.pairing_windows.lock().expect("lock poisoned");
                let Some(window) = windows
                    .enter
                    .as_ref()
                    .filter(|w| hex_window(&w.id) == window_id && w.deadline > Instant::now())
                else {
                    return;
                };
                if !shared.dialing.load(Ordering::Acquire) {
                    // Cancel owns this same gate, including incrementing dial_cancel.
                    dial(
                        Arc::clone(&shared),
                        window.address,
                        Some(ymlink_core::identity::device_id(&window.peer_key)),
                        true,
                    );
                    return;
                }
                if Instant::now() >= until {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
}

async fn authorize_and_serve(
    shared: Arc<Shared>,
    generation: u64,
    mut session: SecureSession,
    remote: &Hello,
    endpoint: &str,
    initiator: bool,
    receiver: mpsc::Receiver<Message>,
    media_receiver: mpsc::Receiver<Message>,
    allow_pairing: bool,
    mut cancelled: watch::Receiver<bool>,
) -> session::Result<()> {
    let authorization = authenticate(
        &shared,
        generation,
        &mut session,
        remote,
        endpoint,
        initiator,
        allow_pairing,
    );
    let result = tokio::select! {
        biased;
        _ = cancelled.changed() => Err(session::Error::Cancelled),
        result = authorization => result,
    };
    if matches!(result, Err(session::Error::Cancelled)) {
        // The session is still exclusively owned here. Dropping the pending
        // reader first then closing after this terminal frame prevents a new
        // read on a partially consumed encrypted frame.
        let _ = tokio::time::timeout(
            Duration::from_millis(200),
            session.reject_pairing(AuthErrorCode::Cancelled, 0),
        )
        .await;
    }
    result?;
    {
        let mut trust = shared.trust.lock().await;
        let active = shared.active.lock().expect("lock poisoned");
        if !active
            .as_ref()
            .is_some_and(|a| a.generation == generation && !*a.cancel.borrow())
        {
            return Err(session::Error::Cancelled);
        }
        trust.check_endpoint(remote, &session.remote_key(), endpoint)?;
        trust.remember_endpoint(remote, &session.remote_key(), Some(endpoint.to_string()))?;
    }
    if !shared.publish(generation, true, |s| {
        publish_authenticated_peer(
            s,
            Peer {
                id: remote.device_id.clone(),
                address: endpoint.into(),
                name: remote.name.clone(),
                platform: remote.platform.clone(),
                status: "connected".into(),
                trusted: true,
            },
        );
        s.pairing = None;
        s.status = "connected".into();
        s.error = None;
    }) {
        return Err(session::Error::Cancelled);
    }
    tokio::select! {
        _ = cancelled.changed() => Ok(()),
        result = serve(shared, session, receiver, media_receiver, generation) => result,
    }
}

async fn authenticate(
    shared: &Arc<Shared>,
    generation: u64,
    session: &mut SecureSession,
    remote: &Hello,
    endpoint: &str,
    initiator: bool,
    allow_pairing: bool,
) -> session::Result<()> {
    let mut local_trust = shared.trust.lock().await.detached();
    let needs_pairing = session.needs_pairing(&local_trust).await?;
    if needs_pairing {
        if !allow_pairing {
            return Err(session::Error::PairingRequired);
        }
        let ip = endpoint
            .parse::<SocketAddr>()
            .map_err(|_| session::Error::Protocol("invalid peer endpoint"))?
            .ip();
        let display = !matches!(remote.platform.as_str(), "macos" | "windows") || !initiator;
        let peer_key = session.remote_key();
        if display {
            let resume = session.receive_pairing_request().await?;
            if let Err(ms) = shared.pairing_admission(ip) {
                session
                    .reject_pairing(AuthErrorCode::RateLimited, ms)
                    .await?;
                return Err(session::Error::RateLimited(ms));
            }
            let selected = shared.pairing_window(generation, |windows| {
                windows.display(peer_key, resume, Instant::now())
            })?;
            let window = match selected {
                Ok(window) => window,
                Err(code) => {
                    session.reject_pairing(code, 0).await?;
                    return Err(if code == AuthErrorCode::Expired {
                        session::Error::PairingExpired
                    } else {
                        session::Error::Busy
                    });
                }
            };
            session.offer_pairing(window.id, window.deadline).await?;
            if !shared.publish(generation, false, |s| {
                s.pairing = Some(pairing_view(
                    remote,
                    "display",
                    window.id,
                    window.deadline,
                    generation,
                    Some(window.code.to_string()),
                ));
            }) {
                return Err(session::Error::Cancelled);
            }
            let result = session
                .authorize(
                    &mut local_trust,
                    Pairing::Display {
                        code: window.code.to_string(),
                        expires_at: window.deadline,
                    },
                )
                .await;
            if matches!(result, Err(session::Error::PairingExpired)) {
                let _ = session.reject_pairing(AuthErrorCode::Expired, 0).await;
            }
            result?;
        } else {
            let address = endpoint
                .parse()
                .map_err(|_| session::Error::Protocol("invalid peer endpoint"))?;
            shared.pairing_window(generation, |windows| {
                windows.target = Some(EnterTarget { address, peer_key })
            })?;
            if let Err(ms) = shared.pairing_admission(ip) {
                session
                    .reject_pairing(AuthErrorCode::RateLimited, ms)
                    .await?;
                return Err(session::Error::RateLimited(ms));
            }
            let old = shared
                .pairing_windows
                .lock()
                .expect("lock poisoned")
                .enter
                .clone();
            if old.as_ref().is_some_and(|w| w.peer_key != peer_key) {
                return Err(session::Error::IdentityChanged);
            }
            if old.as_ref().is_some_and(|w| w.deadline <= Instant::now()) {
                return Err(session::Error::PairingExpired);
            }
            if let Some(old) = &old {
                session.restrict_pairing_deadline(old.deadline)?;
            }
            let ready = session.request_pairing(old.as_ref().map(|w| w.id)).await?;
            let mut deadline =
                Instant::now() + Duration::from_millis(u64::from(ready.remaining_ms));
            if let Some(old) = &old {
                deadline = deadline.min(old.deadline);
            }
            shared.pairing_window(generation, |windows| {
                windows.enter = Some(EnterWindow {
                    id: ready.window_id,
                    peer_key,
                    deadline,
                    address,
                })
            })?;
            let (send, receive) = oneshot::channel();
            if let Some(active) = shared
                .active
                .lock()
                .expect("lock poisoned")
                .as_mut()
                .filter(|a| a.generation == generation)
            {
                active.pair_input = Some(send);
            }
            if !shared.publish(generation, false, |s| {
                let prior = s
                    .pairing
                    .as_ref()
                    .filter(|p| p.window_id == hex_window(&ready.window_id));
                let prior_error = prior.and_then(|p| p.error.clone());
                let prior_revision = prior.map_or(0, |p| p.error_revision);
                let mut view =
                    pairing_view(remote, "enter", ready.window_id, deadline, generation, None);
                view.error = prior_error;
                view.error_revision = prior_revision;
                s.pairing = Some(view);
            }) {
                return Err(session::Error::Cancelled);
            }
            session
                .authorize_enter(&mut local_trust, async {
                    receive.await.map_err(|_| session::Error::Cancelled)
                })
                .await?;
        }
    } else {
        session
            .authorize(&mut local_trust, Pairing::ReconnectOnly)
            .await?;
    }
    Ok(())
}

fn hex_window(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

fn pairing_view(
    remote: &Hello,
    mode: &str,
    id: [u8; 16],
    deadline: Instant,
    attempt: u64,
    code: Option<String>,
) -> PairingView {
    PairingView {
        peer_name: remote.name.clone(),
        code,
        expires_in: deadline.saturating_duration_since(Instant::now()).as_secs(),
        mode: mode.into(),
        deadline,
        window_id: hex_window(&id),
        attempt,
        stage: "ready".into(),
        error: None,
        error_revision: 0,
        retry_after_ms: 0,
        cooldown_deadline: None,
    }
}

struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

const MAX_FILE_WORK: usize = 4;
const MAX_TRANSFER_IDS: usize = 1024;

enum WriterEvent {
    TransferAction {
        id: String,
        action: String,
        accepted: oneshot::Sender<bool>,
    },
    Finished(session::Result<()>),
}

enum FileTarget {
    Request(u64),
    Transfer(String),
}

type FileWork = tokio::task::JoinSet<session::Result<(FileTarget, FileResponse)>>;

async fn serve(
    shared: Arc<Shared>,
    session: SecureSession,
    mut outgoing: mpsc::Receiver<Message>,
    mut media: mpsc::Receiver<Message>,
    generation: u64,
) -> session::Result<()> {
    let (mut reader, mut writer) = session.into_split()?;
    let control = {
        let active = shared.active.lock().expect("lock poisoned");
        active
            .as_ref()
            .filter(|a| a.generation == generation && a.authorized && !*a.cancel.borrow())
            .map(|a| a.outbox.clone())
            .ok_or(session::Error::Protocol("connection no longer active"))?
    };
    let (incoming_tx, mut incoming) = mpsc::channel(32);
    let _reader = AbortTask(tokio::spawn(async move {
        loop {
            let result = reader.recv::<Message>().await;
            let stop = result.is_err();
            if incoming_tx.send(result).await.is_err() || stop {
                break;
            }
        }
    }));
    let (writer_events, mut events) = mpsc::channel(1);
    let _writer = AbortTask(tokio::spawn(async move {
        let mut ping = tokio::time::interval(Duration::from_secs(60));
        let result = async {
            loop {
                tokio::select! {
                    biased;
                    Some(message) = outgoing.recv() => {
                        if let Message::TransferAction { id, action, .. } = &message {
                            let (accepted, receive) = oneshot::channel();
                            let allowed = tokio::time::timeout(Duration::from_secs(5), async {
                                writer_events.send(WriterEvent::TransferAction { id: id.clone(), action: action.clone(), accepted }).await
                                    .map_err(|_| session::Error::Protocol("transfer controller closed"))?;
                                receive.await.map_err(|_| session::Error::Protocol("transfer controller closed"))
                            }).await.map_err(|_| session::Error::Protocol("transfer control timed out"))??;
                            if !allowed { continue; }
                        }
                        writer.send(&message).await?;
                    }
                    _ = ping.tick() => writer.send(&Message::Ping { v: VERSION }).await?,
                    Some(message) = media.recv() => writer.send(&message).await?,
                    else => return Ok(()),
                }
            }
        }.await;
        let _ = writer_events.send(WriterEvent::Finished(result)).await;
    }));
    let mut maintenance = tokio::time::interval(Duration::from_secs(60));
    let mut transfers: HashMap<String, TransferJob> = HashMap::new();
    let mut transfer_ids = HashSet::new();
    let mut files = FileWork::new();
    loop {
        for (id, job) in &mut transfers {
            send_transfer_chunk(&shared, &mut files, id, job);
        }
        tokio::select! {
            biased;
            event = events.recv() => {
                match event.ok_or(session::Error::Protocol("writer closed"))? {
                    WriterEvent::Finished(result) => return result,
                    WriterEvent::TransferAction { id, action, accepted } => {
                        if !transfers.contains_key(&id) {
                            shared.publish(generation, false, |s| s.set_error("transfer no longer active"));
                            let _ = accepted.send(false);
                            continue;
                        }
                        match transfer_action(&shared, &id, &action, Some(generation)) {
                            Ok(()) => {
                                if action == "cancel" { transfers.remove(&id); }
                                // The writer already owns this Action, so it writes it before
                                // any newly scheduled resumed chunk, without blocking this actor.
                                let _ = accepted.send(true);
                            }
                            Err(error) => { shared.publish(generation, false, |s| s.set_error(error.to_string())); let _ = accepted.send(false); }
                        }
                    }
                }
            }
            received = incoming.recv() => {
                let message = received.ok_or(session::Error::Protocol("connection closed"))??;
                match message {
                    Message::Ping{v:VERSION} => enqueue(&control,Message::Pong{v:VERSION})?,
                    Message::Pong{v:VERSION} => {},
                    Message::Bye{v:VERSION} => return Ok(()),
                    Message::MirrorQuery{v:VERSION,id}=>{
                        let result=remote_command(&shared,generation,Command::RefreshDisplays).await;
                        enqueue(&control,mirror_reply(&shared,id,result))?;
                    }
                    Message::MirrorControl{v:VERSION,id,command}=>{
                        let command=match command {MirrorAction::Pause=>Command::PauseMirror,MirrorAction::Resume=>Command::ResumeMirror,MirrorAction::Stop=>Command::StopMirror,MirrorAction::SelectDisplay{display_id}=>Command::SelectMirrorDisplay{display_id}};
                        let result=remote_command(&shared,generation,command).await;
                        enqueue(&control,mirror_reply(&shared,id,result))?;
                    }
                    Message::FileRequest{v:VERSION,id,request} => {
                        if files.len() >= MAX_FILE_WORK {
                            enqueue(&control,Message::FileResponse{v:VERSION,id,response:FileResponse::Error{code:ymlink_core::files::FileError::Unavailable}})?;
                        } else { file_request(&shared,&mut files,FileTarget::Request(id),request); }
                    }
                    Message::TransferOpen{v:VERSION,id,share_id,path,version,offset} => {
                        remember_transfer_id(&mut transfer_ids,&id)?;
                        if transfers.len() >= 4 {
                            return Err(session::Error::Protocol("invalid transfer request"));
                        }
                        let mut recorded = false;
                        if !shared.publish(generation, false, |state| {
                            state.transfers.retain(|t| t.id != id);
                            if state.transfers.len() >= 128 {
                                if let Some(index) = state.transfers.iter().position(|t| ["completed", "cancelled", "failed"].contains(&t.status.as_str())) {
                                    state.transfers.remove(index);
                                }
                            }
                            if state.transfers.len() < 128 {
                                state.transfers.push(Transfer{id:id.clone(),name:path.rsplit('/').next().unwrap_or("file").into(),transferred:offset,total:offset,status:"transferring".into()});
                                recorded = true;
                            }
                        }) { return Err(session::Error::Protocol("connection no longer active")); }
                        if !recorded {
                            enqueue(&control, Message::TransferError{v:VERSION,id,code:"too many unfinished transfers; cancel an interrupted transfer".into()})?;
                            continue;
                        }
                        transfers.insert(id,TransferJob{share_id,path,version,offset,awaiting:None,read_pending:false,last_activity:Instant::now()});
                    }
                    Message::TransferAck{v:VERSION,id,offset} => {
                        let Some(job) = transfers.get_mut(&id) else {
                            if transfer_ids.contains(&id){continue;}
                            return Err(session::Error::Protocol("unknown transfer acknowledgment"));
                        };
                        if job.awaiting != Some(offset) { return Err(session::Error::Protocol("invalid transfer acknowledgment")); }
                        job.offset = offset; job.awaiting = None;job.last_activity=Instant::now();
                        let mut complete = false;
                        if !shared.publish(generation, false, |s| { if let Some(t) = s.transfers.iter_mut().find(|t| t.id == id) { t.transferred = offset; complete = offset == t.total; if complete { t.status = "completed".into(); } } }) { return Err(session::Error::Protocol("connection no longer active")); }
                        if complete { transfers.remove(&id); }
                    }
                    Message::TransferAction{v:VERSION,id,action} => {
                        if !transfers.contains_key(&id) { return Err(session::Error::Protocol("unknown transfer")); }
                        transfer_action(&shared,&id,&action,Some(generation))?;
                        if action == "cancel" { transfers.remove(&id); }
                    }
                    _ => return Err(session::Error::Protocol("unsupported application frame")),
                }
            }
            result = files.join_next(), if !files.is_empty() => {
                let (target,response)=result.expect("join_next only fires while the JoinSet is non-empty").map_err(|_|session::Error::Protocol("file service failed"))??;
                match target {
                    FileTarget::Request(id)=>enqueue(&control,Message::FileResponse{v:VERSION,id,response})?,
                    FileTarget::Transfer(id)=>{
                        let Some(job)=transfers.get_mut(&id) else {continue;};
                        job.read_pending=false;
                        match response {
                            FileResponse::Read{offset,total,version,data,eof}=>{
                                if !shared.publish(generation,false,|s|{if let Some(t)=s.transfers.iter_mut().find(|t|t.id==id){t.total=total;}}) { return Err(session::Error::Protocol("connection no longer active")); }
                                if transfer_running(&shared,&id) {
                                    job.awaiting=Some(offset+data.len()as u64);
                                    enqueue(&control,Message::TransferChunk{v:VERSION,id,offset,total,version,data,eof})?;
                                }
                            }
                            response=>{
                                transfers.remove(&id);
                                if !shared.publish(generation,false,|s|{if let Some(t)=s.transfers.iter_mut().find(|t|t.id==id){t.status="failed".into();}}) { return Err(session::Error::Protocol("connection no longer active")); }
                                enqueue(&control,Message::TransferError{v:VERSION,id,code:format_file_error(response)})?;
                            }
                        }
                    }
                }
            }
            _ = maintenance.tick() => {
                let expired:Vec<_>=transfers.iter().filter(|(_,job)|job.last_activity.elapsed()>=Duration::from_secs(300)).map(|(id,_)|id.clone()).collect();
                for id in expired {
                    transfers.remove(&id);
                    if !shared.publish(generation,false,|s|{if let Some(t)=s.transfers.iter_mut().find(|t|t.id==id){t.status="failed".into();}}) { return Err(session::Error::Protocol("connection no longer active")); }
                    enqueue(&control,Message::TransferError{v:VERSION,id,code:"transfer timed out".into()})?;
                }
            }
        }
    }
}

fn enqueue(control: &mpsc::Sender<Message>, message: Message) -> session::Result<()> {
    control
        .try_send(message)
        .map_err(|_| session::Error::Protocol("connection control queue is full or closed"))
}

fn remember_transfer_id(ids: &mut HashSet<String>, id: &str) -> session::Result<()> {
    if !valid_transfer_id(id) {
        return Err(session::Error::Protocol("invalid transfer id"));
    }
    if ids.len() >= MAX_TRANSFER_IDS {
        return Err(session::Error::Protocol(
            "transfer id limit reached; reconnect",
        ));
    }
    if !ids.insert(id.into()) {
        return Err(session::Error::Protocol(
            "transfer id already used; reconnect",
        ));
    }
    Ok(())
}

struct TransferJob {
    share_id: String,
    path: String,
    version: String,
    offset: u64,
    awaiting: Option<u64>,
    read_pending: bool,
    last_activity: Instant,
}

fn transfer_running(shared: &Shared, id: &str) -> bool {
    shared
        .state
        .lock()
        .expect("lock poisoned")
        .transfers
        .iter()
        .any(|t| t.id == id && t.status == "transferring")
}

fn send_transfer_chunk(
    shared: &Arc<Shared>,
    files: &mut FileWork,
    id: &str,
    job: &mut TransferJob,
) {
    if files.len() >= MAX_FILE_WORK
        || job.read_pending
        || job.awaiting.is_some()
        || !transfer_running(shared, id)
    {
        return;
    }
    job.read_pending = true;
    file_request(
        shared,
        files,
        FileTarget::Transfer(id.into()),
        FileRequest::Read {
            share_id: job.share_id.clone(),
            path: job.path.clone(),
            version: job.version.clone(),
            offset: job.offset,
            length: MAX_READ_BYTES,
        },
    );
}

fn file_request(
    shared: &Arc<Shared>,
    files: &mut FileWork,
    target: FileTarget,
    request: FileRequest,
) {
    let shared = Arc::clone(shared);
    files.spawn(async move {
        let permit = Arc::clone(&shared.file_gate)
            .acquire_owned()
            .await
            .map_err(|_| session::Error::Protocol("file service closed"))?;
        tokio::task::spawn_blocking(move || {
            // Cancellation may drop the async waiter, but the physical IO keeps
            // its global slot until it actually finishes, including across reconnects.
            let _permit = permit;
            (
                target,
                shared
                    .catalog
                    .lock()
                    .expect("lock poisoned")
                    .handle(request),
            )
        })
        .await
        .map_err(|_| session::Error::Protocol("file service failed"))
    });
}

fn format_file_error(response: FileResponse) -> String {
    match response {
        FileResponse::Error { code } => code.to_string(),
        _ => "invalid file operation".into(),
    }
}
fn valid_transfer_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn transfer_action(
    shared: &Shared,
    id: &str,
    action: &str,
    expected: Option<u64>,
) -> session::Result<()> {
    let status = match action {
        "pause" => "paused",
        "resume" => "transferring",
        "cancel" => "cancelled",
        _ => return Err(session::Error::Protocol("invalid transfer action")),
    };
    let active = shared.active.lock().expect("lock poisoned");
    if expected.is_some_and(|generation| {
        !active
            .as_ref()
            .is_some_and(|a| a.generation == generation && a.authorized && !*a.cancel.borrow())
    }) {
        return Err(session::Error::Protocol("connection no longer active"));
    }
    let mut state = shared.state.lock().expect("lock poisoned");
    let transfer = state
        .transfers
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or(session::Error::Protocol("unknown transfer"))?;
    if expected.is_none() && transfer.status != "waiting" {
        return Err(session::Error::Protocol("transfer no longer waiting"));
    }
    if transfer.status == "completed" || transfer.status == "cancelled" {
        return Err(session::Error::Protocol("transfer already finished"));
    }
    transfer.status = status.into();
    Ok(())
}

async fn handle_command(
    shared: Arc<Shared>,
    discovery: &Discovery,
    command: Command,
    expected: Option<u64>,
) -> Result<(), String> {
    match command {
        Command::Rescan => {
            discovery.rescan().await.map_err(|e| e.to_string())?;
        }
        Command::Connect { address, peer_id } => {
            let active = shared.active.lock().expect("lock poisoned");
            if shared.dialing.load(Ordering::Acquire) {
                return Err("pairing busy; try again".into());
            }
            if active.is_some() {
                return Err("disconnect the current device first".into());
            }
            let peer = peer_id.and_then(|id| {
                shared
                    .state
                    .lock()
                    .expect("lock poisoned")
                    .peers
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
            });
            let address = address
                .or_else(|| peer.as_ref().map(|p| p.address.clone()))
                .ok_or("select a device or enter its address")?;
            let address: SocketAddr = address
                .parse()
                .map_err(|_| "enter an IPv4 address and port")?;
            if (!lan_address(address.ip())
                && (!shared.allow_loopback || !address.ip().is_loopback()))
                || address.port() == 0
            {
                return Err("only LAN device addresses are allowed".into());
            }
            let expected = peer.filter(|p| p.trusted).map(|p| p.id);
            if let Some(id) = &expected {
                shared
                    .disconnected
                    .lock()
                    .expect("lock poisoned")
                    .remove(id);
            }
            shared
                .pairing_windows
                .lock()
                .expect("lock poisoned")
                .clear();
            shared.edit(|s| {
                s.error = None;
                s.pairing = None;
            });
            dial(Arc::clone(&shared), address, expected, true);
            drop(active);
        }
        Command::Pair {
            code,
            window_id,
            attempt,
        } => {
            if code.len() != 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
                return Err("enter the four digit pairing code".into());
            }
            let mut active = shared.active.lock().expect("lock poisoned");
            let Some(current) = active
                .as_mut()
                .filter(|a| a.generation == attempt && !*a.cancel.borrow())
            else {
                return Ok(());
            };
            let mut state = shared.state.lock().expect("lock poisoned");
            let Some(view) = state
                .pairing
                .as_mut()
                .filter(|p| p.window_id == window_id && p.attempt == attempt && p.stage == "ready")
            else {
                return Ok(());
            };
            let Some(input) = current.pair_input.take() else {
                return Ok(());
            };
            if input.send(code).is_err() {
                return Ok(());
            }
            view.stage = "verifying".into();
            view.error = None;
        }
        Command::RetryPairing {} => {
            let active = shared.active.lock().expect("lock poisoned");
            if active.is_some() || shared.dialing.load(Ordering::Acquire) {
                return Err("pairing busy; try again".into());
            }
            let mut windows = shared.pairing_windows.lock().expect("lock poisoned");
            let target = windows
                .target
                .clone()
                .ok_or("no pairing input is pending")?;
            if shared
                .state
                .lock()
                .expect("lock poisoned")
                .pairing
                .as_ref()
                .and_then(|p| p.cooldown_deadline)
                .is_some_and(|d| d > Instant::now())
            {
                return Err("pairing rate limit; wait one minute".into());
            }
            if windows
                .enter
                .as_ref()
                .is_some_and(|w| w.deadline <= Instant::now())
            {
                windows.enter.take();
            }
            dial(
                Arc::clone(&shared),
                target.address,
                Some(ymlink_core::identity::device_id(&target.peer_key)),
                true,
            );
        }
        Command::CancelPairing => {
            let active = shared.active.lock().expect("lock poisoned");
            shared
                .dial_cancel
                .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
            if let Some(active) = active.as_ref() {
                let _ = active.cancel.send(true);
            }
            shared
                .pairing_windows
                .lock()
                .expect("lock poisoned")
                .clear();
            shared.edit(|s| s.pairing = None);
        }
        Command::Disconnect { peer_id } => {
            let active = shared.active.lock().expect("lock poisoned");
            let mut windows = shared.pairing_windows.lock().expect("lock poisoned");
            // Remember an explicit disconnect even before Noise has produced an
            // active identity, otherwise the reconnect tick dials it straight back.
            let pending_ids: Vec<String> = {
                let target = shared.dial_target.lock().expect("lock poisoned");
                target
                    .as_ref()
                    .map(|target| {
                        let state = shared.state.lock().expect("lock poisoned");
                        state
                            .peers
                            .iter()
                            .filter(|p| {
                                peer_id.as_ref().is_none_or(|id| p.id == *id)
                                    && target.expected.as_ref().map_or_else(
                                        || {
                                            p.address.parse::<SocketAddr>().ok()
                                                == Some(target.address)
                                        },
                                        |id| p.id == *id,
                                    )
                            })
                            .map(|p| p.id.clone())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            shared
                .disconnected
                .lock()
                .expect("lock poisoned")
                .extend(pending_ids);
            let pending_selected = peer_id
                .as_ref()
                .is_some_and(|id| shared.pending_dial_matches(id));
            let selected = peer_id.as_ref().is_none_or(|id| {
                active.as_ref().is_some_and(|a| a.peer_id == *id)
                    || windows
                        .target
                        .as_ref()
                        .is_some_and(|t| ymlink_core::identity::device_id(&t.peer_key) == *id)
            });
            if pending_selected && !selected {
                shared
                    .dial_cancel
                    .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
            }
            if selected {
                shared
                    .dial_cancel
                    .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
                if let Some(active) = active
                    .as_ref()
                    .filter(|a| peer_id.as_ref().is_none_or(|id| *id == a.peer_id))
                {
                    shared
                        .disconnected
                        .lock()
                        .expect("lock poisoned")
                        .insert(active.peer_id.clone());
                    let _ = active.cancel.send(true);
                }
                shared.edit(|s| s.pairing = None);
            }
            if let Some(id) = &peer_id {
                windows.clear_peer(id);
            } else {
                windows.clear();
            }
        }
        Command::Forget { peer_id } => {
            {
                let active = shared.active.lock().expect("lock poisoned");
                let mut windows = shared.pairing_windows.lock().expect("lock poisoned");
                if shared.pending_dial_matches(&peer_id) {
                    shared
                        .dial_cancel
                        .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
                }
                if windows
                    .target
                    .as_ref()
                    .is_some_and(|t| ymlink_core::identity::device_id(&t.peer_key) == peer_id)
                {
                    shared
                        .dial_cancel
                        .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
                    shared.edit(|s| s.pairing = None);
                }
                if let Some(active) = active.as_ref().filter(|a| a.peer_id == peer_id) {
                    let _ = active.cancel.send(true);
                    shared.edit(|s| s.pairing = None);
                }
                windows.clear_peer(&peer_id);
            }
            shared
                .trust
                .lock()
                .await
                .forget(&peer_id)
                .map_err(|e| e.to_string())?;
            shared
                .disconnected
                .lock()
                .expect("lock poisoned")
                .remove(&peer_id);
            shared.edit(|s| s.peers.retain(|p| p.id != peer_id));
        }
        command @ (Command::AddShare { .. } | Command::RemoveShare { .. }) => {
            tokio::task::spawn_blocking(move || {
                let _changes = shared.share_changes.lock().expect("lock poisoned");
                if let Err(error) = update_shares(&shared, command) {
                    shared.error(error);
                }
            });
        }
        Command::RefreshDisplays => {
            let displays = crate::capture::displays()?;
            shared.edit(|s| s.displays = displays);
        }
        command @ (Command::StartMirror { .. }
        | Command::StopMirror
        | Command::PauseMirror
        | Command::ResumeMirror
        | Command::SelectMirrorDisplay { .. }) => mirror_command(&shared, command, expected)?,
        Command::PauseTransfer { id } => {
            transfer_command(&shared, id, "pause").await?;
        }
        Command::ResumeTransfer { id } => {
            transfer_command(&shared, id, "resume").await?;
        }
        Command::CancelTransfer { id } => {
            transfer_command(&shared, id, "cancel").await?;
        }
    }
    Ok(())
}

fn mirror_command(
    shared: &Arc<Shared>,
    command: Command,
    expected: Option<u64>,
) -> Result<(), String> {
    let active = shared.active.lock().expect("lock poisoned");
    let current = active.as_ref().filter(|a| {
        a.authorized
            && !*a.cancel.borrow()
            && expected.is_none_or(|generation| generation == a.generation)
    });
    if matches!(command, Command::StopMirror) {
        if expected.is_some() && current.is_none() {
            return Err("connection no longer active".into());
        }
        shared.stop_mirror();
        return Ok(());
    }
    let current = current.ok_or("connect and pair a device before mirroring")?;
    let mut state = shared.state.lock().expect("lock poisoned");
    let selected = state.mirror.clone();
    if !matches!(command, Command::StartMirror { .. })
        && ["starting", "running", "paused"].contains(&selected.status.as_str())
    {
        let failure = shared
            .mirror_failure
            .lock()
            .expect("lock poisoned")
            .message();
        if let Some(error) = failure {
            shared.end_mirror_locked(&mut state, MirrorEnd::Stopped);
            state.mirror.status = "error".into();
            state.mirror.error = Some(error.clone());
            return Err(error);
        }
    }
    match command {
        Command::PauseMirror => {
            if selected.status == "paused" {
                return Ok(());
            }
            if !["starting", "running"].contains(&selected.status.as_str()) {
                return Err("mirror is not active".into());
            }
            shared.end_mirror_locked(&mut state, MirrorEnd::Paused);
        }
        Command::ResumeMirror => {
            if ["starting", "running"].contains(&selected.status.as_str()) {
                return Ok(());
            }
            if selected.status != "paused" {
                return Err("first mirror start requires local confirmation".into());
            }
            if expected.is_some() && !crate::capture::permission_granted() {
                return Err("Screen Recording permission is required before remote resume".into());
            }
            let display = selected
                .display_id
                .ok_or("selected display is unavailable")?;
            start_mirror(
                shared,
                current,
                &mut state,
                display,
                selected.audio,
                selected.quality,
                expected.is_none(),
                false,
            )?;
        }
        Command::SelectMirrorDisplay { display_id } => {
            if !["starting", "running", "paused"].contains(&selected.status.as_str()) {
                return Err("first mirror start requires local confirmation".into());
            }
            check_display(&state, display_id)?;
            if selected.display_id == Some(display_id) {
                return Ok(());
            }
            if selected.status == "paused" {
                state.mirror.display_id = Some(display_id);
            } else {
                if expected.is_some() && !crate::capture::permission_granted() {
                    return Err(
                        "Screen Recording permission is required before remote display selection"
                            .into(),
                    );
                }
                start_mirror(
                    shared,
                    current,
                    &mut state,
                    display_id,
                    selected.audio,
                    selected.quality,
                    expected.is_none(),
                    false,
                )?;
            }
        }
        Command::StartMirror {
            display_id,
            audio,
            quality,
        } => {
            if expected.is_some() {
                return Err("first mirror start requires local confirmation".into());
            }
            start_mirror(
                shared, current, &mut state, display_id, audio, quality, true, true,
            )?;
        }
        _ => return Err("invalid mirror command".into()),
    }
    Ok(())
}

fn check_display(state: &Snapshot, id: u32) -> Result<(), String> {
    if state.displays.iter().any(|display| display.id == id) {
        Ok(())
    } else {
        Err("selected display is unavailable".into())
    }
}

fn start_mirror(
    shared: &Arc<Shared>,
    current: &Active,
    state: &mut Snapshot,
    display_id: u32,
    audio: bool,
    quality: String,
    allow_permission_request: bool,
    fresh_start: bool,
) -> Result<(), String> {
    if !["smooth", "balanced", "quality"].contains(&quality.as_str()) {
        return Err("invalid mirror quality".into());
    }
    check_display(state, display_id)?;
    if fresh_start {
        *shared.mirror_failure.lock().expect("lock poisoned") =
            Arc::new(crate::capture::Failure::default());
    }
    let failure = shared.mirror_failure.lock().expect("lock poisoned").clone();
    shared.end_mirror_locked(state, MirrorEnd::Stopped);
    let generation = shared.mirror_generation.fetch_add(1, Ordering::AcqRel) + 1;
    let (cancel, receiver) = watch::channel(None);
    *shared.mirror_cancel.lock().expect("lock poisoned") = Some(cancel);
    state.mirror.status = "starting".into();
    state.mirror.error = None;
    state.mirror.display_id = Some(display_id);
    state.mirror.audio = audio;
    state.mirror.quality.clone_from(&quality);
    tokio::spawn(mirror(
        Arc::clone(shared),
        current.outbox.clone(),
        current.media.clone(),
        display_id,
        audio,
        quality,
        receiver,
        generation,
        allow_permission_request,
        failure,
    ));
    Ok(())
}

async fn remote_command(shared: &Shared, generation: u64, command: Command) -> Result<(), String> {
    let (reply, receive) = oneshot::channel();
    shared
        .commands
        .try_send(Request {
            command,
            reply: Some(reply),
            generation: Some(generation),
        })
        .map_err(|_| "backend busy or unavailable")?;
    tokio::time::timeout(Duration::from_secs(5), receive)
        .await
        .map_err(|_| "mirror command timed out")?
        .map_err(|_| "connection no longer active")?
}

fn mirror_reply(shared: &Shared, id: u64, result: Result<(), String>) -> Message {
    let snapshot = shared.state.lock().expect("lock poisoned");
    Message::MirrorReply {
        v: VERSION,
        id,
        ok: result.is_ok(),
        error: result.err(),
        state: MirrorInfo {
            status: snapshot.mirror.status.clone(),
            display_id: snapshot.mirror.display_id,
            audio: snapshot.mirror.audio,
            quality: snapshot.mirror.quality.clone(),
            error: snapshot.mirror.error.clone(),
            displays: snapshot.displays.clone(),
        },
    }
}

async fn transfer_command(shared: &Shared, id: String, action: &str) -> Result<(), String> {
    let waiting = shared
        .state
        .lock()
        .expect("lock poisoned")
        .transfers
        .iter()
        .any(|t| t.id == id && t.status == "waiting");
    if waiting {
        if action == "cancel" {
            transfer_action(shared, &id, action, None).map_err(|e| e.to_string())?;
            return Ok(());
        }
        return Err("the receiving device must resume this interrupted transfer".into());
    }
    let outgoing = shared
        .outgoing()
        .ok_or("reconnect the device to control this transfer")?;
    outgoing
        .try_send(Message::TransferAction {
            v: VERSION,
            id,
            action: action.into(),
        })
        .map_err(|_| "connection busy; try the transfer control again".into())
}

// Initial feedback parameters; real LAN / Quest runs must calibrate them.
const MIRROR_PRESSURE_WINDOW: Duration = Duration::from_secs(2);
const MIN_MIRROR_EDGE: u32 = 640;

fn mirror_congested(windows: &mut u8, attempted: u64, dropped: u64, queue_full: bool) -> bool {
    let congested = queue_full || (dropped >= 10 && dropped.saturating_mul(5) >= attempted);
    *windows = if congested {
        windows.saturating_add(1)
    } else {
        0
    };
    *windows >= 2
}

fn lower_mirror_edge(width: u32, height: u32) -> Option<u32> {
    let longest = width.max(height);
    (longest > MIN_MIRROR_EDGE)
        .then(|| (((u64::from(longest) * 3 / 4) as u32) & !1).max(MIN_MIRROR_EDGE))
}

fn record_capture_failure(failure: &crate::capture::Failure, message: String) -> String {
    failure
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_or_insert(message)
        .clone()
}

async fn mirror(
    shared: Arc<Shared>,
    control: mpsc::Sender<Message>,
    outbox: mpsc::Sender<Message>,
    display: u32,
    audio: bool,
    quality: String,
    mut cancel: watch::Receiver<Option<MirrorEnd>>,
    generation: u64,
    mut allow_permission_request: bool,
    failure: Arc<crate::capture::Failure>,
) {
    let mut maximum_edge = None;
    loop {
        if cancel.borrow().is_some() {
            return;
        }
        let stream_id = rand::random::<u64>();
        let permit = tokio::select! {
            biased;
            _ = cancel.changed() => return,
            permit = Arc::clone(&shared.capture_gate).acquire_owned() => match permit { Ok(permit) => permit, Err(_) => return },
        };
        if cancel.borrow().is_some() {
            return;
        }
        shared.mirror_edit(generation, |s| {
            s.mirror.status = "starting".into();
            s.mirror.fps = None;
            s.mirror.bitrate_mbps = None;
        });
        let (packets_tx, mut packets) = mpsc::channel(2);
        let capture_quality = quality.clone();
        let capture_failure = Arc::clone(&failure);
        let started = tokio::task::spawn_blocking(move || {
            crate::capture::Capture::start(
                display,
                audio,
                &capture_quality,
                packets_tx,
                allow_permission_request,
                maximum_edge,
                capture_failure,
            )
        })
        .await;
        // Automatic restarts never ask the OS for recording access.
        allow_permission_request = false;
        let capture = match started {
            Ok(Ok(capture)) => capture,
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    _ => "screen capture worker failed".into(),
                };
                let error = record_capture_failure(&failure, error);
                shared.mirror_edit(generation, |s| {
                    s.mirror.status = "error".into();
                    s.mirror.permission = if crate::capture::permission_granted() {
                        "granted"
                    } else {
                        "denied"
                    }
                    .into();
                    s.mirror.error = Some(error);
                    let _ = control.try_send(Message::MirrorState {
                        v: VERSION,
                        status: "error".into(),
                        stream_id,
                    });
                });
                return;
            }
        };
        if cancel.borrow().is_some() {
            finish_capture(capture, permit).await;
            return;
        }
        if !announce_mirror(
            &shared.state,
            &shared.mirror_generation,
            generation,
            &control,
            stream_id,
        ) {
            finish_capture(capture, permit).await;
            return;
        }
        let dimensions = capture.dimensions();
        let mut pressure = tokio::time::interval_at(
            tokio::time::Instant::now() + MIRROR_PRESSURE_WINDOW,
            MIRROR_PRESSURE_WINDOW,
        );
        pressure.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut pressure_windows = 0;
        let mut packet_id = 0u64;
        let mut frames = 0u64;
        let mut bytes = 0u64;
        let mut window = Instant::now();
        let mut failed = false;
        let mut reduce = false;
        'stream: loop {
            tokio::select! {
                biased;
                _ = cancel.changed() => break,
                _ = pressure.tick() => {
                    if failure.message().is_some() { failed = true; break; }
                    let (attempted, dropped) = capture.take_video_pressure();
                    if mirror_congested(&mut pressure_windows, attempted, dropped, outbox.capacity() == 0) { reduce = true; break; }
                }
                packet = packets.recv() => {
                    let Some(packet) = packet else { break };
                    if packet.kind == "error" || packet.data.len() > 16 * 1024 * 1024 {
                        failed = true;
                        let error = if packet.kind == "error" { String::from_utf8_lossy(&packet.data).into_owned() } else { "encoded screen packet exceeds limit".into() };
                        shared.mirror_edit(generation, |s| { s.mirror.status = "error".into(); s.mirror.error = Some(error); });
                        break;
                    }
                    let parts = packet.data.len().max(1).div_ceil(8192) as u32;
                    for part in 0..parts {
                        // Reserve before moving the fragment: timer ticks cannot lose it.
                        let slot = loop {
                            tokio::select! {
                                biased;
                                _ = cancel.changed() => break 'stream,
                                _ = pressure.tick() => {
                                    if failure.message().is_some() { failed = true; break 'stream; }
                                    let (attempted, dropped) = capture.take_video_pressure();
                                    if mirror_congested(&mut pressure_windows, attempted, dropped, outbox.capacity() == 0) { reduce = true; break 'stream; }
                                }
                                slot = outbox.reserve() => if let Ok(slot) = slot { break slot } else { failed = true; break 'stream; },
                            }
                        };
                        let start = part as usize * 8192;
                        let end = (start + 8192).min(packet.data.len());
                        slot.send(Message::Media { v: VERSION, stream_id, packet_id, part, parts, media_kind: packet.kind.clone(), codec: packet.codec.clone(), pts_us: packet.pts_us, keyframe: packet.keyframe, width: packet.width, height: packet.height, data: packet.data[start..end].to_vec() });
                    }
                    if packet.kind == "video" { frames += 1; }
                    bytes += packet.data.len() as u64;
                    packet_id = packet_id.wrapping_add(1);
                    let elapsed = window.elapsed().as_secs_f64();
                    if elapsed >= 1.0 {
                        shared.mirror_edit(generation, |s| { s.mirror.fps = Some(frames as f64 / elapsed); s.mirror.bitrate_mbps = Some(bytes as f64 * 8.0 / elapsed / 1_000_000.0); });
                        frames = 0; bytes = 0; window = Instant::now();
                    }
                }
            }
        }
        finish_capture(capture, permit).await;
        if let Some(error) = failure.message() {
            failed = true;
            reduce = false;
            shared.mirror_edit(generation, |s| {
                s.mirror.status = "error".into();
                s.mirror.error = Some(error);
            });
        }
        if reduce && cancel.borrow().is_none() {
            if let Some(next) = lower_mirror_edge(dimensions.0, dimensions.1) {
                // A new stream discards old partial packets and decoder dependencies.
                if control
                    .try_send(Message::MirrorState {
                        v: VERSION,
                        status: "stopped".into(),
                        stream_id,
                    })
                    .is_err()
                {
                    shared.mirror_edit(generation, |s| {
                        s.mirror.status = "error".into();
                        s.mirror.error = Some("connection busy; try starting mirror again".into());
                    });
                    return;
                }
                maximum_edge = Some(next);
                continue;
            }
            failed = true;
            shared.mirror_edit(generation, |s| {
                s.mirror.status = "error".into();
                s.mirror.error = Some(
                    "connection too slow for screen mirroring; turn off system audio or reconnect"
                        .into(),
                );
            });
        }
        let status = if failed {
            "error"
        } else {
            cancel.borrow().map_or("stopped", MirrorEnd::status)
        };
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            control.send(Message::MirrorState {
                v: VERSION,
                status: status.into(),
                stream_id,
            }),
        )
        .await;
        shared.mirror_edit(generation, |s| {
            if ["running", "starting"].contains(&s.mirror.status.as_str()) {
                s.mirror.status = status.into();
            }
            s.mirror.fps = None;
            s.mirror.bitrate_mbps = None;
        });
        return;
    }
}

async fn finish_capture(
    mut capture: crate::capture::Capture,
    permit: tokio::sync::OwnedSemaphorePermit,
) {
    let _ = tokio::task::spawn_blocking(move || {
        capture.stop();
        drop(capture);
        drop(permit);
    })
    .await;
}

fn announce_mirror(
    state: &Mutex<Snapshot>,
    current: &AtomicU64,
    generation: u64,
    control: &mpsc::Sender<Message>,
    stream_id: u64,
) -> bool {
    // Hold the same state gate used when a new generation starts. Enqueue without
    // awaiting so an old task cannot publish running after the replacement stream.
    let mut state = state.lock().expect("lock poisoned");
    if current.load(Ordering::Acquire) != generation {
        return false;
    }
    if control
        .try_send(Message::MirrorState {
            v: VERSION,
            status: "running".into(),
            stream_id,
        })
        .is_err()
    {
        state.mirror.status = "error".into();
        state.mirror.error = Some("connection busy; try starting mirror again".into());
        return false;
    }
    state.mirror.status = "running".into();
    state.mirror.permission = "granted".into();
    true
}

fn private_directory(path: &Path) -> Result<(), String> {
    ymlink_core::storage::private_directory(path)
        .map_err(|_| "cannot secure application data directory".into())
}

fn save_shares(path: &Path, shares: &[Share]) -> Result<(), String> {
    use std::io::Write;
    let temporary = path.with_file_name(format!(".shares-{:016x}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| "cannot save shared folders")?;
        serde_json::to_writer(&mut file, shares).map_err(|_| "cannot encode shared folders")?;
        file.write_all(b"\n")
            .and_then(|()| file.sync_all())
            .map_err(|_| "cannot sync shared folders")?;
        std::fs::rename(&temporary, path)
            .map_err(|_| "cannot replace shared folder configuration")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

fn update_shares(shared: &Shared, command: Command) -> Result<(), String> {
    match command {
        Command::AddShare { path } => {
            if path.len() > 4096 || !Path::new(&path).is_absolute() {
                return Err("choose a local folder".into());
            }
            let canonical =
                std::fs::canonicalize(&path).map_err(|_| "the selected folder is unavailable")?;
            let path = canonical
                .to_str()
                .ok_or("folder path must be valid UTF-8")?
                .to_string();
            if shared
                .state
                .lock()
                .expect("lock poisoned")
                .shares
                .iter()
                .any(|s| s.path == path)
            {
                return Err("folder is already shared".into());
            }
            let name = canonical
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Shared folder")
                .to_string();
            let id = format!("{:016x}", rand::random::<u64>());
            shared
                .catalog
                .lock()
                .expect("lock poisoned")
                .add(id.clone(), name.clone(), &canonical)
                .map_err(|e| e.to_string())?;
            let mut shares = shared.state.lock().expect("lock poisoned").shares.clone();
            shares.push(Share {
                id: id.clone(),
                name,
                path,
            });
            if let Err(error) = save_shares(&shared.config_path, &shares) {
                shared.catalog.lock().expect("lock poisoned").remove(&id);
                return Err(error);
            }
            shared.edit(|s| s.shares = shares);
        }
        Command::RemoveShare { id } => {
            let mut shares = shared.state.lock().expect("lock poisoned").shares.clone();
            if !shares.iter().any(|s| s.id == id) {
                return Err("shared folder was not found".into());
            }
            shares.retain(|s| s.id != id);
            save_shares(&shared.config_path, &shares)?;
            shared.catalog.lock().expect("lock poisoned").remove(&id);
            shared.edit(|s| s.shares = shares);
        }
        _ => unreachable!("only shared-folder commands are dispatched here"),
    }
    Ok(())
}
