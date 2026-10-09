use std::{
    io,
    sync::{Arc, Mutex},
};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use nokhwa::{
    pixel_format::RgbAFormat,
    query,
    utils::{ApiBackend, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType},
    Camera,
};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::collections::{HashMap, HashSet};

use hbb_common::message_proto::{DisplayInfo, Resolution};
// Fork: `log` is not a direct dependency of scrap on desktop; re-exported by hbb_common.
#[cfg(any(target_os = "windows", target_os = "linux"))]
use hbb_common::log;

#[cfg(feature = "vram")]
use crate::AdapterDevice;

use crate::common::{bail, ResultType};
use crate::{Frame, TraitCapturer};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::{PixelBuffer, Pixfmt};

pub const PRIMARY_CAMERA_IDX: usize = 0;
// Fork: consecutive capture failures before a camera is reopened in the driver's default
// format (and, after as many more, refused). See CAMERA_FAILURES below.
#[cfg(any(target_os = "windows", target_os = "linux"))]
const CAMERA_FAILURES_BEFORE_FALLBACK: u32 = 3;
lazy_static::lazy_static! {
    static ref SYNC_CAMERA_DISPLAYS: Arc<Mutex<Vec<DisplayInfo>>> = Arc::new(Mutex::new(Vec::new()));
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
lazy_static::lazy_static! {
    // Fork: cameras whose capture kept failing (stream wouldn't open, or frames didn't match
    // their own advertised format/resolution). The next capturer for such a camera asks the
    // driver for its *default* format instead of the highest resolution - the mode a driver is
    // most likely to deliver correctly. Found via real testing: a virtual "WiFi camera" driver
    // killed the remote process the instant a client selected it (the release build aborts on
    // panic, so a frame the capture library couldn't make sense of left no trace in the log,
    // three times).
    static ref CAMERA_SAFE_FORMAT: Arc<Mutex<HashSet<u32>>> = Default::default();
    // Fork: consecutive capture failures per camera index. Escalation (safe format, then
    // unusable) happens only after CAMERA_FAILURES_BEFORE_FALLBACK in a row, so a single
    // transient error on a healthy camera never changes anything; a good frame resets it.
    static ref CAMERA_FAILURES: Arc<Mutex<HashMap<u32, u32>>> = Default::default();
    // Fork: cameras that failed in the safe format too - refused outright with a clear error
    // instead of being retried forever. Cleared by restarting the process.
    static ref CAMERA_UNUSABLE: Arc<Mutex<HashSet<u32>>> = Default::default();
    // Fork: our display index -> the device's stable id (Windows: the Media Foundation
    // symbolic link, nokhwa's `CameraInfo::misc()`; Linux: unused, index-based open is kept).
    // nokhwa opens `CameraIndex::Index(i)` by re-enumerating devices and taking the i-th, so
    // when a flaky virtual camera driver drops out of / re-enters the enumeration between the
    // listing and the open, index i can land on a *different* camera (found via real testing:
    // the same physical camera shown twice, standing in for the broken one). Opening by id
    // (`CameraIndex::String`, matched on the symbolic link) cannot be redirected like that: a
    // vanished device is an open error, never another camera. Built by all_info().
    static ref CAMERA_IDS: Arc<Mutex<Vec<String>>> = Default::default();
    // Fork: device id -> last known resolution. all_info() runs on every camera login and used
    // to open every camera each time just to read its resolution - every login touched the
    // broken driver again. Cached after the first successful open; the capturer corrects the
    // size anyway once it actually opens the camera (CameraCapturer::new).
    static ref CAMERA_RESOLUTIONS: Arc<Mutex<HashMap<String, (i32, i32)>>> = Default::default();
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
const CAMERA_NOT_SUPPORTED: &str = "This platform doesn't support camera yet";

pub struct Cameras;

// pre-condition
pub fn primary_camera_exists() -> bool {
    Cameras::exists(PRIMARY_CAMERA_IDX)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Cameras {
    pub fn all_info() -> ResultType<Vec<DisplayInfo>> {
        match query(ApiBackend::Auto) {
            Ok(cameras) => {
                let mut camera_displays = SYNC_CAMERA_DISPLAYS.lock().unwrap();
                camera_displays.clear();
                // FIXME: nokhwa returns duplicate info for one physical camera on linux for now.
                // issue: https://github.com/l1npengtul/nokhwa/issues/171
                // Use only one camera as a temporary hack.
                cfg_if::cfg_if! {
                    if #[cfg(target_os = "linux")] {
                        let Some(info) = cameras.first() else {
                            bail!("No camera found")
                        };
                        // Use index (0) camera as main camera, fallback to the first camera if index (0) is not available.
                        // But maybe we also need to check index (1) or the lowest index camera.
                        //
                        // https://askubuntu.com/questions/234362/how-to-fix-this-problem-where-sometimes-dev-video0-becomes-automatically-dev
                        // https://github.com/rustdesk/rustdesk/pull/12010#issue-3125329069
                        let mut camera_index = info.index().clone();
                        if !matches!(camera_index, CameraIndex::Index(0)) {
                            if cameras.iter().any(|cam| matches!(cam.index(), CameraIndex::Index(0))) {
                                camera_index = CameraIndex::Index(0);
                            }
                        }
                        let camera = Self::open_camera(0, camera_index)?;
                        let resolution = camera.resolution();
                        let (width, height) = (resolution.width() as i32, resolution.height() as i32);
                        camera_displays.push(DisplayInfo {
                            x: 0,
                            y: 0,
                            name: info.human_name().clone(),
                            width,
                            height,
                            online: true,
                            cursor_embedded: false,
                            scale:1.0,
                            original_resolution: Some(Resolution {
                                width,
                                height,
                                ..Default::default()
                            }).into(),
                            ..Default::default()
                        });
                    } else {
                        // Fork: see CAMERA_IDS / CAMERA_RESOLUTIONS. Compared with upstream:
                        // duplicates (same device id listed twice) are skipped; a camera that
                        // cannot be opened is listed as offline with a placeholder size instead
                        // of failing the whole list (`?`), so one broken device no longer hides
                        // the working ones; and a camera whose size is already known is not
                        // reopened. Our display index is the position in `ids`.
                        let mut x = 0;
                        let mut ids: Vec<String> = Vec::new();
                        for info in &cameras {
                            let id = info.misc();
                            let name = info.human_name();
                            if !id.is_empty() && ids.contains(&id) {
                                log::warn!("camera '{name}' listed twice by the system ({id}); skipping the duplicate");
                                continue;
                            }
                            let our_idx = ids.len() as u32;
                            let cached = CAMERA_RESOLUTIONS.lock().unwrap().get(&id).cloned();
                            let (width, height, online) = match cached {
                                Some((w, h)) => (w, h, true),
                                None => match Self::open_camera(our_idx, CameraIndex::String(id.clone())) {
                                    Ok(camera) => {
                                        let r = camera.resolution();
                                        let (w, h) = (r.width() as i32, r.height() as i32);
                                        CAMERA_RESOLUTIONS.lock().unwrap().insert(id.clone(), (w, h));
                                        (w, h, true)
                                    }
                                    Err(e) => {
                                        let reason = format!("{e}");
                                        log::warn!("camera{our_idx} '{name}' cannot be opened ({reason}); listed as offline");
                                        Self::note_capture_failure(our_idx, &reason);
                                        (640, 480, false)
                                    }
                                },
                            };
                            ids.push(id);
                            camera_displays.push(DisplayInfo {
                                x,
                                y: 0,
                                name,
                                width,
                                height,
                                online,
                                cursor_embedded: false,
                                scale:1.0,
                                original_resolution: Some(Resolution {
                                    width,
                                    height,
                                    ..Default::default()
                                }).into(),
                                ..Default::default()
                            });
                            x += width;
                        }
                        *CAMERA_IDS.lock().unwrap() = ids;
                    }
                }
                Ok(camera_displays.clone())
            }
            Err(e) => {
                bail!("Query cameras error: {}", e)
            }
        }
    }

    pub fn exists(index: usize) -> bool {
        // Fork: once all_info() has built the (de-duplicated) list, that is the authority.
        let known = CAMERA_IDS.lock().unwrap().len();
        if known > 0 {
            return index < known;
        }
        match query(ApiBackend::Auto) {
            Ok(cameras) => index < cameras.len(),
            _ => return false,
        }
    }

    // Fork: record that capturing from camera `index` failed. First failure -> that camera is
    // reopened in the driver's default format from now on; a failure in the default format too
    // -> the camera is refused (see CAMERA_SAFE_FORMAT / CAMERA_UNUSABLE). Both outcomes are
    // logged loudly so the log shows *why* a camera stopped, instead of the process dying with
    // nothing written (the release build aborts on panic).
    pub fn note_capture_failure(index: u32, reason: &str) {
        if CAMERA_UNUSABLE.lock().unwrap().contains(&index) {
            return;
        }
        let failures = {
            let mut map = CAMERA_FAILURES.lock().unwrap();
            let n = map.entry(index).or_insert(0);
            *n += 1;
            *n
        };
        if failures < CAMERA_FAILURES_BEFORE_FALLBACK {
            log::warn!(
                "camera{index}: capture failed ({reason}), {failures}/{CAMERA_FAILURES_BEFORE_FALLBACK}"
            );
            return;
        }
        CAMERA_FAILURES.lock().unwrap().remove(&index);
        if CAMERA_SAFE_FORMAT.lock().unwrap().insert(index) {
            log::warn!(
                "camera{index}: capture failed {failures} times in a row ({reason}); will reopen \
                 it in the driver's default format instead of the highest resolution"
            );
        } else {
            CAMERA_UNUSABLE.lock().unwrap().insert(index);
            log::error!(
                "camera{index}: capture failed {failures} times in a row in the driver's default \
                 format too ({reason}); refusing this camera until restart"
            );
        }
    }

    // Fork: a frame decoded fine - forget any earlier transient failures for this camera.
    pub fn note_capture_ok(index: u32) {
        CAMERA_FAILURES.lock().unwrap().remove(&index);
    }

    // Fork: open our display index `idx`. On Windows this resolves to the device's stable id
    // recorded by all_info() (see CAMERA_IDS) and opens by `CameraIndex::String`; before
    // all_info() has run, or on Linux, it falls back to nokhwa's enumeration index.
    fn create_camera(idx: u32) -> ResultType<Camera> {
        let id = CAMERA_IDS.lock().unwrap().get(idx as usize).cloned();
        let index = match id {
            Some(id) if cfg!(target_os = "windows") && !id.is_empty() => CameraIndex::String(id),
            _ => CameraIndex::Index(idx),
        };
        Self::open_camera(idx, index)
    }

    // Fork: the actual open, keyed by our display index for the failure bookkeeping
    // (CAMERA_UNUSABLE / CAMERA_SAFE_FORMAT). `index` is whatever nokhwa should open.
    fn open_camera(idx: u32, index: CameraIndex) -> ResultType<Camera> {
        if CAMERA_UNUSABLE.lock().unwrap().contains(&idx) {
            bail!(
                "camera{} refused: it produced unusable frames in both its highest-resolution \
                 and default formats",
                idx
            );
        }
        let safe_format = CAMERA_SAFE_FORMAT.lock().unwrap().contains(&idx);
        let format_type = if cfg!(target_os = "linux") || safe_format {
            RequestedFormatType::None
        } else {
            RequestedFormatType::AbsoluteHighestResolution
        };
        let result = Camera::new(index, RequestedFormat::new::<RgbAFormat>(format_type));
        match result {
            Ok(camera) => Ok(camera),
            Err(e) => bail!("create camera{} error:  {}", idx, e),
        }
    }

    pub fn get_camera_resolution(index: usize) -> ResultType<Resolution> {
        // Fork: all_info() has just listed this camera (login) or the capturer has opened it;
        // use that size instead of opening the device a second time. Only an unknown index
        // (no list yet) still opens the camera.
        if let Some(info) = SYNC_CAMERA_DISPLAYS.lock().unwrap().get(index) {
            return Ok(Resolution {
                width: info.width,
                height: info.height,
                ..Default::default()
            });
        }
        let camera = Self::create_camera(index as u32)?;
        let resolution = camera.resolution();
        Ok(Resolution {
            width: resolution.width() as i32,
            height: resolution.height() as i32,
            ..Default::default()
        })
    }

    pub fn get_sync_cameras() -> Vec<DisplayInfo> {
        SYNC_CAMERA_DISPLAYS.lock().unwrap().clone()
    }

    pub fn get_capturer(current: usize) -> ResultType<Box<dyn TraitCapturer>> {
        Ok(Box::new(CameraCapturer::new(current)?))
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
impl Cameras {
    pub fn all_info() -> ResultType<Vec<DisplayInfo>> {
        return Ok(Vec::new());
    }

    pub fn exists(_index: usize) -> bool {
        false
    }

    pub fn get_camera_resolution(_index: usize) -> ResultType<Resolution> {
        bail!(CAMERA_NOT_SUPPORTED);
    }

    pub fn get_sync_cameras() -> Vec<DisplayInfo> {
        vec![]
    }

    pub fn get_capturer(_current: usize) -> ResultType<Box<dyn TraitCapturer>> {
        bail!(CAMERA_NOT_SUPPORTED);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct CameraCapturer {
    camera: Camera,
    index: u32,
    // Fork: set once the first frame decodes - the only time the failure counter is reset, so
    // the normal streaming loop never touches a lock.
    first_frame_ok: bool,
    data: Vec<u8>,
    last_data: Vec<u8>, // for faster compare and copy
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub struct CameraCapturer;

impl CameraCapturer {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn new(current: usize) -> ResultType<Self> {
        let camera = Cameras::create_camera(current as u32)?;
        // Fork: the cached camera list may still advertise the highest-resolution mode (or the
        // placeholder size of a camera that could not be opened at login); now that the camera
        // is actually open, publish the real size so the video service sizes its encoder
        // correctly (see get_capturer_camera in src/server/video_service.rs) and remember it
        // for the next login listing (CAMERA_RESOLUTIONS).
        {
            let res = camera.resolution();
            let (w, h) = (res.width() as i32, res.height() as i32);
            if let Some(info) = SYNC_CAMERA_DISPLAYS.lock().unwrap().get_mut(current) {
                if info.width != w || info.height != h {
                    log::info!(
                        "camera{current}: advertised {}x{}, actual format {w}x{h}",
                        info.width,
                        info.height
                    );
                    info.width = w;
                    info.height = h;
                }
                info.online = true;
            }
            if let Some(id) = CAMERA_IDS.lock().unwrap().get(current) {
                CAMERA_RESOLUTIONS
                    .lock()
                    .unwrap()
                    .insert(id.clone(), (w, h));
            }
        }
        Ok(CameraCapturer {
            camera,
            index: current as u32,
            first_frame_ok: false,
            data: Vec::new(),
            last_data: Vec::new(),
        })
    }

    #[allow(dead_code)]
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn new(_current: usize) -> ResultType<Self> {
        bail!(CAMERA_NOT_SUPPORTED);
    }

    // Fork: sanity-check a raw frame against the format and resolution the driver declared for
    // it. Only uncompressed formats have a fixed size; compressed ones (MJPEG) are checked for
    // emptiness only. Returns a human-readable reason on mismatch. (Inherent, not part of
    // `TraitCapturer`.)
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn validate_buffer(w: usize, h: usize, fmt: FrameFormat, len: usize) -> Result<(), String> {
        if w == 0 || h == 0 {
            return Err(format!("camera reported an empty resolution {w}x{h}"));
        }
        if len == 0 {
            return Err(format!("camera delivered an empty {fmt:?} frame for {w}x{h}"));
        }
        let expected = match fmt {
            FrameFormat::YUYV => Some(w * h * 2),
            FrameFormat::NV12 => Some(w * h * 3 / 2),
            _ => None, // MJPEG and anything else: variable or unknown size, can't be checked
        };
        if let Some(expected) = expected {
            if len < expected {
                return Err(format!(
                    "camera delivered a {fmt:?} frame of {len} bytes for {w}x{h}, expected {expected}"
                ));
            }
        }
        Ok(())
    }
}

impl TraitCapturer for CameraCapturer {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<Frame<'a>> {
        // TODO: move this check outside `frame`.
        if !self.camera.is_stream_open() {
            if let Err(e) = self.camera.open_stream() {
                let reason = format!("Camera open stream error: {}", e);
                Cameras::note_capture_failure(self.index, &reason);
                return Err(io::Error::new(io::ErrorKind::Other, reason));
            }
        }
        match self.camera.frame() {
            Ok(buffer) => {
                // Fork: refuse a frame that doesn't match what the driver itself declared,
                // before the capture library decodes it. A driver that advertises one
                // resolution/format and delivers another (seen with a virtual "WiFi camera")
                // otherwise gets fed into unsafe decoding paths; with panic = 'abort' in the
                // release profile that ends the whole remote process without a line in the log.
                // Here it becomes an ordinary, logged capture error instead - and the camera is
                // reopened in the driver's default format next time (note_capture_failure).
                if let Err(reason) = Self::validate_buffer(
                    buffer.resolution().width() as usize,
                    buffer.resolution().height() as usize,
                    buffer.source_frame_format(),
                    buffer.buffer().len(),
                ) {
                    Cameras::note_capture_failure(self.index, &reason);
                    return Err(io::Error::new(io::ErrorKind::Other, reason));
                }
                match buffer.decode_image::<RgbAFormat>() {
                    Ok(decoded) => {
                        if !self.first_frame_ok {
                            self.first_frame_ok = true;
                            Cameras::note_capture_ok(self.index);
                        }
                        self.data = decoded.as_raw().to_vec();
                        crate::would_block_if_equal(&mut self.last_data, &self.data)?;
                        // FIXME: macos's PixelBuffer cannot be directly created from bytes slice.
                        cfg_if::cfg_if! {
                            if #[cfg(any(target_os = "linux", target_os = "windows"))] {
                                Ok(Frame::PixelBuffer(PixelBuffer::new(
                                    &self.data,
                                    Pixfmt::RGBA,
                                    decoded.width() as usize,
                                    decoded.height() as usize,
                                )))
                            } else {
                                Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    format!("Camera is not supported on this platform yet"),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        let reason = format!("Camera frame decode error: {}", e);
                        Cameras::note_capture_failure(self.index, &reason);
                        Err(io::Error::new(io::ErrorKind::Other, reason))
                    }
                }
            }
            Err(e) => {
                let reason = format!("Camera frame error: {}", e);
                Cameras::note_capture_failure(self.index, &reason);
                Err(io::Error::new(io::ErrorKind::Other, reason))
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<Frame<'a>> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            CAMERA_NOT_SUPPORTED.to_string(),
        ))
    }

    #[cfg(windows)]
    fn is_gdi(&self) -> bool {
        true
    }

    #[cfg(windows)]
    fn set_gdi(&mut self) -> bool {
        true
    }

    #[cfg(feature = "vram")]
    fn device(&self) -> AdapterDevice {
        AdapterDevice::default()
    }

    #[cfg(feature = "vram")]
    fn set_output_texture(&mut self, _texture: bool) {}
}
