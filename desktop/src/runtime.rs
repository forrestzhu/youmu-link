use crate::model::{Command, PairingView, Peer, Share, Snapshot, Transfer};
use crate::wire::{Message, MirrorAction, MirrorInfo, VERSION};
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
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
use ymlink_core::session::{self, Hello, Pairing, SecureSession};

pub struct Backend {
    runtime: Option<tokio::runtime::Runtime>,
    commands: mpsc::Sender<Request>,
    state: Arc<Mutex<Snapshot>>,
}

impl Backend {
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
        let state_task = state.clone();
        let worker_commands = commands.clone();
        runtime.spawn(async move {
            if let Err(error) = run(
                path,
                state_task.clone(),
                receiver,
                identity,
                allow_loopback,
                worker_commands,
            )
            .await
            {
                let mut state = state_task.lock().unwrap_or_else(|e| e.into_inner());
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
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pairing) = &mut state.pairing {
            pairing.expires_in = pairing
                .deadline
                .saturating_duration_since(Instant::now())
                .as_secs();
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

struct Shared {
    state: Arc<Mutex<Snapshot>>,
    identity: Identity,
    trust: AsyncMutex<TrustStore>,
    catalog: Mutex<FileCatalog>,
    active: Mutex<Option<Active>>,
    generation: AtomicU64,
    dialing: AtomicBool,
    pairing_attempts: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
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
    fn edit(&self, edit: impl FnOnce(&mut Snapshot)) {
        edit(&mut self.state.lock().unwrap_or_else(|e| e.into_inner()));
    }
    fn error(&self, error: impl Into<String>) {
        self.edit(|s| s.set_error(error));
    }
    fn stop_mirror(&self) {
        self.end_mirror(MirrorEnd::Stopped);
    }
    fn end_mirror(&self, reason: MirrorEnd) {
        let mut state = self.state.lock().unwrap();
        self.end_mirror_locked(&mut state, reason);
    }
    fn end_mirror_locked(&self, state: &mut Snapshot, reason: MirrorEnd) {
        self.mirror_generation.fetch_add(1, Ordering::AcqRel);
        if let Some(cancel) = self.mirror_cancel.lock().unwrap().take() {
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
            .unwrap()
            .as_ref()
            .filter(|a| a.authorized && !*a.cancel.borrow())
            .map(|a| a.outbox.clone())
    }
    fn mirror_edit(&self, generation: u64, edit: impl FnOnce(&mut Snapshot)) {
        let mut state = self.state.lock().unwrap();
        if self.mirror_generation.load(Ordering::Acquire) == generation {
            edit(&mut state);
        }
    }
    fn publish(&self, generation: u64, authorized: bool, edit: impl FnOnce(&mut Snapshot)) -> bool {
        let mut active = self.active.lock().unwrap();
        let Some(active) = active
            .as_mut()
            .filter(|a| a.generation == generation && !*a.cancel.borrow())
        else {
            return false;
        };
        if authorized {
            active.authorized = true;
        }
        self.edit(edit);
        true
    }
    fn pairing_allowed(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut attempts = self.pairing_attempts.lock().unwrap();
        attempts.retain(|_, entries| {
            entries
                .back()
                .is_some_and(|t| now.duration_since(*t) < Duration::from_secs(60))
        });
        if attempts.len() >= 128 && !attempts.contains_key(&ip) {
            return false;
        }
        let entries = attempts.entry(ip).or_default();
        while entries
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
        {
            entries.pop_front();
        }
        if entries.len() >= 5 {
            return false;
        }
        entries.push_back(now);
        true
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
        let mut state = state.lock().unwrap();
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
            caps: vec!["control".into(), "files".into(), "mirror".into()],
            allow_loopback,
            ..Default::default()
        },
        events_tx,
    )
    .await
    .map_err(|e| e.to_string())?;
    {
        let mut state = state.lock().unwrap();
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
        pairing_attempts: Mutex::new(HashMap::new()),
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
                if !lan_address(address.ip()) && !(shared.allow_loopback && address.ip().is_loopback()) { continue; }
                let Ok(permit) = slots.clone().try_acquire_owned() else { continue };
                let shared = shared.clone();
                tokio::spawn(async move { let _permit = permit; connection(shared,stream,address,false,None,false).await; });
            }
            event = events.recv() => {
                if let Some(discovery::Event::PeerDiscovered(peer)) = event {
                    let address = SocketAddr::new(peer.addr, peer.listen_port);
                    let trusted = shared.trust.lock().await.peers().into_iter().find(|p| p.endpoint.as_ref().and_then(|e| e.parse::<SocketAddr>().ok()).is_some_and(|e| e.ip() == peer.addr));
                    shared.edit(|s| {
                        if let Some(known) = &trusted {
                            if let Some(item) = s.peers.iter_mut().find(|p| p.id == known.device_id) { item.address = address.to_string(); if item.status != "connected" { item.status = "discovered".into(); } }
                        } else if let Some(item) = s.peers.iter_mut().find(|p| p.address == address.to_string()) {
                            if item.status != "connected" { item.status = "discovered".into(); }
                        } else if s.peers.len() < 128 {
                            s.peers.push(Peer { id: address.to_string(), address: address.to_string(), name: String::new(), platform: String::new(), status: "discovered".into(), trusted: false });
                        }
                        if s.status == "searching" { s.status = "ready".into(); }
                    });
                } else if let Some(discovery::Event::PeerExpired(ip)) = event {
                    shared.edit(|s| { s.peers.retain(|p| p.trusted || p.address.parse::<SocketAddr>().map_or(true, |a| a.ip() != ip)); });
                }
            }
            Some(request) = commands.recv() => {
                let expired=request.reply.as_ref().is_some_and(|reply|reply.is_closed());
                let authorized=request.generation.map_or(true,|generation|shared.active.lock().unwrap().as_ref().is_some_and(|a|a.generation==generation && a.authorized && !*a.cancel.borrow()));
                let result=if expired{Err("request expired".into())}else if !authorized{Err("connection no longer active".into())}else{handle_command(shared.clone(), &discovery, request.command,request.generation).await};
                if let Some(reply)=request.reply {let _=reply.send(result);}else if let Err(error)=result{shared.error(error);}
            }
            _ = reconnect.tick() => {
                if shared.active.lock().unwrap().is_none() && !shared.dialing.load(Ordering::Acquire) {
                    let state=shared.state.lock().unwrap();
                    let has_trust=state.peers.iter().any(|p|p.trusted);
                    let candidates:Vec<_>=state.peers.iter().filter(|p| !p.address.is_empty() && !shared.disconnected.lock().unwrap().contains(&p.id) && (p.trusted || (has_trust && p.status=="discovered"))).cloned().collect();
                    drop(state);
                    if !candidates.is_empty() {
                        let peer = &candidates[reconnect_cursor % candidates.len()];
                        reconnect_cursor = reconnect_cursor.wrapping_add(1);
                        if let Ok(address) = peer.address.parse() { dial(shared.clone(), address,peer.trusted.then(||peer.id.clone()),false); }
                    }
                }
            }
        }
    }
}

fn lan_address(ip: IpAddr) -> bool {
    discovery::is_lan_source(ip, false)
}

fn dial(shared: Arc<Shared>, address: SocketAddr, expected: Option<String>, manual: bool) {
    if shared.dialing.swap(true, Ordering::AcqRel) {
        return;
    }
    tokio::spawn(async move {
        let connected =
            tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address)).await;
        match connected {
            Ok(Ok(stream)) => {
                connection(
                    shared.clone(),
                    stream,
                    address,
                    true,
                    expected.clone(),
                    manual,
                )
                .await
            }
            _ if manual => {
                shared.error("cannot connect; confirm the other device is on the same Wi-Fi")
            }
            _ => {}
        }
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
    let (name, port) = {
        let s = shared.state.lock().unwrap();
        (s.device_name.clone(), s.listen_port)
    };
    let local = Hello::new(&shared.identity, name, std::env::consts::OS, port);
    let session = match SecureSession::handshake(stream, initiator, &shared.identity, local).await {
        Ok(session) => session,
        Err(error) => {
            if initiator && manual {
                shared.error(error.to_string());
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
    let (cancel, mut cancelled) = watch::channel(false);
    let generation = shared.generation.fetch_add(1, Ordering::Relaxed);
    let preferred = initiator == (shared.identity.device_id() < remote.device_id);
    {
        let mut active = shared.active.lock().unwrap();
        if manual {
            shared
                .disconnected
                .lock()
                .unwrap()
                .remove(&remote.device_id);
        } else if shared
            .disconnected
            .lock()
            .unwrap()
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
    let result = tokio::select! {
        _ = cancelled.changed() => Ok(()),
        result = authorize_and_serve(shared.clone(),generation,session,&remote,&endpoint,initiator,receiver,media_receiver,!initiator||manual) => result,
    };
    let mut active = shared.active.lock().unwrap();
    if active.as_ref().is_some_and(|a| a.generation == generation) {
        active.take();
        shared.stop_mirror();
        shared.edit(|s| {
            s.pairing = None;
            s.status = "ready".into();
            for peer in &mut s.peers {
                if peer.id == remote.device_id {
                    peer.status = "disconnected".into();
                }
            }
            s.interrupt_transfers();
            if let Err(error) = result {
                s.set_error(error.to_string());
            }
        });
    }
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
) -> session::Result<()> {
    let mut local_trust = shared.trust.lock().await.detached();
    let needs_pairing = session.needs_pairing(&local_trust).await?;
    let pairing = if needs_pairing {
        if !allow_pairing {
            return Err(session::Error::PairingRequired);
        }
        let ip = endpoint
            .parse::<SocketAddr>()
            .map_err(|_| session::Error::Protocol("invalid peer endpoint"))?
            .ip();
        if !shared.pairing_allowed(ip) {
            return Err(session::Error::Protocol(
                "pairing rate limit; wait one minute",
            ));
        }
        let deadline = Instant::now() + session::PAIRING_TTL;
        let display = !matches!(remote.platform.as_str(), "macos" | "windows") || !initiator;
        if display {
            let code = session::new_pairing_code();
            if !shared.publish(generation, false, |s| {
                s.pairing = Some(PairingView {
                    peer_name: remote.name.clone(),
                    code: Some(code.clone()),
                    expires_in: 300,
                    mode: "display".into(),
                    deadline,
                });
            }) {
                return Err(session::Error::PairingRejected);
            }
            Pairing::Display {
                code,
                expires_at: deadline,
            }
        } else {
            let (send, receive) = oneshot::channel();
            if let Some(active) = shared
                .active
                .lock()
                .unwrap()
                .as_mut()
                .filter(|a| a.generation == generation)
            {
                active.pair_input = Some(send);
            }
            if !shared.publish(generation, false, |s| {
                s.pairing = Some(PairingView {
                    peer_name: remote.name.clone(),
                    code: None,
                    expires_in: 300,
                    mode: "enter".into(),
                    deadline,
                });
            }) {
                return Err(session::Error::PairingRejected);
            }
            let code = tokio::time::timeout(session::PAIRING_TTL, receive)
                .await
                .map_err(|_| session::Error::PairingExpired)?
                .map_err(|_| session::Error::PairingRejected)?;
            Pairing::Enter { code }
        }
    } else {
        Pairing::ReconnectOnly
    };
    session.authorize(&mut local_trust, pairing).await?;
    {
        let mut trust = shared.trust.lock().await;
        let active = shared.active.lock().unwrap();
        if !active
            .as_ref()
            .is_some_and(|a| a.generation == generation && !*a.cancel.borrow())
        {
            return Err(session::Error::PairingRejected);
        }
        trust.check_endpoint(remote, &session.remote_key(), endpoint)?;
        trust.remember_endpoint(remote, &session.remote_key(), Some(endpoint.to_string()))?;
    }
    if !shared.publish(generation, true, |s| {
        s.peers
            .retain(|p| p.id != remote.device_id && p.address != endpoint);
        s.peers.push(Peer {
            id: remote.device_id.clone(),
            address: endpoint.into(),
            name: remote.name.clone(),
            platform: remote.platform.clone(),
            status: "connected".into(),
            trusted: true,
        });
        s.pairing = None;
        s.status = "connected".into();
        s.error = None;
    }) {
        return Err(session::Error::PairingRejected);
    }
    serve(shared, session, receiver, media_receiver, generation).await
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
        let active = shared.active.lock().unwrap();
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
                let (target,response)=result.unwrap().map_err(|_|session::Error::Protocol("file service failed"))??;
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
        .unwrap()
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
    let shared = shared.clone();
    files.spawn(async move {
        let permit = shared
            .file_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| session::Error::Protocol("file service closed"))?;
        tokio::task::spawn_blocking(move || {
            // Cancellation may drop the async waiter, but the physical IO keeps
            // its global slot until it actually finishes, including across reconnects.
            let _permit = permit;
            (target, shared.catalog.lock().unwrap().handle(request))
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
    let active = shared.active.lock().unwrap();
    if expected.is_some_and(|generation| {
        !active
            .as_ref()
            .is_some_and(|a| a.generation == generation && a.authorized && !*a.cancel.borrow())
    }) {
        return Err(session::Error::Protocol("connection no longer active"));
    }
    let mut state = shared.state.lock().unwrap();
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
            if shared.active.lock().unwrap().is_some() {
                return Err("disconnect the current device first".into());
            }
            let peer = peer_id.and_then(|id| {
                shared
                    .state
                    .lock()
                    .unwrap()
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
                && !(shared.allow_loopback && address.ip().is_loopback()))
                || address.port() == 0
            {
                return Err("only LAN device addresses are allowed".into());
            }
            let expected = peer.filter(|p| p.trusted).map(|p| p.id);
            if let Some(id) = &expected {
                shared.disconnected.lock().unwrap().remove(id);
            }
            shared.edit(|s| s.error = None);
            dial(shared.clone(), address, expected, true);
        }
        Command::Pair { code } => {
            if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
                return Err("enter the six digit pairing code".into());
            }
            let input = shared
                .active
                .lock()
                .unwrap()
                .as_mut()
                .and_then(|a| a.pair_input.take())
                .ok_or("no pairing input is pending")?;
            input.send(code).map_err(|_| "pairing expired")?;
        }
        Command::CancelPairing => {
            if let Some(active) = shared.active.lock().unwrap().as_ref() {
                let _ = active.cancel.send(true);
            }
        }
        Command::Disconnect { peer_id } => {
            if let Some(active) = shared.active.lock().unwrap().as_ref() {
                if peer_id.as_ref().map_or(true, |id| *id == active.peer_id) {
                    shared
                        .disconnected
                        .lock()
                        .unwrap()
                        .insert(active.peer_id.clone());
                    let _ = active.cancel.send(true);
                }
            }
        }
        Command::Forget { peer_id } => {
            // Cancel before taking the trust lock: authorization can be waiting for UI.
            if let Some(active) = shared
                .active
                .lock()
                .unwrap()
                .as_ref()
                .filter(|a| a.peer_id == peer_id)
            {
                let _ = active.cancel.send(true);
            }
            shared
                .trust
                .lock()
                .await
                .forget(&peer_id)
                .map_err(|e| e.to_string())?;
            shared.disconnected.lock().unwrap().remove(&peer_id);
            shared.edit(|s| {
                s.peers.retain(|p| p.id != peer_id);
            });
        }
        command @ (Command::AddShare { .. } | Command::RemoveShare { .. }) => {
            tokio::task::spawn_blocking(move || {
                let _changes = shared.share_changes.lock().unwrap();
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
    let active = shared.active.lock().unwrap();
    let current = active.as_ref().filter(|a| {
        a.authorized
            && !*a.cancel.borrow()
            && expected.map_or(true, |generation| generation == a.generation)
    });
    if matches!(command, Command::StopMirror) {
        if expected.is_some() && current.is_none() {
            return Err("connection no longer active".into());
        }
        shared.stop_mirror();
        return Ok(());
    }
    let current = current.ok_or("connect and pair a device before mirroring")?;
    let mut state = shared.state.lock().unwrap();
    let selected = state.mirror.clone();
    if !matches!(command, Command::StartMirror { .. })
        && ["starting", "running", "paused"].contains(&selected.status.as_str())
    {
        let failure = shared.mirror_failure.lock().unwrap().message();
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
        *shared.mirror_failure.lock().unwrap() = Arc::new(crate::capture::Failure::default());
    }
    let failure = shared.mirror_failure.lock().unwrap().clone();
    shared.end_mirror_locked(state, MirrorEnd::Stopped);
    let generation = shared.mirror_generation.fetch_add(1, Ordering::AcqRel) + 1;
    let (cancel, receiver) = watch::channel(None);
    *shared.mirror_cancel.lock().unwrap() = Some(cancel);
    state.mirror.status = "starting".into();
    state.mirror.error = None;
    state.mirror.display_id = Some(display_id);
    state.mirror.audio = audio;
    state.mirror.quality = quality.clone();
    tokio::spawn(mirror(
        shared.clone(),
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
    let snapshot = shared.state.lock().unwrap();
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
        .unwrap()
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
        .unwrap_or_else(|error| error.into_inner())
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
            permit = shared.capture_gate.clone().acquire_owned() => match permit { Ok(permit) => permit, Err(_) => return },
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
        let capture_failure = failure.clone();
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
                                slot = outbox.reserve() => match slot {
                                    Ok(slot) => break slot,
                                    Err(_) => { failed = true; break 'stream; }
                                },
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
    let mut state = state.lock().unwrap();
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
            .and_then(|_| file.sync_all())
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
                .unwrap()
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
                .unwrap()
                .add(id.clone(), name.clone(), &canonical)
                .map_err(|e| e.to_string())?;
            let mut shares = shared.state.lock().unwrap().shares.clone();
            shares.push(Share {
                id: id.clone(),
                name,
                path,
            });
            if let Err(error) = save_shares(&shared.config_path, &shares) {
                shared.catalog.lock().unwrap().remove(&id);
                return Err(error);
            }
            shared.edit(|s| s.shares = shares);
        }
        Command::RemoveShare { id } => {
            let mut shares = shared.state.lock().unwrap().shares.clone();
            if !shares.iter().any(|s| s.id == id) {
                return Err("shared folder was not found".into());
            }
            shares.retain(|s| s.id != id);
            save_shares(&shared.config_path, &shares)?;
            shared.catalog.lock().unwrap().remove(&id);
            shared.edit(|s| s.shares = shares);
        }
        _ => unreachable!("only shared-folder commands are dispatched here"),
    }
    Ok(())
}
