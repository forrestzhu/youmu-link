//! A real encrypted test peer. This is not a product headset implementation.
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use ymlink_core::files::{FileEntry, FileRequest, FileResponse, MAX_READ_BYTES};
use ymlink_core::identity::{Identity, TrustStore};
use ymlink_core::session::{Hello, Pairing, SecureReader, SecureSession, SecureWriter};
use ymlink_desktop::wire::{Message, MirrorAction, VERSION};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
// Local checkpoint schema is independent of the application wire version.
const DOWNLOAD_STATE_VERSION: u8 = 3;

fn failure(message: &'static str) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message).into()
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("headset-test: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err(failure("usage: headset_stub ADDRESS shares | list SHARE [PATH] | stat SHARE PATH | range SHARE PATH OFFSET LENGTH | download SHARE PATH DEST | mirror SECONDS [OUTPUT_DIR] | mirror-info | mirror-control pause|resume|stop|display ID"));
    }
    let address: SocketAddr = args[0]
        .parse()
        .map_err(|_| failure("invalid socket address"))?;
    if address.port() == 0 || !ymlink_core::discovery::is_lan_source(address.ip(), true) {
        return Err(failure(
            "test peer requires a local IPv4 or loopback endpoint",
        ));
    }
    let local = TcpListener::bind(("0.0.0.0", 0)).await?;
    let identity = Identity::generate()?;
    let hello = Hello::new(
        &identity,
        "YM_Link headset test",
        "headset-test",
        local.local_addr()?.port(),
    );
    let socket =
        tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address)).await??;
    let mut session = SecureSession::handshake(socket, true, &identity, hello).await?;
    let mut trust = TrustStore::memory();
    if session.needs_pairing(&trust).await? {
        eprintln!("Enter the four-digit code displayed by YM_Link, then press Return:");
        let code = tokio::task::spawn_blocking(|| -> std::io::Result<String> {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            Ok(line.trim().to_owned())
        })
        .await??;
        session
            .authorize(&mut trust, Pairing::Enter { code })
            .await?;
    } else {
        session
            .authorize(&mut trust, Pairing::ReconnectOnly)
            .await?;
    }
    let source_device_id = session.remote().device_id.clone();
    let (mut reader, mut writer) = session.into_split()?;
    eprintln!("Authenticated encrypted test connection established.");
    let command = args[1].as_str();
    let mut business_ok = true;
    match command {
        "shares" if args.len() == 2 => {
            let response = request(&mut reader, &mut writer, FileRequest::Shares).await?;
            println!("{}", serde_json::to_string(&response)?);
        }
        "list" if (3..=4).contains(&args.len()) => {
            let mut cursor = None;
            loop {
                let response = request(
                    &mut reader,
                    &mut writer,
                    FileRequest::List {
                        share_id: args[2].clone(),
                        path: args.get(3).cloned().unwrap_or_default(),
                        cursor,
                        limit: 256,
                    },
                )
                .await?;
                if let FileResponse::List {
                    entries,
                    next_cursor,
                } = response
                {
                    for entry in entries {
                        println!("{}", serde_json::to_string(&entry)?);
                    }
                    match next_cursor {
                        Some(next) => cursor = Some(next),
                        None => break,
                    }
                } else {
                    return Err(failure("directory listing rejected"));
                }
            }
        }
        "stat" if args.len() == 4 => {
            println!(
                "{}",
                serde_json::to_string(&stat(&mut reader, &mut writer, &args[2], &args[3]).await?)?
            );
        }
        "range" if args.len() == 6 => {
            let entry = stat(&mut reader, &mut writer, &args[2], &args[3]).await?;
            let response = request(
                &mut reader,
                &mut writer,
                FileRequest::Read {
                    share_id: args[2].clone(),
                    path: args[3].clone(),
                    version: entry
                        .version
                        .ok_or_else(|| failure("selected entry is not a file"))?,
                    offset: args[4].parse().map_err(|_| failure("invalid offset"))?,
                    length: args[5].parse().map_err(|_| failure("invalid length"))?,
                },
            )
            .await?;
            println!("{}", serde_json::to_string(&response)?);
        }
        "download" if args.len() == 5 => {
            download(
                &mut reader,
                &mut writer,
                &source_device_id,
                &args[2],
                &args[3],
                Path::new(&args[4]),
            )
            .await?
        }
        "mirror-info" if args.len() == 2 => {
            business_ok = mirror_request(&mut reader, &mut writer, None).await?;
        }
        "mirror-control" => {
            let action = mirror_action(&args[2..])?;
            business_ok = mirror_request(&mut reader, &mut writer, Some(action)).await?;
        }
        "mirror" if (3..=4).contains(&args.len()) => {
            let seconds: u64 = args[2].parse().map_err(|_| failure("invalid duration"))?;
            if !(1..=300).contains(&seconds) {
                return Err(failure("mirror duration must be 1..300 seconds"));
            }
            mirror(
                &mut reader,
                &mut writer,
                seconds,
                args.get(3).map(Path::new),
            )
            .await?;
            // The duration cancels the current read, so the connection must now close.
            return Ok(());
        }
        _ => return Err(failure("invalid test command or arguments")),
    }
    writer.send(&Message::Bye { v: VERSION }).await?;
    if business_ok {
        Ok(())
    } else {
        Err(failure("mirror command rejected"))
    }
}

fn mirror_action(arguments: &[String]) -> Result<MirrorAction> {
    match arguments {
        [action] if action == "pause" => Ok(MirrorAction::Pause),
        [action] if action == "resume" => Ok(MirrorAction::Resume),
        [action] if action == "stop" => Ok(MirrorAction::Stop),
        [action, display] if action == "display" => Ok(MirrorAction::SelectDisplay {
            display_id: display.parse().map_err(|_| failure("invalid display id"))?,
        }),
        _ => Err(failure("mirror-control requires pause, resume, stop or display ID; remote start is not allowed")),
    }
}

async fn receive(reader: &mut SecureReader, writer: &mut SecureWriter) -> Result<Message> {
    loop {
        let message: Message = reader.recv().await?;
        let version = match &message {
            Message::Ping { v }
            | Message::Pong { v }
            | Message::Bye { v }
            | Message::FileRequest { v, .. }
            | Message::FileResponse { v, .. }
            | Message::TransferOpen { v, .. }
            | Message::TransferChunk { v, .. }
            | Message::TransferAck { v, .. }
            | Message::TransferAction { v, .. }
            | Message::TransferError { v, .. }
            | Message::MirrorQuery { v, .. }
            | Message::MirrorControl { v, .. }
            | Message::MirrorReply { v, .. }
            | Message::MirrorState { v, .. }
            | Message::Media { v, .. } => *v,
        };
        if version != VERSION {
            return Err(failure("incompatible application version"));
        }
        match message {
            Message::Ping { .. } => writer.send(&Message::Pong { v: VERSION }).await?,
            Message::Pong { .. } => {}
            Message::Bye { .. } => return Err(failure("desktop closed the test connection")),
            message => return Ok(message),
        }
    }
}

async fn mirror_request(
    reader: &mut SecureReader,
    writer: &mut SecureWriter,
    command: Option<MirrorAction>,
) -> Result<bool> {
    let id = rand::random::<u64>();
    let message = match command {
        Some(command) => Message::MirrorControl {
            v: VERSION,
            id,
            command,
        },
        None => Message::MirrorQuery { v: VERSION, id },
    };
    writer.send(&message).await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match receive(reader, writer).await? {
                reply @ Message::MirrorReply { id: reply_id, .. } if reply_id == id => {
                    let Message::MirrorReply { ok, .. } = &reply else {
                        unreachable!()
                    };
                    println!("{}", serde_json::to_string(&reply)?);
                    return Ok(*ok);
                }
                Message::MirrorState {
                    status, stream_id, ..
                } => {
                    println!(
                        "{}",
                        serde_json::json!({"mirror_state":status,"stream_id":stream_id})
                    );
                }
                Message::Media { .. } | Message::MirrorReply { .. } => {}
                _ => return Err(failure("unexpected mirror control response")),
            }
        }
    })
    .await
    .map_err(|_| failure("mirror command reply timed out"))?
}

async fn request(
    reader: &mut SecureReader,
    writer: &mut SecureWriter,
    request: FileRequest,
) -> Result<FileResponse> {
    let id = rand::random::<u64>();
    writer
        .send(&Message::FileRequest {
            v: VERSION,
            id,
            request,
        })
        .await?;
    loop {
        match receive(reader, writer).await? {
            Message::FileResponse {
                id: response_id,
                response,
                ..
            } if response_id == id => return Ok(response),
            Message::MirrorState { .. } | Message::Media { .. } => {}
            _ => return Err(failure("unexpected file response")),
        }
    }
}

async fn stat(
    reader: &mut SecureReader,
    writer: &mut SecureWriter,
    share: &str,
    path: &str,
) -> Result<FileEntry> {
    match request(
        reader,
        writer,
        FileRequest::Stat {
            share_id: share.into(),
            path: path.into(),
        },
    )
    .await?
    {
        FileResponse::Stat { entry } => Ok(entry),
        _ => Err(failure("file metadata request rejected")),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    v: u8,
    source_device_id: String,
    share_id: String,
    path: String,
    version: String,
    total: u64,
    offset: u64,
}

struct Download {
    state: Resume,
    file: File,
    part: PathBuf,
    metadata: PathBuf,
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

impl Download {
    fn open(
        destination: &Path,
        source_device_id: &str,
        share: &str,
        path: &str,
        version: &str,
        total: u64,
    ) -> Result<Self> {
        if fs::symlink_metadata(destination).is_ok() {
            return Err(failure("destination already exists"));
        }
        let part = sidecar(destination, ".part");
        let metadata = sidecar(destination, ".part.json");
        let has_part = fs::symlink_metadata(&part).is_ok();
        let has_metadata = fs::symlink_metadata(&metadata).is_ok();
        if has_part != has_metadata {
            return Err(failure(
                "incomplete download sidecars; inspect them before restarting",
            ));
        }
        let (state, file) = if has_part {
            let original = fs::symlink_metadata(&part)?;
            if !original.is_file() || !fs::symlink_metadata(&metadata)?.is_file() {
                return Err(failure("download sidecars must be regular files"));
            }
            let mut bytes = Vec::new();
            File::open(&metadata)?
                .take(16_385)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 16_384 {
                return Err(failure("download metadata exceeds limit"));
            }
            let state: Resume = serde_json::from_slice(&bytes)
                .map_err(|_| failure("invalid download metadata; old partial download retained"))?;
            if state.v != DOWNLOAD_STATE_VERSION {
                return Err(failure(
                    "unsupported download checkpoint version; old partial download retained",
                ));
            }
            if state.source_device_id != source_device_id
                || state.share_id != share
                || state.path != path
                || state.version != version
                || state.total != total
                || state.offset > total
            {
                return Err(failure(
                    "download source changed; old partial download retained",
                ));
            }
            let file = OpenOptions::new().read(true).write(true).open(&part)?;
            let opened = file.metadata()?;
            if !opened.is_file() || !same_file(&original, &opened) {
                return Err(failure("partial file was replaced while opening it"));
            }
            if file.metadata()?.len() < state.offset {
                return Err(failure("partial file is shorter than its saved offset"));
            }
            file.set_len(state.offset)?;
            (state, file)
        } else {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&part)?;
            (
                Resume {
                    v: DOWNLOAD_STATE_VERSION,
                    source_device_id: source_device_id.into(),
                    share_id: share.into(),
                    path: path.into(),
                    version: version.into(),
                    total,
                    offset: 0,
                },
                file,
            )
        };
        let mut download = Self {
            state,
            file,
            part,
            metadata,
        };
        download.file.seek(SeekFrom::Start(download.state.offset))?;
        download.checkpoint()?;
        Ok(download)
    }

    fn checkpoint(&self) -> Result<()> {
        self.file.sync_data()?;
        let temporary = sidecar(
            &self.metadata,
            &format!(".{:016x}.tmp", rand::random::<u64>()),
        );
        let result = (|| -> Result<()> {
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            serde_json::to_writer(&mut output, &self.state)?;
            output.sync_all()?;
            fs::rename(&temporary, &self.metadata)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }

    fn append(
        &mut self,
        offset: u64,
        total: u64,
        version: &str,
        data: &[u8],
        eof: bool,
    ) -> Result<()> {
        let end = offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| failure("invalid transfer offset"))?;
        if offset != self.state.offset
            || total != self.state.total
            || version != self.state.version
            || data.len() > MAX_READ_BYTES as usize
            || end > total
            || eof != (end == total)
            || (data.is_empty() && !eof)
        {
            return Err(failure("inconsistent transfer chunk"));
        }
        self.file.write_all(data)?;
        self.state.offset = end;
        self.checkpoint()
    }

    fn finish(self, destination: &Path) -> Result<()> {
        if self.state.offset != self.state.total {
            return Err(failure("download is incomplete"));
        }
        self.file.sync_all()?;
        let original = fs::symlink_metadata(&self.part)?;
        if !original.is_file() || !same_file(&original, &self.file.metadata()?) {
            return Err(failure("partial file was replaced before completion"));
        }
        // Same-directory hard linking creates the destination atomically without replacing an existing file.
        fs::hard_link(&self.part, destination)?;
        fs::remove_file(&self.part)?;
        fs::remove_file(&self.metadata)?;
        Ok(())
    }
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len()
            && left.created().ok() == right.created().ok()
            && left.modified().ok() == right.modified().ok()
    }
}

async fn download(
    reader: &mut SecureReader,
    writer: &mut SecureWriter,
    source_device_id: &str,
    share: &str,
    path: &str,
    destination: &Path,
) -> Result<()> {
    let entry = stat(reader, writer, share, path).await?;
    let version = entry
        .version
        .ok_or_else(|| failure("selected entry is not a file"))?;
    let mut download = Download::open(
        destination,
        source_device_id,
        share,
        path,
        &version,
        entry.size,
    )?;
    let resumed_from = download.state.offset;
    let id = format!("test-{:016x}", rand::random::<u64>());
    writer
        .send(&Message::TransferOpen {
            v: VERSION,
            id: id.clone(),
            share_id: share.into(),
            path: path.into(),
            version,
            offset: resumed_from,
        })
        .await?;
    loop {
        match receive(reader, writer).await? {
            Message::TransferChunk {
                id: response_id,
                offset,
                total,
                version,
                data,
                eof,
                ..
            } if response_id == id => {
                download.append(offset, total, &version, &data, eof)?;
                writer
                    .send(&Message::TransferAck {
                        v: VERSION,
                        id: id.clone(),
                        offset: download.state.offset,
                    })
                    .await?;
                if eof {
                    break;
                }
            }
            Message::TransferError {
                id: response_id, ..
            } if response_id == id => {
                return Err(failure(
                    "desktop rejected the transfer; partial download retained",
                ))
            }
            Message::TransferAction {
                id: response_id,
                action,
                ..
            } if response_id == id => {
                if action == "cancel" {
                    return Err(failure(
                        "desktop cancelled the transfer; partial download retained",
                    ));
                }
            }
            Message::MirrorState { .. } | Message::Media { .. } => {}
            _ => return Err(failure("unexpected transfer response")),
        }
    }
    let total = download.state.total;
    download.finish(destination)?;
    println!(
        "{}",
        serde_json::json!({"download":"completed","bytes":total,"resumed_from":resumed_from})
    );
    Ok(())
}

#[derive(PartialEq)]
struct MediaHeader {
    stream: u64,
    packet: u64,
    parts: u32,
    kind: String,
    codec: String,
    pts: u64,
    keyframe: bool,
    width: u32,
    height: u32,
}
struct PendingMedia {
    header: MediaHeader,
    next: u32,
    data: Vec<u8>,
}

#[derive(Default)]
struct MediaAssembly {
    active: Option<u64>,
    pending: Option<PendingMedia>,
}

impl MediaAssembly {
    fn state(&mut self, status: &str, stream: u64) {
        match status {
            "running" if self.active != Some(stream) => {
                self.active = Some(stream);
                self.pending = None;
            }
            "stopped" | "paused" | "error" if self.active == Some(stream) => {
                self.active = None;
                self.pending = None;
            }
            _ => {}
        }
    }

    fn push(
        &mut self,
        header: MediaHeader,
        part: u32,
        data: Vec<u8>,
    ) -> Result<Option<PendingMedia>> {
        if self.active != Some(header.stream) {
            return Ok(None);
        }
        if header.parts == 0 || header.parts > 2048 || part >= header.parts || data.len() > 8192 {
            return Err(failure("invalid media fragment"));
        }
        let parts = header.parts;
        if part == 0 {
            if self.pending.is_some() {
                return Err(failure("incomplete media packet"));
            }
            self.pending = Some(PendingMedia {
                header,
                next: 0,
                data: Vec::new(),
            });
        } else if self.pending.as_ref().map(|p| &p.header) != Some(&header) {
            return Err(failure("media fragments disagree"));
        }
        let packet = self
            .pending
            .as_mut()
            .ok_or_else(|| failure("media fragment has no first part"))?;
        if packet.next != part {
            return Err(failure("media fragments are out of order"));
        }
        packet.data.extend_from_slice(&data);
        packet.next += 1;
        Ok(if packet.next == parts {
            self.pending.take()
        } else {
            None
        })
    }
}

async fn mirror(
    reader: &mut SecureReader,
    writer: &mut SecureWriter,
    seconds: u64,
    output: Option<&Path>,
) -> Result<()> {
    let mut video = None;
    let mut audio = None;
    if let Some(directory) = output {
        fs::create_dir_all(directory)?;
        video = Some(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("screen.h264"))?,
        );
        audio = Some(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("system-audio.f32le"))?,
        );
    }
    let start = Instant::now();
    let (mut frames, mut audio_packets, mut bytes, mut configs) = (0u64, 0u64, 0u64, 0u64);
    let mut assembly = MediaAssembly::default();
    let collection = async {
        loop {
            match receive(reader, writer).await? {
                Message::Media {
                    stream_id,
                    packet_id,
                    part,
                    parts,
                    media_kind,
                    codec,
                    pts_us,
                    keyframe,
                    width,
                    height,
                    data,
                    ..
                } => {
                    let header = MediaHeader {
                        stream: stream_id,
                        packet: packet_id,
                        parts,
                        kind: media_kind,
                        codec,
                        pts: pts_us,
                        keyframe,
                        width,
                        height,
                    };
                    if let Some(packet) = assembly.push(header, part, data)? {
                        bytes += packet.data.len() as u64;
                        match packet.header.kind.as_str() {
                            "video_config" if packet.header.codec == "h264-avcc" => {
                                let annex = avcc_configuration(&packet.data)?;
                                if let Some(file) = &mut video {
                                    file.write_all(&annex)?;
                                }
                                configs += 1;
                                println!(
                                    "{}",
                                    serde_json::json!({"config":"video","codec":packet.header.codec,"width":packet.header.width,"height":packet.header.height,"bytes":packet.data.len()})
                                );
                            }
                            "audio_config" if packet.header.codec == "pcm-f32le" => {
                                let config: serde_json::Value =
                                    serde_json::from_slice(&packet.data)
                                        .map_err(|_| failure("invalid audio configuration"))?;
                                configs += 1;
                                println!(
                                    "{}",
                                    serde_json::json!({"config":"audio","format":config})
                                );
                            }
                            "video" if packet.header.codec == "h264-avcc" => {
                                let annex = avcc_frame(&packet.data)?;
                                if let Some(file) = &mut video {
                                    file.write_all(&annex)?;
                                }
                                frames += 1;
                            }
                            "audio" if packet.header.codec == "pcm-f32le" => {
                                if let Some(file) = &mut audio {
                                    file.write_all(&packet.data)?;
                                }
                                audio_packets += 1;
                            }
                            _ => return Err(failure("unsupported media format")),
                        }
                    }
                }
                Message::MirrorState {
                    status, stream_id, ..
                } => {
                    assembly.state(&status, stream_id);
                    println!("{}", serde_json::json!({"mirror_state":status}))
                }
                _ => return Err(failure("unexpected mirror response")),
            }
        }
        #[allow(unreachable_code)]
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    };
    if let Ok(result) = tokio::time::timeout(Duration::from_secs(seconds), collection).await {
        result?;
    }
    if let Some(file) = video {
        file.sync_all()?;
    }
    if let Some(file) = audio {
        file.sync_all()?;
    }
    println!(
        "{}",
        serde_json::json!({"elapsed_seconds":start.elapsed().as_secs_f64(),"video_frames":frames,"audio_packets":audio_packets,"config_packets":configs,"bytes":bytes})
    );
    if frames == 0 {
        return Err(failure("no complete video frames received"));
    }
    Ok(())
}

fn avcc_frame(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut remaining = data;
    while !remaining.is_empty() {
        if remaining.len() < 4 {
            return Err(failure("truncated AVCC NAL length"));
        }
        let length = u32::from_be_bytes(
            remaining[..4]
                .try_into()
                .expect("slice is exactly four bytes"),
        ) as usize;
        remaining = &remaining[4..];
        if length == 0 || length > remaining.len() {
            return Err(failure("invalid AVCC NAL length"));
        }
        output.extend_from_slice(&[0, 0, 0, 1]);
        output.extend_from_slice(&remaining[..length]);
        remaining = &remaining[length..];
    }
    Ok(output)
}

fn avcc_configuration(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 7 || data[0] != 1 || data[4] & 3 != 3 {
        return Err(failure("unsupported AVC decoder configuration"));
    }
    let mut remaining = &data[6..];
    let mut output = Vec::new();
    let mut count = usize::from(data[5] & 31);
    for group in 0..2 {
        if count == 0 {
            return Err(failure("missing H.264 parameter sets"));
        }
        for _ in 0..count {
            if remaining.len() < 2 {
                return Err(failure("truncated H.264 parameter set"));
            }
            let length = u16::from_be_bytes(
                remaining[..2]
                    .try_into()
                    .expect("slice is exactly two bytes"),
            ) as usize;
            remaining = &remaining[2..];
            if length == 0 || length > remaining.len() {
                return Err(failure("invalid H.264 parameter set length"));
            }
            output.extend_from_slice(&[0, 0, 0, 1]);
            output.extend_from_slice(&remaining[..length]);
            remaining = &remaining[length..];
        }
        if group == 0 {
            let Some((&pps, tail)) = remaining.split_first() else {
                return Err(failure("missing H.264 PPS count"));
            };
            count = usize::from(pps);
            remaining = tail;
        }
    }
    if !remaining.is_empty() {
        return Err(failure("unsupported AVC configuration extension"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarting_mirror_discards_old_partial_and_ignores_late_old_fragments() {
        fn header(stream: u64) -> MediaHeader {
            MediaHeader {
                stream,
                packet: 0,
                parts: 2,
                kind: "video".into(),
                codec: "h264-avcc".into(),
                pts: 0,
                keyframe: true,
                width: 640,
                height: 480,
            }
        }
        let mut assembly = MediaAssembly::default();
        assembly.state("running", 1);
        assert!(assembly.push(header(1), 0, vec![1]).unwrap().is_none());
        assembly.state("paused", 1);
        assert!(assembly.push(header(1), 1, vec![9]).unwrap().is_none());
        assembly.state("running", 2);
        assert!(assembly.push(header(2), 0, vec![2]).unwrap().is_none());
        assert!(assembly.push(header(1), 1, vec![9]).unwrap().is_none());
        assembly.state("paused", 1);
        assembly.state("stopped", 1);
        let packet = assembly.push(header(2), 1, vec![3]).unwrap().unwrap();
        assert_eq!(packet.data, [2, 3]);
        assert!(assembly.push(header(2), 1, vec![3]).is_err());
        assembly.state("stopped", 2);
        assert!(assembly.push(header(2), 0, vec![2]).unwrap().is_none());
    }

    #[test]
    fn mirror_controls_have_no_remote_start_and_validate_display_id() {
        let args = |values: &[&str]| {
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        };
        assert!(matches!(
            mirror_action(&args(&["pause"])).unwrap(),
            MirrorAction::Pause
        ));
        assert!(matches!(
            mirror_action(&args(&["resume"])).unwrap(),
            MirrorAction::Resume
        ));
        assert!(matches!(
            mirror_action(&args(&["stop"])).unwrap(),
            MirrorAction::Stop
        ));
        assert!(matches!(
            mirror_action(&args(&["display", "42"])).unwrap(),
            MirrorAction::SelectDisplay { display_id: 42 }
        ));
        for invalid in [
            &["start"][..],
            &["display"][..],
            &["display", "bad"][..],
            &["resume", "extra"][..],
        ] {
            assert!(mirror_action(&args(invalid)).is_err());
        }
    }

    #[test]
    fn partial_download_resumes_validates_chunks_and_never_overwrites() {
        let directory =
            std::env::temp_dir().join(format!("ymlink-stub-{:016x}", rand::random::<u64>()));
        fs::create_dir(&directory).unwrap();
        let destination = directory.join("movie");
        let version = "a".repeat(64);
        let mut download =
            Download::open(&destination, "desktop-a", "share", "movie", &version, 6).unwrap();
        download.append(0, 6, &version, b"abc", false).unwrap();
        assert!(download.append(0, 6, &version, b"bad", false).is_err());
        drop(download);
        let part = sidecar(&destination, ".part");
        let metadata = sidecar(&destination, ".part.json");
        let saved_metadata = fs::read(&metadata).unwrap();
        assert!(Download::open(&destination, "desktop-b", "share", "movie", &version, 6).is_err());
        assert_eq!(fs::read(&part).unwrap(), b"abc");
        assert_eq!(fs::read(&metadata).unwrap(), saved_metadata);
        let saved_state: serde_json::Value = serde_json::from_slice(&saved_metadata).unwrap();
        assert_eq!(saved_state["offset"], 3);
        assert_eq!(saved_state["source_device_id"], "desktop-a");
        for (schema, missing_source) in [(2, true), (2, false), (3, true)] {
            let mut old_state = saved_state.clone();
            old_state["v"] = serde_json::json!(schema);
            if missing_source {
                old_state
                    .as_object_mut()
                    .unwrap()
                    .remove("source_device_id");
            }
            let old_metadata = serde_json::to_vec(&old_state).unwrap();
            fs::write(&metadata, &old_metadata).unwrap();
            assert!(
                Download::open(&destination, "desktop-a", "share", "movie", &version, 6).is_err()
            );
            assert_eq!(fs::read(&part).unwrap(), b"abc");
            assert_eq!(fs::read(&metadata).unwrap(), old_metadata);
        }
        fs::write(&metadata, &saved_metadata).unwrap();
        assert!(Download::open(
            &destination,
            "desktop-a",
            "share",
            "movie",
            &"b".repeat(64),
            6
        )
        .is_err());
        let mut resumed =
            Download::open(&destination, "desktop-a", "share", "movie", &version, 6).unwrap();
        assert_eq!(resumed.state.offset, 3);
        resumed.append(3, 6, &version, b"def", true).unwrap();
        fs::write(&destination, b"keep me").unwrap();
        assert!(resumed.finish(&destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"keep me");
        fs::remove_file(&destination).unwrap();
        Download::open(&destination, "desktop-a", "share", "movie", &version, 6)
            .unwrap()
            .finish(&destination)
            .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"abcdef");
        assert!(Download::open(&destination, "desktop-a", "share", "movie", &version, 6).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn h264_conversion_rejects_truncation_and_preserves_nals() {
        assert_eq!(
            avcc_frame(&[0, 0, 0, 2, 0x65, 0x10]).unwrap(),
            [0, 0, 0, 1, 0x65, 0x10]
        );
        assert!(avcc_frame(&[0, 0, 1]).is_err());
        assert!(avcc_frame(&[0, 0, 0, 3, 0x65]).is_err());
        let configuration = [1, 100, 0, 31, 0xff, 0xe1, 0, 2, 0x67, 100, 1, 0, 1, 0x68];
        assert_eq!(
            avcc_configuration(&configuration).unwrap(),
            [0, 0, 0, 1, 0x67, 100, 0, 0, 0, 1, 0x68]
        );
        for end in 0..configuration.len() {
            assert!(avcc_configuration(&configuration[..end]).is_err());
        }
    }
}
