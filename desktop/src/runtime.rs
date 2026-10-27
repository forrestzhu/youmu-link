use crate::model::{Command, PairingView, Peer, Share, Snapshot, Transfer};
use crate::wire::{Message, VERSION};
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
    commands: mpsc::Sender<Command>,
    pub state: Arc<Mutex<Snapshot>>,
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
        runtime.spawn(async move {
            if let Err(error) =
                run(path, state_task.clone(), receiver, identity, allow_loopback).await
            {
                let mut state = state_task.lock().unwrap_or_else(|e| e.into_inner());
                state.status = "error".into();
                state.error = Some(error);
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
            .try_send(command)
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
    mirror_cancel: Mutex<Option<watch::Sender<bool>>>,
    disconnected: Mutex<HashSet<String>>,
    mirror_generation: AtomicU64,
    allow_loopback: bool,
    share_changes: Mutex<()>,
}

impl Shared {
    fn edit(&self, edit: impl FnOnce(&mut Snapshot)) {
        edit(&mut self.state.lock().unwrap_or_else(|e| e.into_inner()));
    }
    fn error(&self, error: impl Into<String>) {
        self.edit(|s| s.error = Some(error.into()));
    }
    fn stop_mirror(&self) {
        self.mirror_generation.fetch_add(1, Ordering::AcqRel);
        if let Some(cancel) = self.mirror_cancel.lock().unwrap().take() {
            let _ = cancel.send(true);
        }
        self.edit(|s| {
            s.mirror.status = "stopped".into();
            s.mirror.fps = None;
            s.mirror.bitrate_mbps = None;
        });
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
    mut commands: mpsc::Receiver<Command>,
    identity: Option<Identity>,
    allow_loopback: bool,
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
                state.error = Some(
                    "a shared folder is unavailable; add it again after reconnecting the disk"
                        .into(),
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
            Some(command) = commands.recv() => {
                if let Err(error) = handle_command(shared.clone(), &discovery, command).await { shared.error(error); }
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
    let local = Hello::new(&shared.identity, name, "macos", port);
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
            for transfer in &mut s.transfers {
                if ["transferring", "paused"].contains(&transfer.status.as_str()) {
                    transfer.status = "waiting".into();
                }
            }
            if let Err(error) = result {
                s.error = Some(error.to_string());
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
        let display = remote.platform != "macos" || !initiator;
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
    serve(shared, session, receiver, media_receiver).await
}

struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn serve(
    shared: Arc<Shared>,
    session: SecureSession,
    mut outgoing: mpsc::Receiver<Message>,
    mut media: mpsc::Receiver<Message>,
) -> session::Result<()> {
    let (mut reader, mut writer) = session.into_split()?;
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
    let mut ping = tokio::time::interval(Duration::from_secs(60));
    let mut transfers: HashMap<String, TransferJob> = HashMap::new();
    loop {
        tokio::select! {
            biased;
            received = incoming.recv() => {
                let message = received.ok_or(session::Error::Protocol("connection closed"))??;
                match message {
                    Message::Ping{v:VERSION} => writer.send(&Message::Pong{v:VERSION}).await?,
                    Message::Pong{v:VERSION} => {},
                    Message::Bye{v:VERSION} => return Ok(()),
                    Message::FileRequest{v:VERSION,id,request} => {
                        let response = file_request(&shared,request).await?;
                        writer.send(&Message::FileResponse{v:VERSION,id,response}).await?;
                    }
                    Message::TransferOpen{v:VERSION,id,share_id,path,version,offset} => {
                        if !valid_transfer_id(&id) || transfers.len() >= 4 { return Err(session::Error::Protocol("invalid transfer request")); }
                        let response = file_request(&shared,FileRequest::Read{share_id:share_id.clone(),path:path.clone(),version:version.clone(),offset,length:MAX_READ_BYTES}).await?;
                        if let FileResponse::Read{total,..} = response {
                            shared.edit(|s| {
                                s.transfers.retain(|t| t.id != id);
                                if s.transfers.len() >= 128 { s.transfers.remove(0); }
                                s.transfers.push(Transfer{id:id.clone(),name:path.rsplit('/').next().unwrap_or("file").into(),transferred:offset,total,status:"transferring".into()});
                            });
                            let job = TransferJob {share_id,path,version,offset,awaiting:None,last_activity:Instant::now()};
                            transfers.insert(id.clone(),job);
                            send_transfer_chunk(&shared,&mut writer,&id,transfers.get_mut(&id).unwrap()).await?;
                        } else { writer.send(&Message::TransferError{v:VERSION,id,code:format_file_error(response)}).await?; }
                    }
                    Message::TransferAck{v:VERSION,id,offset} => {
                        let Some(job) = transfers.get_mut(&id) else {
                            if shared.state.lock().unwrap().transfers.iter().any(|t|t.id==id && ["cancelled","failed","completed"].contains(&t.status.as_str())){continue;}
                            return Err(session::Error::Protocol("unknown transfer acknowledgment"));
                        };
                        if job.awaiting != Some(offset) { return Err(session::Error::Protocol("invalid transfer acknowledgment")); }
                        job.offset = offset; job.awaiting = None;job.last_activity=Instant::now();
                        let mut complete = false;
                        shared.edit(|s| { if let Some(t) = s.transfers.iter_mut().find(|t| t.id == id) { t.transferred = offset; complete = offset == t.total; if complete { t.status = "completed".into(); } } });
                        if complete { transfers.remove(&id); } else { send_transfer_chunk(&shared,&mut writer,&id,job).await?; }
                    }
                    Message::TransferAction{v:VERSION,id,action} => { transfer_action(&shared,&id,&action)?; if action == "cancel" { transfers.remove(&id); } else if action == "resume" { if let Some(job)=transfers.get_mut(&id) { send_transfer_chunk(&shared,&mut writer,&id,job).await?; } } }
                    _ => return Err(session::Error::Protocol("unsupported application frame")),
                }
            }
            Some(message) = outgoing.recv() => {
                if let Message::TransferAction{id,action,..}=&message {
                    if let Err(error)=transfer_action(&shared,id,action){shared.error(error.to_string());continue;}
                }
                writer.send(&message).await?;
                match &message {
                    Message::TransferAction{id,action,..} if action == "resume" => {
                        if let Some(job)=transfers.get_mut(id) { send_transfer_chunk(&shared,&mut writer,id,job).await?; }
                    }
                    Message::TransferAction{id,action,..} if action == "cancel" => { transfers.remove(id); }
                    _ => {},
                }
            }
            _ = ping.tick() => {
                writer.send(&Message::Ping{v:VERSION}).await?;
                let expired:Vec<_>=transfers.iter().filter(|(_,job)|job.last_activity.elapsed()>=Duration::from_secs(300)).map(|(id,_)|id.clone()).collect();
                for id in expired {
                    transfers.remove(&id);
                    shared.edit(|s|{if let Some(t)=s.transfers.iter_mut().find(|t|t.id==id){t.status="failed".into();}});
                    writer.send(&Message::TransferError{v:VERSION,id,code:"transfer timed out".into()}).await?;
                }
            }
            Some(message)=media.recv()=>{writer.send(&message).await?;}
        }
    }
}

struct TransferJob {
    share_id: String,
    path: String,
    version: String,
    offset: u64,
    awaiting: Option<u64>,
    last_activity: Instant,
}

async fn send_transfer_chunk(
    shared: &Arc<Shared>,
    writer: &mut session::SecureWriter,
    id: &str,
    job: &mut TransferJob,
) -> session::Result<()> {
    let running = shared
        .state
        .lock()
        .unwrap()
        .transfers
        .iter()
        .any(|t| t.id == id && t.status == "transferring");
    if !running || job.awaiting.is_some() {
        return Ok(());
    }
    let response = file_request(
        shared,
        FileRequest::Read {
            share_id: job.share_id.clone(),
            path: job.path.clone(),
            version: job.version.clone(),
            offset: job.offset,
            length: MAX_READ_BYTES,
        },
    )
    .await?;
    match response {
        FileResponse::Read {
            offset,
            total,
            version,
            data,
            eof,
        } => {
            job.awaiting = Some(offset + data.len() as u64);
            writer
                .send(&Message::TransferChunk {
                    v: VERSION,
                    id: id.into(),
                    offset,
                    total,
                    version,
                    data,
                    eof,
                })
                .await
        }
        response => {
            shared.edit(|s| {
                if let Some(t) = s.transfers.iter_mut().find(|t| t.id == id) {
                    t.status = "failed".into();
                }
            });
            writer
                .send(&Message::TransferError {
                    v: VERSION,
                    id: id.into(),
                    code: format_file_error(response),
                })
                .await
        }
    }
}

async fn file_request(shared: &Arc<Shared>, request: FileRequest) -> session::Result<FileResponse> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || shared.catalog.lock().unwrap().handle(request))
        .await
        .map_err(|_| session::Error::Protocol("file service failed"))
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

fn transfer_action(shared: &Shared, id: &str, action: &str) -> session::Result<()> {
    let status = match action {
        "pause" => "paused",
        "resume" => "transferring",
        "cancel" => "cancelled",
        _ => return Err(session::Error::Protocol("invalid transfer action")),
    };
    let mut state = shared.state.lock().unwrap();
    let transfer = state
        .transfers
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or(session::Error::Protocol("unknown transfer"))?;
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
        Command::StartMirror {
            display_id,
            audio,
            quality,
        } => {
            if !["smooth", "balanced", "quality"].contains(&quality.as_str()) {
                return Err("invalid mirror quality".into());
            }
            if !shared
                .state
                .lock()
                .unwrap()
                .displays
                .iter()
                .any(|d| d.id == display_id)
            {
                return Err("selected display is unavailable".into());
            }
            let active = shared.active.lock().unwrap();
            let current = active
                .as_ref()
                .filter(|a| a.authorized && !*a.cancel.borrow())
                .ok_or("connect and pair a device before mirroring")?;
            let outbox = current.media.clone();
            let control = current.outbox.clone();
            shared.stop_mirror();
            let generation = shared.mirror_generation.fetch_add(1, Ordering::AcqRel) + 1;
            let (cancel, receiver) = watch::channel(false);
            *shared.mirror_cancel.lock().unwrap() = Some(cancel);
            shared.edit(|s| {
                s.mirror.status = "starting".into();
                s.mirror.error = None;
                s.mirror.display_id = Some(display_id);
                s.mirror.audio = audio;
                s.mirror.quality = quality.clone();
            });
            drop(active);
            tokio::spawn(mirror(
                shared.clone(),
                control,
                outbox,
                display_id,
                audio,
                quality,
                receiver,
                generation,
            ));
        }
        Command::StopMirror => shared.stop_mirror(),
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
            transfer_action(shared, &id, action).map_err(|e| e.to_string())?;
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

async fn mirror(
    shared: Arc<Shared>,
    control: mpsc::Sender<Message>,
    outbox: mpsc::Sender<Message>,
    display: u32,
    audio: bool,
    quality: String,
    mut cancel: watch::Receiver<bool>,
    generation: u64,
) {
    let stream_id = rand::random::<u64>();
    if *cancel.borrow() {
        return;
    }
    let (packets_tx, mut packets) = mpsc::channel(2);
    let started = tokio::task::spawn_blocking(move || {
        crate::capture::Capture::start(display, audio, &quality, packets_tx)
    })
    .await;
    let mut capture = match started {
        Ok(Ok(capture)) => capture,
        Ok(Err(error)) => {
            shared.mirror_edit(generation, |s| {
                s.mirror.status = "error".into();
                s.mirror.permission = if crate::capture::permission_granted() {
                    "granted"
                } else {
                    "denied"
                }
                .into();
                s.mirror.error = Some(error);
            });
            return;
        }
        Err(_) => {
            shared.mirror_edit(generation, |s| {
                s.mirror.status = "error".into();
                s.mirror.error = Some("screen capture worker failed".into());
            });
            return;
        }
    };
    if *cancel.borrow() {
        let _ = tokio::task::spawn_blocking(move || capture.stop()).await;
        return;
    }
    shared.mirror_edit(generation, |s| {
        s.mirror.status = "running".into();
        s.mirror.permission = "granted".into();
    });
    let announced = tokio::select! {
        _=cancel.changed()=>false,
        result=control.send(Message::MirrorState {
            v: VERSION,
            status: "running".into(),
            stream_id,
        })=>result.is_ok(),
    };
    if !announced {
        let _ = tokio::task::spawn_blocking(move || capture.stop()).await;
        return;
    }
    let mut packet_id = 0u64;
    let mut frames = 0u64;
    let mut bytes = 0u64;
    let mut window = Instant::now();
    loop {
        tokio::select! {
            _=cancel.changed()=>break,
            packet=packets.recv()=>{
                let Some(packet)=packet else{break};
                if packet.kind=="error" {
                    shared.mirror_edit(generation,|s|{s.mirror.status="error".into();s.mirror.error=Some(String::from_utf8_lossy(&packet.data).into_owned());});break;
                }
                if packet.data.len()>16*1024*1024 { shared.error("encoded screen packet exceeds limit");break; }
                if packet.kind=="video" {frames+=1;}
                bytes+=packet.data.len() as u64;
                let parts=packet.data.len().max(1).div_ceil(8192) as u32;
                let mut failed=false;
                for part in 0..parts {
                    let start=part as usize*8192;
                    let end=(start+8192).min(packet.data.len());
                    let frame=Message::Media{v:VERSION,stream_id,packet_id,part,parts,media_kind:packet.kind.clone(),codec:packet.codec.clone(),pts_us:packet.pts_us,keyframe:packet.keyframe,width:packet.width,height:packet.height,data:packet.data[start..end].to_vec()};
                    tokio::select! {
                        _=cancel.changed()=>{failed=true;break;},
                        result=outbox.send(frame)=>{if result.is_err(){failed=true;break;}},
                    }
                }
                if failed{break;}
                packet_id=packet_id.wrapping_add(1);
                let elapsed=window.elapsed().as_secs_f64();
                if elapsed>=1.0 {
                    shared.mirror_edit(generation,|s|{s.mirror.fps=Some(frames as f64/elapsed);s.mirror.bitrate_mbps=Some(bytes as f64*8.0/elapsed/1_000_000.0);});
                    frames=0;bytes=0;window=Instant::now();
                }
            }
        }
    }
    let _ = tokio::task::spawn_blocking(move || capture.stop()).await;
    let _ = control.try_send(Message::MirrorState {
        v: VERSION,
        status: "stopped".into(),
        stream_id,
    });
    shared.mirror_edit(generation, |s| {
        if s.mirror.status == "running" {
            s.mirror.status = "stopped".into();
        }
        s.mirror.fps = None;
        s.mirror.bitrate_mbps = None;
    });
}

fn private_directory(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|_| "cannot create application data directory")?;
    if std::fs::symlink_metadata(path)
        .map_err(|_| "cannot inspect application data directory")?
        .file_type()
        .is_symlink()
    {
        return Err("application data directory must not be a symlink".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "cannot secure application data directory")?;
    }
    Ok(())
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
