//! macOS capture and hardware H.264 encoding. No media or user paths are logged.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Display {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct MediaPacket {
    pub kind: String,
    pub codec: String,
    pub pts_us: u64,
    pub keyframe: bool,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use screencapturekit::prelude::*;
    use screencapturekit::stream::delegate_trait::SCStreamDelegateTrait;
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    type Ref = *const c_void;
    type SessionRef = *mut c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
        fn CGDisplayPixelsWide(display: u32) -> usize;
        fn CGDisplayPixelsHigh(display: u32) -> usize;
        fn CGDisplayIsBuiltin(display: u32) -> u32;
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: Ref;
        static kCFBooleanFalse: Ref;
        fn CFRelease(value: Ref);
        fn CFNumberCreate(allocator: Ref, kind: isize, value: *const c_void) -> Ref;
        fn CFDictionaryCreate(
            allocator: Ref,
            keys: *const Ref,
            values: *const Ref,
            count: isize,
            key_callbacks: Ref,
            value_callbacks: Ref,
        ) -> Ref;
    }

    #[link(name = "VideoToolbox", kind = "framework")]
    extern "C" {
        static kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder: Ref;
        static kVTCompressionPropertyKey_RealTime: Ref;
        static kVTCompressionPropertyKey_AllowFrameReordering: Ref;
        static kVTCompressionPropertyKey_AverageBitRate: Ref;
        static kVTCompressionPropertyKey_ExpectedFrameRate: Ref;
        static kVTCompressionPropertyKey_MaxKeyFrameInterval: Ref;
        static kVTCompressionPropertyKey_ProfileLevel: Ref;
        static kVTProfileLevel_H264_Main_AutoLevel: Ref;
        static kVTEncodeFrameOptionKey_ForceKeyFrame: Ref;
        fn VTCompressionSessionCreate(
            allocator: Ref,
            width: i32,
            height: i32,
            codec: u32,
            specification: Ref,
            source_attributes: Ref,
            compressed_allocator: Ref,
            callback: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, i32, u32, *mut c_void)>,
            context: *mut c_void,
            out: *mut SessionRef,
        ) -> i32;
        fn VTSessionSetProperty(session: SessionRef, key: Ref, value: Ref) -> i32;
        fn VTCompressionSessionPrepareToEncodeFrames(session: SessionRef) -> i32;
        fn VTCompressionSessionEncodeFrame(
            session: SessionRef,
            image: *mut c_void,
            pts: CMTime,
            duration: CMTime,
            properties: Ref,
            frame_context: *mut c_void,
            flags: *mut u32,
        ) -> i32;
        fn VTCompressionSessionCompleteFrames(session: SessionRef, until: CMTime) -> i32;
        fn VTCompressionSessionInvalidate(session: SessionRef);
    }

    #[link(name = "CoreMedia", kind = "framework")]
    extern "C" {
        fn CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
            format: Ref,
            index: usize,
            data: *mut *const u8,
            size: *mut usize,
            count: *mut usize,
            header_size: *mut i32,
        ) -> i32;
        fn CMBlockBufferGetDataLength(block: Ref) -> usize;
        fn CMBlockBufferCopyDataBytes(
            block: Ref,
            offset: usize,
            size: usize,
            destination: *mut c_void,
        ) -> i32;
    }

    struct Cf(Ref);
    impl Drop for Cf {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: Cf owns the +1 reference returned by a Create function.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn number(value: i32) -> Result<Cf, String> {
        // kCFNumberSInt32Type = 3; CFNumberCreate copies the stack integer.
        let number = unsafe { CFNumberCreate(ptr::null(), 3, (&value as *const i32).cast()) };
        if number.is_null() {
            Err("Unable to configure the hardware encoder".into())
        } else {
            Ok(Cf(number))
        }
    }

    fn dictionary(entries: &[(Ref, Ref)]) -> Result<Cf, String> {
        let keys: Vec<_> = entries.iter().map(|entry| entry.0).collect();
        let values: Vec<_> = entries.iter().map(|entry| entry.1).collect();
        // Entries are borrowed until this dictionary is released; keys are static CFStrings.
        let dictionary = unsafe {
            CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                entries.len() as isize,
                ptr::null(),
                ptr::null(),
            )
        };
        if dictionary.is_null() {
            Err("Unable to configure the hardware encoder".into())
        } else {
            Ok(Cf(dictionary))
        }
    }

    pub fn displays() -> Result<Vec<Display>, String> {
        let mut ids = [0u32; 64];
        let mut count = 0;
        // CoreGraphics display enumeration does not request screen recording access.
        if unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) } != 0 {
            return Err("Unable to list displays".into());
        }
        Ok(ids[..(count as usize).min(ids.len())]
            .iter()
            .map(|&id| Display {
                id,
                name: if unsafe { CGDisplayIsBuiltin(id) } != 0 {
                    "Built-in display".into()
                } else {
                    format!("Display {id}")
                },
                width: unsafe { CGDisplayPixelsWide(id) } as u32,
                height: unsafe { CGDisplayPixelsHigh(id) } as u32,
            })
            .collect())
    }

    pub fn permission_granted() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    #[derive(Debug, Clone, Copy)]
    struct Profile {
        width: u32,
        height: u32,
        fps: i32,
        bitrate: i32,
    }

    fn profile(width: u32, height: u32, quality: &str) -> Result<Profile, String> {
        let (maximum, fps, bitrate) = match quality {
            "smooth" => (1280u32, 30, 4_000_000),
            "balanced" => (1920, 60, 12_000_000),
            "quality" => (2560, 60, 24_000_000),
            _ => return Err("Unknown mirror quality".into()),
        };
        if width < 2 || height < 2 {
            return Err("Display dimensions are invalid".into());
        }
        let longest = width.max(height);
        let scaled = |value: u32| {
            let value = if longest > maximum {
                (u64::from(value) * u64::from(maximum) / u64::from(longest)) as u32
            } else {
                value
            };
            (value & !1).max(2)
        };
        Ok(Profile {
            width: scaled(width),
            height: scaled(height),
            fps,
            bitrate,
        })
    }

    fn timestamp(time: CMTime) -> Option<u64> {
        if !time.is_numeric() || time.value < 0 || time.timescale <= 0 {
            return None;
        }
        u64::try_from(time.value as u128 * 1_000_000 / time.timescale as u128).ok()
    }

    struct Output {
        sender: Sender<MediaPacket>,
        runtime: tokio::runtime::Handle,
        running: AtomicBool,
        failed: AtomicBool,
        need_keyframe: AtomicBool,
        audio_config_sent: AtomicBool,
        width: u32,
        height: u32,
    }

    impl Output {
        fn packet(
            &self,
            kind: &str,
            codec: &str,
            pts_us: u64,
            keyframe: bool,
            data: Vec<u8>,
        ) -> MediaPacket {
            MediaPacket {
                kind: kind.into(),
                codec: codec.into(),
                pts_us,
                keyframe,
                width: if kind.starts_with("video") {
                    self.width
                } else {
                    0
                },
                height: if kind.starts_with("video") {
                    self.height
                } else {
                    0
                },
                data,
            }
        }

        fn fail(&self, message: &str) {
            if self.failed.swap(true, Ordering::SeqCst) || !self.running.load(Ordering::SeqCst) {
                return;
            }
            // Never block Apple's output queue or silently lose a fatal error to backpressure.
            let packet = self.packet("error", "", 0, false, message.as_bytes().to_vec());
            let sender = self.sender.clone();
            self.runtime.spawn(async move {
                let _ = sender.send(packet).await;
            });
        }

        fn send_video(&self, sample: &CMSampleBuffer) -> Result<(), String> {
            let Some(pts_us) = timestamp(sample.presentation_timestamp()) else {
                return Ok(());
            };
            let keyframe = sample.is_sync_sample();
            if self.need_keyframe.load(Ordering::SeqCst) && !keyframe {
                return Ok(());
            }
            let block = sample
                .data_buffer()
                .ok_or("Hardware encoder returned no video data")?;
            let length = unsafe { CMBlockBufferGetDataLength(block.as_ptr().cast()) };
            if length == 0 || length > 8 * 1024 * 1024 {
                return Err("Hardware encoder returned an invalid frame size".into());
            }
            let mut data = vec![0u8; length];
            if unsafe {
                CMBlockBufferCopyDataBytes(
                    block.as_ptr().cast(),
                    0,
                    length,
                    data.as_mut_ptr().cast(),
                )
            } != 0
            {
                return Err("Unable to read the hardware encoded frame".into());
            }
            let frame = self.packet("video", "h264-avcc", pts_us, keyframe, data);
            if keyframe {
                let format = sample
                    .format_description()
                    .ok_or("Hardware encoder returned no video format")?;
                let config = avcc_config(format.as_ptr().cast())?;
                if let Ok(mut permits) = self.sender.try_reserve_many(2) {
                    permits.next().unwrap().send(self.packet(
                        "video_config",
                        "h264-avcc",
                        pts_us,
                        true,
                        config,
                    ));
                    permits.next().unwrap().send(frame);
                    self.need_keyframe.store(false, Ordering::SeqCst);
                } else {
                    self.need_keyframe.store(true, Ordering::SeqCst);
                }
            } else if self.sender.try_send(frame).is_err() {
                self.need_keyframe.store(true, Ordering::SeqCst);
            }
            Ok(())
        }

        fn send_audio(&self, sample: &CMSampleBuffer) -> Result<(), String> {
            let Some(pts_us) = timestamp(sample.presentation_timestamp()) else {
                return Ok(());
            };
            let format = sample
                .format_description()
                .ok_or("System audio format is unavailable")?;
            if !format.is_pcm()
                || !format.audio_is_float()
                || format.audio_is_big_endian()
                || format.audio_bits_per_channel() != Some(32)
                || format.audio_channel_count() != Some(2)
                || format.audio_sample_rate() != Some(48_000.0)
            {
                return Err(
                    "Unsupported system audio format; expected 48 kHz stereo Float32 PCM".into(),
                );
            }
            let frames = usize::try_from(sample.num_samples())
                .map_err(|_| "Invalid system audio sample count")?;
            if frames == 0 {
                return Ok(());
            }
            if frames > 48_000 {
                return Err("Invalid system audio sample count".into());
            }
            let list = sample
                .audio_buffer_list()
                .map_err(|_| "Unable to read system audio")?;
            let buffers: Vec<&[u8]> = list.iter().map(|buffer| buffer.data()).collect();
            let planar = format.audio_format_flags().unwrap_or(0) & (1 << 5) != 0;
            let data = interleave_audio(&buffers, frames, planar)?;
            let packet = self.packet("audio", "pcm-f32le", pts_us, false, data);
            if !self.audio_config_sent.load(Ordering::SeqCst) {
                if let Ok(mut permits) = self.sender.try_reserve_many(2) {
                    let config = br#"{"sample_rate":48000,"channels":2,"sample_format":"f32le","interleaved":true}"#.to_vec();
                    permits.next().unwrap().send(self.packet(
                        "audio_config",
                        "pcm-f32le",
                        pts_us,
                        false,
                        config,
                    ));
                    permits.next().unwrap().send(packet);
                    self.audio_config_sent.store(true, Ordering::SeqCst);
                }
            } else {
                let _ = self.sender.try_send(packet);
            }
            Ok(())
        }
    }

    fn interleave_audio(buffers: &[&[u8]], frames: usize, planar: bool) -> Result<Vec<u8>, String> {
        if planar && buffers.len() == 2 && buffers.iter().all(|buffer| buffer.len() == frames * 4) {
            let mut result = Vec::with_capacity(frames * 8);
            for frame in 0..frames {
                result.extend_from_slice(&buffers[0][frame * 4..frame * 4 + 4]);
                result.extend_from_slice(&buffers[1][frame * 4..frame * 4 + 4]);
            }
            Ok(result)
        } else if !planar && buffers.len() == 1 && buffers[0].len() == frames * 8 {
            Ok(buffers[0].to_vec())
        } else {
            Err("Unsupported system audio buffer layout".into())
        }
    }

    fn avcc_config(format: Ref) -> Result<Vec<u8>, String> {
        let mut sets = Vec::with_capacity(2);
        for index in 0..2 {
            let (mut data, mut size, mut count, mut header) = (ptr::null(), 0, 0, 0);
            let status = unsafe {
                CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                    format,
                    index,
                    &mut data,
                    &mut size,
                    &mut count,
                    &mut header,
                )
            };
            if status != 0
                || data.is_null()
                || size < 1
                || size > u16::MAX as usize
                || count != 2
                || header != 4
            {
                return Err("Hardware encoder returned unsupported H.264 configuration".into());
            }
            // SAFETY: parameter-set storage is borrowed from the live CMFormatDescription.
            sets.push(unsafe { std::slice::from_raw_parts(data, size) }.to_vec());
        }
        let (sps, pps) = (&sets[0], &sets[1]);
        if sps.len() < 4 {
            return Err("Hardware encoder returned an invalid H.264 SPS".into());
        }
        let mut data = vec![1, sps[1], sps[2], sps[3], 0xff, 0xe1];
        data.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        data.extend_from_slice(sps);
        data.push(1);
        data.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        data.extend_from_slice(pps);
        Ok(data)
    }

    unsafe extern "C" fn encoded(
        context: *mut c_void,
        _: *mut c_void,
        status: i32,
        flags: u32,
        sample: *mut c_void,
    ) {
        // SAFETY: Encoder owns the stable Box until CompleteFrames + Invalidate finish.
        let output = &*(context as *const Arc<Output>);
        if !output.running.load(Ordering::SeqCst) || output.failed.load(Ordering::SeqCst) {
            return;
        }
        if status != 0 {
            output.fail("The hardware video encoder stopped unexpectedly");
            return;
        }
        if flags & 2 != 0 || sample.is_null() {
            output.need_keyframe.store(true, Ordering::SeqCst);
            return;
        }
        if let Some(sample) = CMSampleBuffer::from_raw_borrowed(sample) {
            if let Err(error) = output.send_video(&sample) {
                output.fail(&error)
            }
        }
    }

    struct Encoder {
        session: SessionRef,
        context: Box<Arc<Output>>,
    }

    // VT sessions support arbitrary caller threads; a Mutex serializes encode and teardown.
    unsafe impl Send for Encoder {}

    impl Encoder {
        fn new(profile: Profile, output: Arc<Output>) -> Result<Self, String> {
            let mut encoder = Self {
                session: ptr::null_mut(),
                context: Box::new(output),
            };
            let specification = dictionary(&[unsafe {
                (
                    kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder,
                    kCFBooleanTrue,
                )
            }])?;
            let status = unsafe {
                VTCompressionSessionCreate(
                    ptr::null(),
                    profile.width as i32,
                    profile.height as i32,
                    u32::from_be_bytes(*b"avc1"),
                    specification.0,
                    ptr::null(),
                    ptr::null(),
                    Some(encoded),
                    (&mut *encoder.context as *mut Arc<Output>).cast(),
                    &mut encoder.session,
                )
            };
            if status != 0 || encoder.session.is_null() {
                return Err("Hardware H.264 encoding is unavailable on this Mac".into());
            }
            let bitrate = number(profile.bitrate)?;
            let fps = number(profile.fps)?;
            let key_interval = number(profile.fps * 2)?;
            let properties = unsafe {
                [
                    (kVTCompressionPropertyKey_RealTime, kCFBooleanTrue),
                    (
                        kVTCompressionPropertyKey_AllowFrameReordering,
                        kCFBooleanFalse,
                    ),
                    (
                        kVTCompressionPropertyKey_ProfileLevel,
                        kVTProfileLevel_H264_Main_AutoLevel,
                    ),
                    (kVTCompressionPropertyKey_AverageBitRate, bitrate.0),
                    (kVTCompressionPropertyKey_ExpectedFrameRate, fps.0),
                    (
                        kVTCompressionPropertyKey_MaxKeyFrameInterval,
                        key_interval.0,
                    ),
                ]
            };
            for (key, value) in properties {
                if unsafe { VTSessionSetProperty(encoder.session, key, value) } != 0 {
                    return Err(
                        "The hardware encoder does not support the selected settings".into(),
                    );
                }
            }
            if unsafe { VTCompressionSessionPrepareToEncodeFrames(encoder.session) } != 0 {
                return Err("Unable to prepare the hardware video encoder".into());
            }
            Ok(encoder)
        }

        fn encode(&mut self, image: *mut c_void, pts: CMTime) {
            let output = &self.context;
            if self.session.is_null()
                || !output.running.load(Ordering::SeqCst)
                || output.failed.load(Ordering::SeqCst)
            {
                return;
            }
            if output.sender.capacity() == 0 {
                output.need_keyframe.store(true, Ordering::SeqCst);
                return;
            }
            let force = if output.need_keyframe.load(Ordering::SeqCst) {
                match dictionary(&[unsafe {
                    (kVTEncodeFrameOptionKey_ForceKeyFrame, kCFBooleanTrue)
                }]) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        output.fail(&error);
                        return;
                    }
                }
            } else {
                None
            };
            let mut flags = 0;
            let status = unsafe {
                VTCompressionSessionEncodeFrame(
                    self.session,
                    image,
                    pts,
                    CMTime::INVALID,
                    force.as_ref().map_or(ptr::null(), |value| value.0),
                    ptr::null_mut(),
                    &mut flags,
                )
            };
            if status != 0 {
                output.fail("Unable to encode the captured screen frame")
            }
            if flags & 2 != 0 {
                output.need_keyframe.store(true, Ordering::SeqCst)
            }
        }

        fn stop(&mut self) {
            if !self.session.is_null() {
                // CompleteFrames waits for all pending output callbacks before the context is freed.
                unsafe {
                    VTCompressionSessionCompleteFrames(self.session, CMTime::INVALID);
                    VTCompressionSessionInvalidate(self.session);
                    CFRelease(self.session.cast());
                }
                self.session = ptr::null_mut();
            }
        }
    }

    impl Drop for Encoder {
        fn drop(&mut self) {
            self.stop()
        }
    }

    struct Delegate(Arc<Output>);
    impl SCStreamDelegateTrait for Delegate {
        fn did_stop_with_error(&self, _: screencapturekit::error::SCError) {
            self.0
                .fail("Screen capture stopped; check the display and Screen Recording permission");
        }
    }

    pub struct Capture {
        stream: Option<SCStream>,
        handlers: Vec<(usize, SCStreamOutputType)>,
        encoder: Arc<Mutex<Encoder>>,
        output: Arc<Output>,
    }

    impl Capture {
        pub fn start(
            display_id: u32,
            audio: bool,
            quality: &str,
            sender: Sender<MediaPacket>,
            allow_permission_request: bool,
        ) -> Result<Self, String> {
            if sender.max_capacity() < 2 {
                return Err(
                    "Mirror output queue needs capacity for configuration and video".into(),
                );
            }
            let source = displays()?
                .into_iter()
                .find(|display| display.id == display_id)
                .ok_or("The selected display is no longer available")?;
            let profile = profile(source.width, source.height, quality)?;
            if !permission_granted()
                && (!allow_permission_request || !unsafe { CGRequestScreenCaptureAccess() })
            {
                return Err("Screen Recording permission is required. Enable YM_Link in System Settings and reopen the app.".into());
            }
            let runtime = tokio::runtime::Handle::try_current()
                .map_err(|_| "Screen capture must start in the desktop runtime")?;
            let content = SCShareableContent::get().map_err(|_| {
                "Unable to access screen content; check Screen Recording permission"
            })?;
            let display = content
                .displays()
                .into_iter()
                .find(|display| display.display_id() == display_id)
                .ok_or("The selected display is no longer available")?;
            let filter = SCContentFilter::create()
                .with_display(&display)
                .with_excluding_windows(&[])
                .build()
                .map_err(|_| "Unable to configure screen capture")?;
            let configuration = SCStreamConfiguration::new()
                .with_width(profile.width)
                .with_height(profile.height)
                .with_pixel_format(PixelFormat::BGRA)
                .with_minimum_frame_interval(&CMTime::new(1, profile.fps))
                .with_shows_cursor(true)
                .with_queue_depth(3)
                .with_captures_audio(audio)
                .with_sample_rate(48_000)
                .with_channel_count(2)
                .with_excludes_current_process_audio(true);
            let output = Arc::new(Output {
                sender,
                runtime,
                running: AtomicBool::new(true),
                failed: AtomicBool::new(false),
                need_keyframe: AtomicBool::new(true),
                audio_config_sent: AtomicBool::new(false),
                width: profile.width,
                height: profile.height,
            });
            let encoder = Arc::new(Mutex::new(Encoder::new(profile, Arc::clone(&output))?));
            let stream =
                SCStream::new_with_delegate(&filter, &configuration, Delegate(Arc::clone(&output)))
                    .map_err(|_| "Unable to create screen capture stream")?;
            let mut capture = Self {
                stream: Some(stream),
                handlers: Vec::new(),
                encoder: Arc::clone(&encoder),
                output: Arc::clone(&output),
            };
            let video_output = Arc::clone(&output);
            let handler = capture
                .stream
                .as_mut()
                .unwrap()
                .add_output_handler(
                    move |sample: CMSampleBuffer, _: SCStreamOutputType| {
                        if !video_output.running.load(Ordering::SeqCst)
                            || !sample.is_valid()
                            || !sample.data_is_ready()
                        {
                            return;
                        }
                        let image = sample.image_buffer_ptr_borrowed();
                        let pts = sample.presentation_timestamp();
                        if image.is_null() || timestamp(pts).is_none() {
                            return;
                        }
                        if let Ok(mut encoder) = encoder.lock() {
                            encoder.encode(image, pts)
                        }
                    },
                    SCStreamOutputType::Screen,
                )
                .map_err(|_| "Unable to register screen capture output")?;
            capture.handlers.push((handler, SCStreamOutputType::Screen));
            if audio {
                let audio_output = Arc::clone(&output);
                let handler = capture
                    .stream
                    .as_mut()
                    .unwrap()
                    .add_output_handler(
                        move |sample: CMSampleBuffer, _: SCStreamOutputType| {
                            if !audio_output.running.load(Ordering::SeqCst)
                                || audio_output.failed.load(Ordering::SeqCst)
                            {
                                return;
                            }
                            if let Err(error) = audio_output.send_audio(&sample) {
                                audio_output.fail(&error)
                            }
                        },
                        SCStreamOutputType::Audio,
                    )
                    .map_err(|_| "Unable to register system audio output")?;
                capture.handlers.push((handler, SCStreamOutputType::Audio));
            }
            capture
                .stream
                .as_ref()
                .unwrap()
                .start_capture()
                .map_err(|_| "Unable to start screen capture; check Screen Recording permission")?;
            Ok(capture)
        }

        pub fn stop(&mut self) {
            self.output.running.store(false, Ordering::SeqCst);
            if let Some(mut stream) = self.stream.take() {
                let _ = stream.stop_capture();
                for (handler, kind) in self.handlers.drain(..) {
                    let _ = stream.remove_output_handler(handler, kind);
                }
                drop(stream);
            }
            if let Ok(mut encoder) = self.encoder.lock() {
                encoder.stop()
            }
        }
    }

    impl Drop for Capture {
        fn drop(&mut self) {
            self.stop()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[link(name = "CoreVideo", kind = "framework")]
        extern "C" {
            fn CVPixelBufferCreate(
                allocator: Ref,
                width: usize,
                height: usize,
                format: u32,
                attributes: Ref,
                out: *mut *mut c_void,
            ) -> i32;
            fn CVPixelBufferLockBaseAddress(buffer: *mut c_void, flags: usize) -> i32;
            fn CVPixelBufferUnlockBaseAddress(buffer: *mut c_void, flags: usize) -> i32;
            fn CVPixelBufferGetBaseAddress(buffer: *mut c_void) -> *mut c_void;
            fn CVPixelBufferGetBytesPerRow(buffer: *mut c_void) -> usize;
        }

        #[test]
        fn profiles_and_pcm_keep_dimensions_and_channels() {
            let p = profile(3456, 2234, "balanced").unwrap();
            assert_eq!(
                (p.width, p.height, p.fps, p.bitrate),
                (1920, 1240, 60, 12_000_000)
            );
            assert!(profile(100, 100, "unknown").is_err());
            assert_eq!(timestamp(CMTime::new(3, 2)), Some(1_500_000));
            assert_eq!(timestamp(CMTime::INVALID), None);
            let left: Vec<_> = [1.0f32, 2.0]
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            let right: Vec<_> = [3.0f32, 4.0]
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            let expected: Vec<_> = [1.0f32, 3.0, 2.0, 4.0]
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            assert_eq!(
                interleave_audio(&[&left, &right], 2, true).unwrap(),
                expected
            );
            assert!(interleave_audio(&[&left], 2, true).is_err());
        }

        #[tokio::test]
        #[ignore = "requires a real Mac hardware H.264 encoder; no screen capture or permission prompt"]
        async fn hardware_encoder_emits_avcc_keyframe_without_screen_capture() {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
            let output = Arc::new(Output {
                sender,
                runtime: tokio::runtime::Handle::current(),
                running: AtomicBool::new(true),
                failed: AtomicBool::new(false),
                need_keyframe: AtomicBool::new(true),
                audio_config_sent: AtomicBool::new(false),
                width: 64,
                height: 64,
            });
            let mut encoder =
                Encoder::new(profile(64, 64, "smooth").unwrap(), Arc::clone(&output)).unwrap();
            let mut pixel = ptr::null_mut();
            unsafe {
                assert_eq!(
                    CVPixelBufferCreate(
                        ptr::null(),
                        64,
                        64,
                        u32::from_be_bytes(*b"BGRA"),
                        ptr::null(),
                        &mut pixel
                    ),
                    0
                );
                assert!(!pixel.is_null());
            }
            let _pixel_owner = Cf(pixel.cast());
            unsafe {
                assert_eq!(CVPixelBufferLockBaseAddress(pixel, 0), 0);
                let base = CVPixelBufferGetBaseAddress(pixel);
                assert!(!base.is_null());
                ptr::write_bytes(base, 0, CVPixelBufferGetBytesPerRow(pixel) * 64);
                assert_eq!(CVPixelBufferUnlockBaseAddress(pixel, 0), 0);
            }
            encoder.encode(pixel, CMTime::new(0, 30));
            encoder.stop();
            let config = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                config.kind,
                "video_config",
                "{}",
                String::from_utf8_lossy(&config.data)
            );
            assert_eq!(config.codec, "h264-avcc");
            assert_eq!(config.data[0], 1);
            assert_eq!(config.data[4] & 3, 3);
            let frame = receiver.recv().await.unwrap();
            assert_eq!(frame.kind, "video");
            assert!(frame.keyframe);
            assert!(!frame.data.is_empty());
            let nal_length = u32::from_be_bytes(frame.data[..4].try_into().unwrap()) as usize;
            assert!(nal_length > 0 && nal_length + 4 <= frame.data.len());
            assert!(!output.failed.load(Ordering::SeqCst));
        }
    }
}

#[cfg(target_os = "macos")]
pub use platform::{displays, permission_granted, Capture};

#[cfg(not(target_os = "macos"))]
pub fn displays() -> Result<Vec<Display>, String> {
    Ok(Vec::new())
}
#[cfg(not(target_os = "macos"))]
pub fn permission_granted() -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub struct Capture;
#[cfg(not(target_os = "macos"))]
impl Capture {
    pub fn start(
        _: u32,
        _: bool,
        _: &str,
        _: Sender<MediaPacket>,
        _: bool,
    ) -> Result<Self, String> {
        Err("Native screen capture is currently available on macOS only".into())
    }
    pub fn stop(&mut self) {}
}
