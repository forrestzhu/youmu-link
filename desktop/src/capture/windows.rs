//! WGC display capture, synchronous Media Foundation H.264, WASAPI loopback.
//! COM objects stay on their owning worker; only bounded media packets cross threads.

use super::{Display, Failure, MediaPacket};
use std::mem::{size_of, ManuallyDrop};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use windows::core::{factory, Interface, VARIANT};
use windows::Foundation::{EventRegistrationToken, TypedEventHandler};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem,
    GraphicsCaptureSession, IDirect3D11CaptureFramePoolStatics2, IGraphicsCaptureSessionStatics,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{
    BOOL, E_BOUNDS, E_POINTER, HMODULE, LPARAM, RECT, RPC_E_CHANGED_MODE,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

const MAX_VIDEO_BYTES: usize = 8 * 1024 * 1024;
const AUDIO_CONFIG: &[u8] =
    br#"{"sample_rate":48000,"channels":2,"sample_format":"f32le","interleaved":true}"#;

fn native<T>(result: windows::core::Result<T>, message: &str) -> Result<T, String> {
    // No OS error strings: drivers may include file paths or device identifiers.
    result.map_err(|_| message.to_owned())
}

struct Apartment(bool);
impl Apartment {
    fn new() -> Result<Self, String> {
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(Self(true)),
            // Read-only support queries may run on the native UI's STA.
            Err(error) if error.code() == RPC_E_CHANGED_MODE => Ok(Self(false)),
            Err(_) => Err("Unable to initialize Windows capture services".into()),
        }
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { RoUninitialize() };
        }
    }
}

pub fn permission_granted() -> bool {
    let Ok(_apartment) = Apartment::new() else {
        return false;
    };
    // Win32 CreateForMonitor has no recording permission prompt. User consent is
    // enforced by the desktop runtime before Capture::start is called.
    // Generated static methods retain an agile factory globally. The native
    // backend can repeatedly create/destroy the last MTA thread; that factory
    // then outlives COM and its next use can access freed Windows state.
    // An uncached factory is released before this temporary apartment ends.
    let Ok(factory) = factory::<GraphicsCaptureSession, IGraphicsCaptureSessionStatics>() else {
        return false;
    };
    let mut supported = false;
    unsafe { (factory.vtable().IsSupported)(factory.as_raw(), &mut supported).is_ok() && supported }
}

fn monitors() -> Result<Vec<(Display, HMONITOR)>, String> {
    unsafe extern "system" fn monitor(
        handle: HMONITOR,
        _: HDC,
        _: *mut RECT,
        context: LPARAM,
    ) -> BOOL {
        let result = &mut *(context.0 as *mut Vec<(Display, HMONITOR)>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(handle, &mut info.monitorInfo).as_bool() {
            let end = info.szDevice.iter().position(|&c| c == 0).unwrap_or(32);
            let device = String::from_utf16_lossy(&info.szDevice[..end]);
            // DISPLAYn persists across enumeration ordering, unlike a Vec index.
            if let Some(id) = device.rsplit("DISPLAY").next().and_then(|n| n.parse().ok()) {
                let rect = info.monitorInfo.rcMonitor;
                if rect.right > rect.left && rect.bottom > rect.top {
                    result.push((
                        Display {
                            id,
                            name: format!("Display {id}"),
                            width: (rect.right - rect.left) as u32,
                            height: (rect.bottom - rect.top) as u32,
                        },
                        handle,
                    ));
                }
            }
        }
        BOOL(1)
    }
    let mut result: Vec<(Display, HMONITOR)> = Vec::new();
    if !unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(monitor),
            LPARAM((&mut result as *mut Vec<(Display, HMONITOR)>) as isize),
        )
    }
    .as_bool()
    {
        return Err("Unable to list displays".into());
    }
    result.sort_by_key(|(display, _)| display.id);
    Ok(result)
}

pub fn displays() -> Result<Vec<Display>, String> {
    Ok(monitors()?
        .into_iter()
        .map(|(display, _)| display)
        .collect())
}

#[derive(Clone, Copy)]
struct Profile {
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
}

fn profile(width: u32, height: u32, quality: &str, cap: Option<u32>) -> Result<Profile, String> {
    let (maximum, fps, bitrate) = match quality {
        "smooth" => (1280, 30, 4_000_000u32),
        "balanced" => (1920, 60, 12_000_000),
        "quality" => (2560, 60, 24_000_000),
        _ => return Err("Unknown mirror quality".into()),
    };
    if width < 2 || height < 2 || width > 32768 || height > 32768 {
        return Err("Display dimensions are invalid".into());
    }
    if cap.is_some_and(|edge| edge < 2) {
        return Err("Mirror size limit is invalid".into());
    }
    let longest = width.max(height);
    let scale = |value: u32, maximum: u32| {
        let value = u64::from(value) * u64::from(longest.min(maximum)) / u64::from(longest);
        ((value as u32) & !1).max(2)
    };
    let base = u64::from(scale(width, maximum)) * u64::from(scale(height, maximum));
    let maximum = maximum.min(cap.unwrap_or(maximum));
    let (width, height) = (scale(width, maximum), scale(height, maximum));
    Ok(Profile {
        width,
        height,
        fps,
        bitrate: (u64::from(bitrate) * u64::from(width) * u64::from(height) / base)
            .clamp(256_000, u64::from(bitrate)) as u32,
    })
}

struct Output {
    sender: Sender<MediaPacket>,
    runtime: tokio::runtime::Handle,
    running: AtomicBool,
    need_keyframe: AtomicBool,
    attempts: AtomicU64,
    drops: AtomicU64,
    failure: Arc<Failure>,
    profile: Profile,
    epoch: u64,
}

impl Output {
    fn active(&self) -> bool {
        self.running.load(Ordering::SeqCst) && self.failure.message().is_none()
    }

    fn fail(&self, message: &str) {
        let running = {
            let mut failure = self.failure.0.lock().unwrap_or_else(|e| e.into_inner());
            if failure.is_some() {
                return;
            }
            *failure = Some(message.into());
            self.running.load(Ordering::SeqCst)
        };
        if running {
            let sender = self.sender.clone();
            let packet = self.packet("error", "", 0, false, message.as_bytes().to_vec());
            self.runtime.spawn(async move {
                let _ = sender.send(packet).await;
            });
        }
    }

    fn packet(&self, kind: &str, codec: &str, pts: u64, key: bool, data: Vec<u8>) -> MediaPacket {
        MediaPacket {
            kind: kind.into(),
            codec: codec.into(),
            pts_us: pts,
            keyframe: key,
            width: if kind.starts_with("video") {
                self.profile.width
            } else {
                0
            },
            height: if kind.starts_with("video") {
                self.profile.height
            } else {
                0
            },
            data,
        }
    }

    fn send_video(&self, pts: u64, key: bool, data: Vec<u8>, config: &[u8]) {
        if !self.active() {
            return;
        }
        if self.need_keyframe.load(Ordering::SeqCst) && !key {
            self.drops.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let packet = self.packet("video", "h264-avcc", pts, key, data);
        let sent = if key {
            if let Ok(mut permits) = self.sender.try_reserve_many(2) {
                permits.next().unwrap().send(self.packet(
                    "video_config",
                    "h264-avcc",
                    pts,
                    true,
                    config.to_vec(),
                ));
                permits.next().unwrap().send(packet);
                true
            } else {
                false
            }
        } else {
            self.sender.try_send(packet).is_ok()
        };
        self.need_keyframe.store(!sent, Ordering::SeqCst);
        if !sent {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn qpc_time() -> Result<u64, String> {
    let (mut counter, mut frequency) = (0, 0);
    unsafe {
        native(
            QueryPerformanceCounter(&mut counter),
            "Unable to read media clock",
        )?;
        native(
            QueryPerformanceFrequency(&mut frequency),
            "Unable to read media clock",
        )?;
    }
    if counter < 0 || frequency <= 0 {
        return Err("Windows media clock is invalid".into());
    }
    Ok((counter as u128 * 10_000_000 / frequency as u128) as u64)
}

/// BT.601 limited-range NV12, matching MF's default matrix for this input type.
/// ponytail: CPU nearest-neighbor resize/readback; use GPU conversion if measured throughput needs it.
fn bgra_to_nv12(
    bytes: &[u8],
    width: u32,
    height: u32,
    pitch: usize,
    out: Profile,
) -> Result<Vec<u8>, String> {
    let (width, height) = (width as usize, height as usize);
    let (ow, oh) = (out.width as usize, out.height as usize);
    if width < 2
        || height < 2
        || pitch < width * 4
        || pitch
            .checked_mul(height)
            .is_none_or(|length| bytes.len() < length)
        || ow < 2
        || oh < 2
        || ow % 2 != 0
        || oh % 2 != 0
        || ow > width
        || oh > height
    {
        return Err("Invalid captured screen buffer".into());
    }
    let mut result = vec![0; ow * oh * 3 / 2];
    for y in (0..oh).step_by(2) {
        for x in (0..ow).step_by(2) {
            let (mut rsum, mut gsum, mut bsum) = (0, 0, 0);
            for dy in 0..2 {
                for dx in 0..2 {
                    let at = ((y + dy) * height / oh) * pitch + ((x + dx) * width / ow) * 4;
                    let (b, g, r) = (bytes[at] as i32, bytes[at + 1] as i32, bytes[at + 2] as i32);
                    result[(y + dy) * ow + x + dx] =
                        (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16).clamp(16, 235) as u8;
                    rsum += r;
                    gsum += g;
                    bsum += b;
                }
            }
            let (r, g, b) = (rsum / 4, gsum / 4, bsum / 4);
            let uv = ow * oh + y / 2 * ow + x;
            result[uv] = (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128).clamp(16, 240) as u8;
            result[uv + 1] = (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128).clamp(16, 240) as u8;
        }
    }
    Ok(result)
}

fn annex_b_nals(data: &[u8]) -> Result<Vec<&[u8]>, String> {
    if data.is_empty() || data.len() > MAX_VIDEO_BYTES {
        return Err("Invalid encoded H.264 packet size".into());
    }
    let mut result = Vec::new();
    let mut start = None;
    let mut at = 0;
    while at + 3 <= data.len() {
        let prefix = if data[at..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if data[at..].starts_with(&[0, 0, 1]) {
            3
        } else {
            at += 1;
            continue;
        };
        if let Some(previous) = start {
            if previous == at {
                return Err("Empty H.264 NAL unit".into());
            }
            result.push(&data[previous..at]);
        } else if data[..at].iter().any(|&byte| byte != 0) {
            return Err("Unsupported H.264 byte stream".into());
        }
        at += prefix;
        start = Some(at);
    }
    let start = start.ok_or("H.264 start code is missing")?;
    if start >= data.len() {
        return Err("Empty H.264 NAL unit".into());
    }
    result.push(&data[start..]);
    if result
        .iter()
        .any(|nal| nal[0] & 0x80 != 0 || nal[0] & 31 == 0)
    {
        return Err("Invalid H.264 NAL unit".into());
    }
    Ok(result)
}

// Bounded Main-profile SPS syntax (ITU-T H.264 7.3.2.1, E.1.1). The Windows
// software encoder omits color VUI even with explicit MF color attributes.
// Normalize metadata only; preserve every other syntax bit and reject malformed
// SPS before emitting either decoder configuration or an in-band parameter set.
struct SpsBits {
    bits: Vec<bool>,
    position: usize,
}

impl SpsBits {
    fn read(&mut self, count: usize) -> Result<u32, String> {
        if count > 32 || self.position + count >= self.bits.len() {
            return Err("Unsupported or truncated H.264 SPS".into());
        }
        let mut value = 0;
        for &bit in &self.bits[self.position..self.position + count] {
            value = (value << 1) | u32::from(bit);
        }
        self.position += count;
        Ok(value)
    }

    fn flag(&mut self) -> Result<bool, String> {
        Ok(self.read(1)? != 0)
    }

    fn ue(&mut self, maximum: u32) -> Result<u32, String> {
        let mut zeros = 0;
        while !self.flag()? {
            zeros += 1;
            if zeros > 31 {
                return Err("Invalid H.264 SPS Exp-Golomb value".into());
            }
        }
        let value = (1u32 << zeros) - 1 + self.read(zeros)?;
        if value > maximum {
            return Err("Unsupported H.264 SPS value".into());
        }
        Ok(value)
    }

    fn hrd(&mut self) -> Result<(), String> {
        let count = self.ue(31)? + 1;
        self.read(8)?; // bit_rate_scale, cpb_size_scale
        for _ in 0..count {
            self.ue(u32::MAX)?;
            self.ue(u32::MAX)?;
            self.flag()?;
        }
        self.read(20)?; // four delay length fields
        Ok(())
    }
}

fn append_bits(bits: &mut Vec<bool>, value: u32, count: usize) {
    bits.extend((0..count).rev().map(|shift| value & (1 << shift) != 0));
}

fn normalize_sps(sps: &[u8]) -> Result<Vec<u8>, String> {
    if sps.len() < 5
        || sps.len() > u16::MAX as usize
        || sps[0] & 0x80 != 0
        || sps[0] & 31 != 7
        || sps[0] & 0x60 == 0
    {
        return Err("Invalid H.264 SPS".into());
    }
    let (mut rbsp, mut zeros) = (Vec::new(), 0);
    for (index, &byte) in sps[1..].iter().enumerate() {
        if zeros == 2 {
            if byte == 3 {
                if sps.get(index + 2).is_none_or(|&next| next > 3) {
                    return Err("Invalid H.264 SPS emulation prevention".into());
                }
                zeros = 0;
                continue;
            }
            if byte < 3 {
                return Err("Missing H.264 SPS emulation prevention".into());
            }
        }
        rbsp.push(byte);
        zeros = if byte == 0 { zeros + 1 } else { 0 };
    }
    let mut bits = SpsBits {
        bits: Vec::with_capacity(rbsp.len() * 8),
        position: 0,
    };
    for byte in rbsp {
        append_bits(&mut bits.bits, u32::from(byte), 8);
    }
    // Remove alignment zeros, retaining the final rbsp_stop_one_bit.
    while bits.bits.last() == Some(&false) {
        bits.bits.pop();
    }
    if bits.read(8)? != 77 {
        return Err("Unsupported H.264 SPS profile".into());
    }
    bits.read(16)?; // constraint flags, level_idc
    bits.ue(31)?; // seq_parameter_set_id
    bits.ue(12)?; // log2_max_frame_num_minus4
    match bits.ue(2)? {
        // pic_order_cnt_type
        0 => {
            bits.ue(12)?;
        }
        1 => {
            bits.flag()?;
            bits.ue(u32::MAX)?; // signed offsets have the same bit lengths
            bits.ue(u32::MAX)?;
            for _ in 0..bits.ue(255)? {
                bits.ue(u32::MAX)?;
            }
        }
        _ => {}
    }
    bits.ue(16)?; // max_num_ref_frames
    bits.flag()?; // gaps_in_frame_num_value_allowed_flag
    bits.ue(2047)?; // pic_width_in_mbs_minus1
    bits.ue(2047)?; // pic_height_in_map_units_minus1
    if !bits.flag()? {
        bits.flag()?;
    } // frame_mbs_only / mb_adaptive_frame_field
    bits.flag()?; // direct_8x8_inference_flag
    if bits.flag()? {
        for _ in 0..4 {
            bits.ue(32768)?;
        }
    }
    let vui_flag = bits.position;
    let has_vui = bits.flag()?;
    let mut color_range = bits.position..bits.position;
    if has_vui {
        if bits.flag()? && bits.read(8)? == 255 {
            if bits.read(16)? == 0 || bits.read(16)? == 0 {
                return Err("Invalid H.264 SPS aspect ratio".into());
            }
        }
        if bits.flag()? {
            bits.flag()?;
        } // overscan information
        let color_start = bits.position;
        if bits.flag()? {
            // video_signal_type_present_flag
            bits.read(4)?; // video_format, video_full_range_flag
            if bits.flag()? {
                bits.read(24)?;
            }
        }
        color_range = color_start..bits.position;
        if bits.flag()? {
            bits.ue(5)?;
            bits.ue(5)?;
        } // chroma sample location
        if bits.flag()? {
            // timing_info_present_flag
            if bits.read(32)? == 0 || bits.read(32)? == 0 {
                return Err("Invalid H.264 SPS timing".into());
            }
            bits.flag()?;
        }
        let nal_hrd = bits.flag()?;
        if nal_hrd {
            bits.hrd()?;
        }
        let vcl_hrd = bits.flag()?;
        if vcl_hrd {
            bits.hrd()?;
        }
        if nal_hrd || vcl_hrd {
            bits.flag()?;
        }
        bits.flag()?; // pic_struct_present_flag
        if bits.flag()? {
            // bitstream_restriction_flag
            bits.flag()?;
            for _ in 0..4 {
                bits.ue(16)?;
            }
            let reorder = bits.ue(16)?;
            let buffer = bits.ue(16)?;
            if reorder > buffer {
                return Err("Invalid H.264 SPS reorder limit".into());
            }
        }
    }
    if bits.position + 1 != bits.bits.len() || bits.bits.last() != Some(&true) {
        return Err("Invalid H.264 SPS trailing bits".into());
    }
    let mut color = Vec::new();
    color.push(true); // video_signal_type_present_flag
    append_bits(&mut color, 5, 3); // unspecified video format
    color.push(false); // limited range
    color.push(true); // colour_description_present_flag
    append_bits(&mut color, 2, 8); // unspecified primaries: the matrix does not define source gamut
    append_bits(&mut color, 2, 8); // unspecified transfer characteristics
    append_bits(&mut color, 6, 8); // BT.601 / SMPTE 170M matrix
    if !has_vui {
        bits.bits[vui_flag] = true;
        let mut vui = vec![false, false]; // no aspect ratio or overscan metadata
        vui.extend(color);
        vui.extend([false; 6]); // chroma location, timing, HRDs, picture structure, restrictions
        bits.bits.splice(color_range, vui);
    } else {
        bits.bits.splice(color_range, color);
    }
    while bits.bits.len() % 8 != 0 {
        bits.bits.push(false);
    }
    let (mut result, mut zeros) = (vec![sps[0]], 0);
    for chunk in bits.bits.chunks_exact(8) {
        let byte = chunk
            .iter()
            .fold(0u8, |value, &bit| (value << 1) | u8::from(bit));
        if zeros == 2 && byte <= 3 {
            result.push(3);
            zeros = 0;
        }
        result.push(byte);
        zeros = if byte == 0 { zeros + 1 } else { 0 };
    }
    if result.len() > u16::MAX as usize {
        return Err("H.264 SPS is too large".into());
    }
    Ok(result)
}

fn avcc_config(nals: &[&[u8]]) -> Result<Vec<u8>, String> {
    if nals.iter().any(|nal| nal.is_empty()) {
        return Err("Empty H.264 parameter set".into());
    }
    let sps = nals
        .iter()
        .copied()
        .find(|nal| nal[0] & 31 == 7)
        .ok_or("H.264 SPS is missing")?;
    let pps = nals
        .iter()
        .copied()
        .find(|nal| nal[0] & 31 == 8)
        .ok_or("H.264 PPS is missing")?;
    if sps.len() < 4 || sps.len() > u16::MAX as usize || pps.len() > u16::MAX as usize {
        return Err("Invalid H.264 parameter sets".into());
    }
    let sps = normalize_sps(sps)?;
    let mut result = vec![1, sps[1], sps[2], sps[3], 0xff, 0xe1];
    result.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    result.extend_from_slice(&sps);
    result.push(1);
    result.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    result.extend_from_slice(pps);
    Ok(result)
}

fn avcc_frame(nals: &[&[u8]]) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    for nal in nals {
        let normalized;
        let nal = if nal.first().is_some_and(|byte| byte & 31 == 7) {
            normalized = normalize_sps(nal)?;
            &normalized[..]
        } else {
            nal
        };
        result.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        result.extend_from_slice(nal);
    }
    Ok(result)
}

struct MediaFoundation;
impl MediaFoundation {
    fn new() -> Result<Self, String> {
        native(
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) },
            "Media Foundation is unavailable",
        )?;
        Ok(Self)
    }
}
impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

// Built-in synchronous Windows H.264 encoder. This deliberately avoids async
// hardware transforms until GPU/driver behavior is measured on supported PCs.
struct Encoder {
    transform: IMFTransform,
    codec: ICodecAPI,
    output: Arc<Output>,
    config: Vec<u8>,
    stopped: bool,
}

impl Encoder {
    fn new(output: Arc<Output>) -> Result<Self, String> {
        let transform: IMFTransform = native(
            unsafe { CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER) },
            "Windows H.264 encoding is unavailable; install Media Feature Pack on Windows N",
        )?;
        let codec: ICodecAPI = native(
            transform.cast(),
            "Unable to configure Windows H.264 encoder",
        )?;
        let profile = output.profile;
        unsafe {
            native(
                codec.SetValue(&CODECAPI_AVLowLatencyMode, &VARIANT::from(true)),
                "Unable to enable low latency H.264",
            )?;
            native(
                codec.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &VARIANT::from(0u32)),
                "Unable to disable H.264 frame reordering",
            )?;
            native(
                codec.SetValue(&CODECAPI_AVEncMPVGOPSize, &VARIANT::from(profile.fps * 2)),
                "Unable to set H.264 keyframe interval",
            )?;
            let encoded = Self::media_type(profile, MFVideoFormat_H264)?;
            native(
                encoded.SetUINT32(&MF_MT_AVG_BITRATE, profile.bitrate),
                "Unable to set H.264 bitrate",
            )?;
            native(
                encoded.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Main.0 as u32),
                "Unable to set H.264 profile",
            )?;
            native(
                transform.SetOutputType(0, &encoded, 0),
                "H.264 encoder does not support the selected quality",
            )?;
            let input = Self::media_type(profile, MFVideoFormat_NV12)?;
            native(
                transform.SetInputType(0, &input, 0),
                "H.264 encoder does not support NV12 input",
            )?;
            native(
                transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0),
                "Unable to prepare H.264 encoding",
            )?;
            native(
                transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0),
                "Unable to start H.264 encoding",
            )?;
        }
        Ok(Self {
            transform,
            codec,
            output,
            config: Vec::new(),
            stopped: false,
        })
    }

    unsafe fn media_type(
        profile: Profile,
        subtype: windows::core::GUID,
    ) -> Result<IMFMediaType, String> {
        let media = native(MFCreateMediaType(), "Unable to create video format")?;
        native(
            media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video),
            "Unable to set video format",
        )?;
        native(
            media.SetGUID(&MF_MT_SUBTYPE, &subtype),
            "Unable to set video format",
        )?;
        native(
            media.SetUINT64(
                &MF_MT_FRAME_SIZE,
                (u64::from(profile.width) << 32) | u64::from(profile.height),
            ),
            "Unable to set video size",
        )?;
        native(
            media.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(profile.fps) << 32) | 1),
            "Unable to set video frame rate",
        )?;
        native(
            media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1),
            "Unable to set pixel aspect ratio",
        )?;
        native(
            media.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32),
            "Unable to set progressive video",
        )?;
        // bgra_to_nv12 writes BT.601 studio-range samples. Declare the same
        // matrix/range on input and output; normalize_sps also enforces the VUI
        // because the built-in encoder ignores these attributes in its bitstream.
        native(
            media.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT601.0 as u32),
            "Unable to set video color matrix",
        )?;
        native(
            media.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32),
            "Unable to set video color range",
        )?;
        Ok(media)
    }

    fn encode(&mut self, data: &[u8], pts: u64) -> Result<(), String> {
        let profile = self.output.profile;
        if data.len() != (profile.width * profile.height * 3 / 2) as usize {
            return Err("Invalid encoder input buffer".into());
        }
        unsafe {
            if self.output.need_keyframe.load(Ordering::SeqCst) {
                native(
                    self.codec
                        .SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &VARIANT::from(1u32)),
                    "Unable to request H.264 keyframe",
                )?;
            }
            let sample = native(MFCreateSample(), "Unable to allocate video sample")?;
            let buffer = native(
                MFCreateMemoryBuffer(data.len() as u32),
                "Unable to allocate video buffer",
            )?;
            let mut bytes = ptr::null_mut();
            native(
                buffer.Lock(&mut bytes, None, None),
                "Unable to write video buffer",
            )?;
            ptr::copy_nonoverlapping(data.as_ptr(), bytes, data.len());
            native(buffer.Unlock(), "Unable to unlock video buffer")?;
            native(
                buffer.SetCurrentLength(data.len() as u32),
                "Unable to set video buffer length",
            )?;
            native(sample.AddBuffer(&buffer), "Unable to prepare video buffer")?;
            native(
                sample.SetSampleTime((pts * 10) as i64),
                "Unable to set video timestamp",
            )?;
            native(
                sample.SetSampleDuration(10_000_000 / profile.fps as i64),
                "Unable to set video duration",
            )?;
            match self.transform.ProcessInput(0, &sample, 0) {
                Err(error) if error.code() == MF_E_NOTACCEPTING => {
                    self.receive()?;
                    native(
                        self.transform.ProcessInput(0, &sample, 0),
                        "H.264 encoder rejected captured video",
                    )?;
                }
                result => {
                    native(result, "H.264 encoder rejected captured video")?;
                }
            }
        }
        self.receive()
    }

    fn receive(&mut self) -> Result<(), String> {
        // The synchronous built-in transform produces one output per input;
        // bound draining so a malfunctioning codec cannot keep teardown spinning.
        for _ in 0..64 {
            unsafe {
                let info = native(
                    self.transform.GetOutputStreamInfo(0),
                    "Unable to query H.264 output",
                )?;
                if info.cbSize as usize > MAX_VIDEO_BYTES {
                    return Err("H.264 encoder requested an oversized output buffer".into());
                }
                let sample = native(MFCreateSample(), "Unable to allocate H.264 sample")?;
                let buffer = native(
                    MFCreateMemoryBuffer(info.cbSize.max(1)),
                    "Unable to allocate H.264 output",
                )?;
                native(sample.AddBuffer(&buffer), "Unable to prepare H.264 output")?;
                let mut slots = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(Some(sample)),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut status = 0;
                let result = self.transform.ProcessOutput(0, &mut slots, &mut status);
                let sample = ManuallyDrop::take(&mut slots[0].pSample);
                drop(ManuallyDrop::take(&mut slots[0].pEvents));
                match result {
                    Err(error) if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                    Err(error) if error.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                        let format = native(
                            self.transform.GetOutputAvailableType(0, 0),
                            "Unable to refresh H.264 format",
                        )?;
                        native(
                            self.transform.SetOutputType(0, &format, 0),
                            "Unable to refresh H.264 format",
                        )?;
                        continue;
                    }
                    result => {
                        native(result, "Windows H.264 encoding failed")?;
                    }
                }
                let sample = sample.ok_or("H.264 encoder returned no sample")?;
                let buffer = native(
                    sample.ConvertToContiguousBuffer(),
                    "Unable to read H.264 output",
                )?;
                let length =
                    native(buffer.GetCurrentLength(), "Unable to read H.264 size")? as usize;
                if length == 0 || length > MAX_VIDEO_BYTES {
                    return Err("Invalid encoded H.264 frame size".into());
                }
                let mut bytes = ptr::null_mut();
                native(
                    buffer.Lock(&mut bytes, None, None),
                    "Unable to read H.264 data",
                )?;
                let data = std::slice::from_raw_parts(bytes, length).to_vec();
                native(buffer.Unlock(), "Unable to unlock H.264 data")?;
                let nals = annex_b_nals(&data)?;
                let key = sample.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0;
                if key {
                    let format = native(
                        self.transform.GetOutputCurrentType(0),
                        "Unable to read H.264 configuration",
                    )?;
                    let size = native(
                        format.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER),
                        "H.264 configuration is unavailable",
                    )? as usize;
                    if size == 0 || size > 131072 {
                        return Err("Invalid H.264 configuration size".into());
                    }
                    let mut header = vec![0; size];
                    native(
                        format.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut header, None),
                        "Unable to read H.264 configuration",
                    )?;
                    self.config = avcc_config(&annex_b_nals(&header)?)?;
                }
                let pts = native(sample.GetSampleTime(), "H.264 timestamp is unavailable")?;
                if pts < 0 {
                    return Err("Invalid H.264 timestamp".into());
                }
                self.output
                    .send_video(pts as u64 / 10, key, avcc_frame(&nals)?, &self.config);
            }
        }
        Err("H.264 encoder exceeded its output limit".into())
    }

    fn stop(&mut self) {
        if !self.stopped {
            self.stopped = true;
            unsafe {
                for message in [MFT_MESSAGE_COMMAND_FLUSH, MFT_MESSAGE_NOTIFY_END_STREAMING] {
                    if self.transform.ProcessMessage(message, 0).is_err() {
                        self.output.fail("Unable to stop Windows H.264 encoder");
                    }
                }
            }
        }
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Screen {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    item: GraphicsCaptureItem,
    arrived: Option<EventRegistrationToken>,
    closed: Option<EventRegistrationToken>,
    frame_ready: Arc<AtomicBool>,
    context: ID3D11DeviceContext,
    staging: ID3D11Texture2D,
    width: u32,
    height: u32,
    output: Arc<Output>,
}

impl Screen {
    fn new(id: u32, output: Arc<Output>) -> Result<Self, String> {
        let (_, monitor) = monitors()?
            .into_iter()
            .find(|(display, _)| display.id == id)
            .ok_or("The selected display is no longer available")?;
        let interop: IGraphicsCaptureItemInterop = native(
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>(),
            "Windows Graphics Capture is unavailable",
        )?;
        let item: GraphicsCaptureItem = native(
            unsafe { interop.CreateForMonitor(monitor) },
            "Unable to access the selected display",
        )?;
        let size = native(item.Size(), "Unable to read the selected display size")?;
        if size.Width < 2 || size.Height < 2 || size.Width > 32768 || size.Height > 32768 {
            return Err("Invalid Windows capture dimensions".into());
        }
        let (mut device, mut context) = (None, None);
        unsafe {
            native(
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                ),
                "Direct3D 11 screen capture is unavailable",
            )?;
        }
        let device = device.ok_or("Direct3D 11 device is unavailable")?;
        let context = context.ok_or("Direct3D 11 context is unavailable")?;
        let dxgi: IDXGIDevice =
            native(device.cast(), "Unable to configure capture graphics device")?;
        let winrt: IDirect3DDevice = native(
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }.and_then(|device| device.cast()),
            "Unable to configure Windows Graphics Capture",
        )?;
        // Keep this factory scoped as well: generated static helpers cache it
        // past the final RoUninitialize when a capture worker is restarted.
        let pool_factory = native(
            factory::<Direct3D11CaptureFramePool, IDirect3D11CaptureFramePoolStatics2>(),
            "Windows screen frame pool is unavailable",
        )?;
        let mut raw_pool = ptr::null_mut();
        native(
            unsafe {
                (pool_factory.vtable().CreateFreeThreaded)(
                    pool_factory.as_raw(),
                    winrt.as_raw(),
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    2,
                    size,
                    &mut raw_pool,
                )
                .ok()
            },
            "Unable to create Windows screen frame pool",
        )?;
        if raw_pool.is_null() {
            return Err("Windows screen frame pool is unavailable".into());
        }
        // SAFETY: the successful factory call transferred one non-null COM reference.
        let pool = unsafe { Direct3D11CaptureFramePool::from_raw(raw_pool) };
        let session = match pool.CreateCaptureSession(&item) {
            Ok(session) => session,
            Err(_) => {
                let _ = pool.Close();
                return Err("Unable to create screen capture session".into());
            }
        };
        let desc = D3D11_TEXTURE2D_DESC {
            Width: size.Width as u32,
            Height: size.Height as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..Default::default()
        };
        let mut staging = None;
        if unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }.is_err() {
            let _ = session.Close();
            let _ = pool.Close();
            return Err("Unable to allocate screen readback texture".into());
        }
        let mut screen = Self {
            pool,
            session,
            item,
            arrived: None,
            closed: None,
            frame_ready: Arc::new(AtomicBool::new(false)),
            context,
            staging: staging.ok_or("Screen texture is unavailable")?,
            width: desc.Width,
            height: desc.Height,
            output,
        };
        let frame_ready = screen.frame_ready.clone();
        screen.arrived = Some(native(
            screen
                .pool
                .FrameArrived(&TypedEventHandler::new(move |_, _| {
                    frame_ready.store(true, Ordering::Release);
                    Ok(())
                })),
            "Unable to receive Windows screen frames",
        )?);
        let output = screen.output.clone();
        screen.closed = Some(native(
            screen.item.Closed(&TypedEventHandler::new(move |_, _| {
                output.fail("The captured display is no longer available");
                Ok(())
            })),
            "Unable to monitor captured display",
        )?);
        native(
            screen.session.StartCapture(),
            "Unable to start Windows screen capture",
        )?;
        Ok(screen)
    }

    fn read(&self, frame: &Direct3D11CaptureFrame) -> Result<(Vec<u8>, u64), String> {
        let size = native(frame.ContentSize(), "Unable to read captured display size")?;
        if size.Width != self.width as i32 || size.Height != self.height as i32 {
            return Err("Display size changed; start screen mirroring again".into());
        }
        let pts = native(
            frame.SystemRelativeTime(),
            "Unable to read screen timestamp",
        )?
        .Duration;
        if pts < 0 {
            return Err("Invalid screen timestamp".into());
        }
        let surface = native(frame.Surface(), "Unable to read screen surface")?;
        let access: IDirect3DDxgiInterfaceAccess =
            native(surface.cast(), "Unable to access screen graphics buffer")?;
        let texture: ID3D11Texture2D = native(
            unsafe { access.GetInterface() },
            "Unable to read screen graphics texture",
        )?;
        unsafe {
            self.context.CopyResource(&self.staging, &texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            native(
                self.context
                    .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)),
                "Unable to map captured screen",
            )?;
            let result = if mapped.pData.is_null() || mapped.RowPitch < self.width * 4 {
                Err("Invalid captured screen layout".into())
            } else {
                let bytes = std::slice::from_raw_parts(
                    mapped.pData.cast::<u8>(),
                    mapped.RowPitch as usize * self.height as usize,
                );
                bgra_to_nv12(
                    bytes,
                    self.width,
                    self.height,
                    mapped.RowPitch as usize,
                    self.output.profile,
                )
            };
            self.context.Unmap(&self.staging, 0);
            Ok((result?, (pts as u64).saturating_sub(self.output.epoch) / 10))
        }
    }

    fn next_frame(&self) -> Result<Option<Direct3D11CaptureFrame>, String> {
        let mut latest = None;
        // Pool depth is two. Coalesced notifications must not leave a stale final
        // frame queued, and a busy display must not make this drain unbounded.
        for _ in 0..2 {
            match self.pool.TryGetNextFrame() {
                Ok(frame) => {
                    if let Some(previous) = latest.replace(frame) {
                        if previous.Close().is_err() {
                            if let Some(frame) = latest.take() {
                                let _ = frame.Close();
                            }
                            return Err("Unable to release captured screen frame".into());
                        }
                    }
                }
                // A null WinRT object is projected by windows-rs as E_POINTER.
                Err(error) if error.code() == E_BOUNDS || error.code() == E_POINTER => break,
                Err(_) => {
                    if let Some(frame) = latest.take() {
                        let _ = frame.Close();
                    }
                    return Err("Windows screen capture stopped unexpectedly".into());
                }
            }
        }
        Ok(latest)
    }
}
impl Drop for Screen {
    fn drop(&mut self) {
        if let Some(token) = self.arrived.take() {
            if self.pool.RemoveFrameArrived(token).is_err() {
                self.output.fail("Unable to remove Windows screen callback");
            }
        }
        if let Some(token) = self.closed.take() {
            if self.item.RemoveClosed(token).is_err() {
                self.output
                    .fail("Unable to remove Windows display callback");
            }
        }
        if self.session.Close().is_err() {
            self.output.fail("Unable to stop Windows screen capture");
        }
        if self.pool.Close().is_err() {
            self.output
                .fail("Unable to close Windows screen frame pool");
        }
    }
}

fn video_worker(id: u32, output: Arc<Output>, ready: mpsc::SyncSender<Result<(), String>>) {
    let result: Result<(), String> = (|| {
        let _apartment = Apartment::new()?;
        let _media = MediaFoundation::new()?;
        let mut encoder = Encoder::new(output.clone())?;
        let screen = Screen::new(id, output.clone())?;
        let _ = ready.send(Ok(()));
        let mut previous = None;
        let result: Result<(), String> = (|| {
            while output.active() {
                if !screen.frame_ready.swap(false, Ordering::AcqRel) {
                    thread::sleep(Duration::from_millis(3));
                    continue;
                }
                if let Some(frame) = screen.next_frame()? {
                    let result = (|| {
                        let pts =
                            native(frame.SystemRelativeTime(), "Unable to read screen clock")?
                                .Duration;
                        if previous
                            .is_some_and(|last| pts - last < 10_000_000 / output.profile.fps as i64)
                        {
                            return Ok(());
                        }
                        previous = Some(pts);
                        output.attempts.fetch_add(1, Ordering::Relaxed);
                        if output.sender.capacity() == 0 {
                            output.drops.fetch_add(1, Ordering::Relaxed);
                            output.need_keyframe.store(true, Ordering::SeqCst);
                            return Ok(());
                        }
                        let (data, pts) = screen.read(&frame)?;
                        encoder.encode(&data, pts)
                    })();
                    let close = native(frame.Close(), "Unable to release captured screen frame");
                    result?;
                    close?;
                }
            }
            Ok(())
        })();
        if let Err(message) = &result {
            output.fail(message);
        }
        encoder.stop();
        result
    })();
    if let Err(message) = result {
        output.fail(&message);
        let _ = ready.send(Err(message));
    }
}

struct Audio {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    output: Arc<Output>,
}
impl Audio {
    fn new(output: Arc<Output>) -> Result<Self, String> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = native(
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER),
                "Windows system audio is unavailable",
            )?;
            let device = native(
                enumerator.GetDefaultAudioEndpoint(eRender, eConsole),
                "No default system audio output is available",
            )?;
            let client: IAudioClient = native(
                device.Activate(CLSCTX_INPROC_SERVER, None),
                "Unable to open system audio",
            )?;
            let format = WAVEFORMATEX {
                wFormatTag: 3,
                nChannels: 2,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 48_000 * 8,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            native(
                client.Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK
                        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                    1_000_000,
                    0,
                    &format,
                    None,
                ),
                "Unable to capture 48 kHz stereo system audio",
            )?;
            let capture = native(client.GetService(), "Unable to read system audio stream")?;
            let audio = Self {
                client,
                capture,
                output,
            };
            native(audio.client.Start(), "Unable to start system audio capture")?;
            Ok(audio)
        }
    }

    fn read(&self) -> Result<Option<(Vec<u8>, u64)>, String> {
        unsafe {
            if native(
                self.capture.GetNextPacketSize(),
                "System audio endpoint stopped unexpectedly",
            )? == 0
            {
                return Ok(None);
            }
            let (mut bytes, mut frames, mut flags, mut clock) = (ptr::null_mut(), 0, 0, 0);
            native(
                self.capture
                    .GetBuffer(&mut bytes, &mut frames, &mut flags, None, Some(&mut clock)),
                "Unable to read system audio packet",
            )?;
            let result = if frames > 48_000
                || (bytes.is_null() && flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 == 0)
            {
                Err("Invalid system audio packet".into())
            } else if flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0 {
                Ok(None)
            } else {
                let data = if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    vec![0; frames as usize * 8]
                } else {
                    std::slice::from_raw_parts(bytes, frames as usize * 8).to_vec()
                };
                Ok(Some((data, clock.saturating_sub(self.output.epoch) / 10)))
            };
            native(
                self.capture.ReleaseBuffer(frames),
                "Unable to release system audio packet",
            )?;
            result
        }
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        if unsafe { self.client.Stop() }.is_err() {
            self.output.fail("Unable to stop system audio capture");
        }
    }
}

fn audio_worker(output: Arc<Output>, ready: mpsc::SyncSender<Result<(), String>>) {
    let result: Result<(), String> = (|| {
        let _apartment = Apartment::new()?;
        let audio = Audio::new(output.clone())?;
        let _ = ready.send(Ok(()));
        let mut configured = false;
        while output.active() {
            if let Some((data, pts)) = audio.read()? {
                if !output.active() {
                    break;
                }
                let packet = output.packet("audio", "pcm-f32le", pts, false, data);
                if configured {
                    let _ = output.sender.try_send(packet);
                } else if let Ok(mut permits) = output.sender.try_reserve_many(2) {
                    permits.next().unwrap().send(output.packet(
                        "audio_config",
                        "pcm-f32le",
                        pts,
                        false,
                        AUDIO_CONFIG.to_vec(),
                    ));
                    permits.next().unwrap().send(packet);
                    configured = true;
                }
            } else {
                thread::sleep(Duration::from_millis(3));
            }
        }
        Ok(())
    })();
    if let Err(message) = result {
        output.fail(&message);
        let _ = ready.send(Err(message));
    }
}

pub struct Capture {
    output: Arc<Output>,
    workers: Vec<JoinHandle<()>>,
}
impl Capture {
    pub fn start(
        display_id: u32,
        audio: bool,
        quality: &str,
        sender: Sender<MediaPacket>,
        _allow_permission_request: bool,
        max_edge: Option<u32>,
        failure: Arc<Failure>,
    ) -> Result<Self, String> {
        if let Some(error) = failure.message() {
            return Err(error);
        }
        if sender.max_capacity() < 2 {
            return Err("Mirror output queue needs capacity for configuration and video".into());
        }
        let display = displays()?
            .into_iter()
            .find(|display| display.id == display_id)
            .ok_or("The selected display is no longer available")?;
        let profile = profile(display.width, display.height, quality, max_edge)?;
        if !permission_granted() {
            return Err("Windows Graphics Capture is unavailable on this system".into());
        }
        let output = Arc::new(Output {
            sender,
            runtime: tokio::runtime::Handle::try_current()
                .map_err(|_| "Screen capture must start in the desktop runtime")?,
            running: AtomicBool::new(true),
            need_keyframe: AtomicBool::new(true),
            attempts: AtomicU64::new(0),
            drops: AtomicU64::new(0),
            failure,
            profile,
            epoch: qpc_time()?,
        });
        let mut capture = Self {
            output,
            workers: Vec::new(),
        };
        for kind in 0..if audio { 2 } else { 1 } {
            let (ready, started) = mpsc::sync_channel(1);
            let output = capture.output.clone();
            let worker = thread::Builder::new()
                .name(
                    if kind == 0 {
                        "ymlink-screen"
                    } else {
                        "ymlink-audio"
                    }
                    .into(),
                )
                .spawn(move || {
                    if kind == 0 {
                        video_worker(display_id, output, ready);
                    } else {
                        audio_worker(output, ready);
                    }
                })
                .map_err(|_| "Unable to start Windows media worker")?;
            capture.workers.push(worker);
            started
                .recv()
                .map_err(|_| "Windows media worker stopped during startup")??;
        }
        if let Some(error) = capture.output.failure.message() {
            return Err(error);
        }
        Ok(capture)
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.output.profile.width, self.output.profile.height)
    }
    pub fn take_video_pressure(&self) -> (u64, u64) {
        (
            self.output.attempts.swap(0, Ordering::Relaxed),
            self.output.drops.swap(0, Ordering::Relaxed),
        )
    }
    pub fn stop(&mut self) {
        {
            let _failure = self
                .output
                .failure
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            self.output.running.store(false, Ordering::SeqCst);
        }
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                self.output
                    .fail("Windows media worker stopped unexpectedly");
            }
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only Main-profile SPS reader: ITU-T H.264 7.3.2.1 and E.1.1.
    // Inspect the encoded bitstream, rather than trusting the requested MF type.
    fn sps_color_description(sps: &[u8]) -> Option<(bool, u32, u32, u32)> {
        struct Bits {
            data: Vec<u8>,
            position: usize,
        }
        impl Bits {
            fn read(&mut self, count: usize) -> u32 {
                assert!(count <= 32 && self.position + count <= self.data.len() * 8);
                let mut value = 0;
                for _ in 0..count {
                    value = (value << 1)
                        | u32::from((self.data[self.position / 8] >> (7 - self.position % 8)) & 1);
                    self.position += 1;
                }
                value
            }
            fn flag(&mut self) -> bool {
                self.read(1) != 0
            }
            fn ue(&mut self) -> u32 {
                let mut zeros = 0;
                while !self.flag() {
                    zeros += 1;
                    assert!(zeros < 32, "invalid Exp-Golomb code in encoder SPS");
                }
                (1u32 << zeros) - 1 + self.read(zeros)
            }
        }
        assert_eq!(sps[0] & 31, 7);
        let (mut rbsp, mut zeros) = (Vec::new(), 0);
        for &byte in &sps[1..] {
            if zeros == 2 && byte == 3 {
                zeros = 0;
                continue;
            }
            rbsp.push(byte);
            zeros = if byte == 0 { zeros + 1 } else { 0 };
        }
        let mut bits = Bits {
            data: rbsp,
            position: 0,
        };
        assert_eq!(
            bits.read(8),
            77,
            "test parser expects requested H.264 Main profile"
        );
        bits.read(16); // constraints and level_idc
        bits.ue(); // seq_parameter_set_id
        bits.ue(); // log2_max_frame_num_minus4
        match bits.ue() {
            // pic_order_cnt_type
            0 => {
                bits.ue();
            }
            1 => {
                bits.flag();
                bits.ue(); // signed offsets use the same Exp-Golomb bit lengths
                bits.ue();
                for _ in 0..bits.ue() {
                    bits.ue();
                }
            }
            2 => {}
            _ => panic!("invalid pic_order_cnt_type"),
        }
        bits.ue(); // max_num_ref_frames
        bits.flag(); // gaps_in_frame_num_value_allowed_flag
        bits.ue(); // pic_width_in_mbs_minus1
        bits.ue(); // pic_height_in_map_units_minus1
        if !bits.flag() {
            bits.flag();
        } // frame_mbs_only / mb_adaptive_frame_field
        bits.flag(); // direct_8x8_inference_flag
        if bits.flag() {
            for _ in 0..4 {
                bits.ue();
            }
        } // frame crop offsets
        if !bits.flag() {
            return None;
        } // vui_parameters_present_flag
        if bits.flag() && bits.read(8) == 255 {
            bits.read(32);
        } // aspect ratio
        if bits.flag() {
            bits.flag();
        } // overscan information
        if !bits.flag() {
            return None;
        } // video_signal_type_present_flag
        bits.read(3); // video_format
        let full_range = bits.flag();
        if !bits.flag() {
            return None;
        } // colour_description_present_flag
        Some((full_range, bits.read(8), bits.read(8), bits.read(8)))
    }

    fn main_sps(color: Option<(bool, u32)>) -> Vec<u8> {
        fn ue(bits: &mut Vec<bool>, value: u32) {
            let value = value + 1;
            let length = (32 - value.leading_zeros()) as usize;
            bits.extend(vec![false; length - 1]);
            append_bits(bits, value, length);
        }
        let mut bits = Vec::new();
        for byte in [77, 0, 31] {
            append_bits(&mut bits, byte, 8);
        }
        for value in [0, 0, 0, 0, 1] {
            ue(&mut bits, value);
        }
        bits.push(false); // gaps
        ue(&mut bits, 39);
        ue(&mut bits, 29); // 640 x 480
        bits.extend([true, true, false, color.is_some()]); // frame-only, inference, crop, VUI
        if let Some((full_range, matrix)) = color {
            bits.extend([false, false, true]); // no aspect/overscan, video signal present
            append_bits(&mut bits, 5, 3);
            bits.extend([full_range, true]);
            for value in [6, 1, matrix] {
                append_bits(&mut bits, value, 8);
            }
            bits.extend([false; 6]); // remaining VUI flags
        }
        bits.push(true); // rbsp_stop_one_bit
        while bits.len() % 8 != 0 {
            bits.push(false);
        }
        let mut result = vec![0x67];
        result.extend(bits.chunks_exact(8).map(|chunk| {
            chunk
                .iter()
                .fold(0u8, |value, &bit| (value << 1) | u8::from(bit))
        }));
        result
    }

    #[test]
    fn sps_color_normalization_preserves_valid_syntax_and_rejects_truncation() {
        // Actual native encoder SPS: aspect ratio, timing, bitstream restrictions,
        // and emulation-prevention bytes, with no video color description.
        let timed = [
            0x67, 0x4d, 0x40, 0x1f, 0x95, 0xa0, 0x14, 0x01, 0x6e, 0xc0, 0x44, 0, 0, 3, 0, 4, 0, 0,
            3, 0, 0xf0, 0x36, 0x82, 0x21, 0x1a, 0x80,
        ];
        for source in [main_sps(None), main_sps(Some((true, 1))), timed.to_vec()] {
            let result = normalize_sps(&source).unwrap();
            assert_eq!(sps_color_description(&result), Some((false, 2, 2, 6)));
            assert_eq!(
                normalize_sps(&result).unwrap(),
                result,
                "normalization is idempotent"
            );
            assert!(!result[1..]
                .windows(3)
                .any(|bytes| bytes[..2] == [0, 0] && bytes[2] < 3));
            for length in 0..source.len() {
                assert!(
                    normalize_sps(&source[..length]).is_err(),
                    "accepted truncated SPS at {length}"
                );
            }
        }
        let mut malformed_escape = timed.to_vec();
        malformed_escape[14] = 4;
        assert!(normalize_sps(&malformed_escape).is_err());
        let mut missing_escape = timed.to_vec();
        missing_escape.remove(13);
        assert!(normalize_sps(&missing_escape).is_err());
        let mut unsupported = main_sps(None);
        unsupported[1] = 100;
        assert!(normalize_sps(&unsupported).is_err());
        assert!(avcc_frame(&[&unsupported]).is_err());
    }

    #[test]
    fn capability_query_survives_repeated_worker_apartments() {
        // Capability queries never create a capture item or read pixels/audio.
        for _ in 0..5 {
            std::thread::spawn(permission_granted).join().unwrap();
        }
    }

    fn output(capacity: usize) -> (Arc<Output>, tokio::sync::mpsc::Receiver<MediaPacket>) {
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        (
            Arc::new(Output {
                sender,
                runtime: tokio::runtime::Handle::current(),
                running: AtomicBool::new(true),
                need_keyframe: AtomicBool::new(true),
                attempts: AtomicU64::new(0),
                drops: AtomicU64::new(0),
                failure: Arc::new(Failure::default()),
                profile: profile(64, 64, "smooth", None).unwrap(),
                epoch: 0,
            }),
            receiver,
        )
    }

    #[test]
    fn quality_scaling_and_nv12_conversion_are_bounded() {
        let p = profile(3456, 2234, "balanced", Some(960)).unwrap();
        assert_eq!(
            (p.width, p.height, p.bitrate, p.fps),
            (960, 620, 3_000_000, 60)
        );
        assert!(profile(100, 100, "bad", None).is_err());
        assert!(profile(100, 100, "quality", Some(1)).is_err());
        let p = profile(2, 2, "smooth", None).unwrap();
        assert_eq!(
            bgra_to_nv12(&[0; 16], 2, 2, 8, p).unwrap(),
            [16, 16, 16, 16, 128, 128]
        );
        assert_eq!(
            bgra_to_nv12(&[255; 24], 2, 2, 12, p).unwrap(),
            [235, 235, 235, 235, 128, 128]
        );
        assert!(bgra_to_nv12(&[0; 15], 2, 2, 8, p).is_err());
        assert_eq!(bgra_to_nv12(&[0; 64], 4, 4, 16, p).unwrap().len(), 6);
        let red = [0, 0, 255, 255].repeat(4);
        assert_eq!(
            bgra_to_nv12(&red, 2, 2, 8, p).unwrap(),
            [82, 82, 82, 82, 90, 240]
        );
    }

    #[test]
    fn annex_b_becomes_four_byte_avcc_and_decoder_configuration() {
        let sps = main_sps(None);
        let mut data = vec![0, 0, 0, 1];
        data.extend_from_slice(&sps);
        data.extend_from_slice(&[0, 0, 1, 0x68, 0xee, 0, 0, 0, 1, 0x65, 0x88]);
        let nals = annex_b_nals(&data).unwrap();
        assert_eq!(nals.len(), 3);
        let config = avcc_config(&nals).unwrap();
        let length = u16::from_be_bytes([config[6], config[7]]) as usize;
        assert_eq!(&config[..6], &[1, 0x4d, 0, 31, 255, 225]);
        assert_eq!(
            sps_color_description(&config[8..8 + length]),
            Some((false, 2, 2, 6))
        );
        let frame = avcc_frame(&nals).unwrap();
        assert_eq!(
            u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize,
            length
        );
        assert_eq!(&frame[4..4 + length], &config[8..8 + length]);
        for invalid in [&[][..], &[0, 0, 1][..], &[1, 2, 3][..], &[0, 0, 1, 0][..]] {
            assert!(annex_b_nals(invalid).is_err());
        }
        assert!(avcc_config(&[&[0x67, 0x4d, 0, 31]]).is_err());
        assert!(avcc_config(&[&[]]).is_err());
    }

    #[tokio::test]
    async fn backpressure_requires_new_keyframe_and_fatal_signal_survives_full_queue() {
        let (output, mut receiver) = output(2);
        output.send_video(0, true, vec![1], &[1]);
        output.send_video(1, false, vec![2], &[]);
        assert_eq!(output.drops.load(Ordering::Relaxed), 1);
        assert!(output.need_keyframe.load(Ordering::SeqCst));
        assert_eq!(receiver.recv().await.unwrap().kind, "video_config");
        assert_eq!(receiver.recv().await.unwrap().kind, "video");
        output.send_video(2, false, vec![3], &[]);
        assert!(receiver.try_recv().is_err());
        output.send_video(3, true, vec![4], &[1]);
        output.fail("first error");
        output.fail("second error");
        assert_eq!(output.failure.message().as_deref(), Some("first error"));
        assert!(!output.active());
        output.running.store(false, Ordering::SeqCst);
    }

    #[tokio::test]
    #[ignore = "requires Windows Media Foundation; synthetic pixels only, no screen or audio capture"]
    async fn native_encoder_emits_avcc_keyframe_without_screen_capture() {
        let _apartment = Apartment::new().unwrap();
        let _media = MediaFoundation::new().unwrap();
        let (mut output, mut receiver) = output(8);
        Arc::get_mut(&mut output).unwrap().profile = profile(1280, 720, "smooth", None).unwrap();
        let mut encoder = Encoder::new(output.clone()).unwrap();
        let data = bgra_to_nv12(
            &vec![0; 1280 * 720 * 4],
            1280,
            720,
            1280 * 4,
            output.profile,
        )
        .unwrap();
        for frame in 0..5 {
            encoder.encode(&data, frame * 33_333).unwrap();
            if receiver.len() >= 2 {
                break;
            }
        }
        let config = receiver
            .try_recv()
            .expect("encoder must produce low-latency output");
        assert_eq!(config.kind, "video_config");
        assert_eq!((config.data[0], config.data[4] & 3), (1, 3));
        assert_eq!(
            sps_color_description(
                &config.data[8..8 + u16::from_be_bytes([config.data[6], config.data[7]]) as usize]
            ),
            Some((false, 2, 2, 6)),
            "SPS must describe the actual BT.601 limited-range NV12 conversion"
        );
        let frame = receiver.try_recv().unwrap();
        assert_eq!(frame.kind, "video");
        assert!(frame.keyframe);
        let length = u32::from_be_bytes(frame.data[..4].try_into().unwrap()) as usize;
        assert!(length > 0 && length + 4 <= frame.data.len());
        let mut nals = &frame.data[..];
        while !nals.is_empty() {
            let length = u32::from_be_bytes(nals[..4].try_into().unwrap()) as usize;
            let nal = &nals[4..4 + length];
            if nal[0] & 31 == 7 {
                assert_eq!(sps_color_description(nal), Some((false, 2, 2, 6)));
            }
            nals = &nals[4 + length..];
        }
        output.running.store(false, Ordering::SeqCst);
        encoder.stop();
        assert!(output.failure.message().is_none());
    }

    #[tokio::test]
    #[ignore = "records the selected desktop and system audio; requires explicit YM_LINK_CAPTURE_TEST=1 consent"]
    async fn real_display_and_system_audio_capture_stop_cleanly() {
        assert_eq!(
            std::env::var("YM_LINK_CAPTURE_TEST").as_deref(),
            Ok("1"),
            "Explicit consent is required before this test captures screen and system audio"
        );
        let display = displays()
            .unwrap()
            .into_iter()
            .next()
            .expect("an attached display");
        let failure = Arc::new(Failure::default());
        let (sender, mut receiver) = tokio::sync::mpsc::channel(16);
        let mut capture = Capture::start(
            display.id,
            true,
            "smooth",
            sender,
            true,
            Some(640),
            failure.clone(),
        )
        .unwrap();
        let until = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut video = false;
        while tokio::time::Instant::now() < until {
            match tokio::time::timeout(Duration::from_millis(200), receiver.recv()).await {
                Ok(Some(packet)) => {
                    assert_ne!(
                        packet.kind,
                        "error",
                        "{}",
                        String::from_utf8_lossy(&packet.data)
                    );
                    if packet.kind == "video" {
                        video = true;
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => {}
            }
        }
        capture.stop();
        capture.stop();
        assert!(
            video,
            "Windows Graphics Capture must emit an encoded display frame"
        );
        assert!(failure.message().is_none(), "{:?}", failure.message());
        // WASAPI may emit no packets when the endpoint is idle. A real listening
        // test and Quest synchronization remain separate device acceptance work.
    }
}
