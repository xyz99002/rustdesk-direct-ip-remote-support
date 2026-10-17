use std::{
    io,
    sync::{Arc, Mutex},
};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use nokhwa::{
    pixel_format::RgbAFormat,
    query,
    utils::{
        ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
    },
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
// Fork: capture-mode limits for `Cameras::choose_format()` (Windows). A camera is opened in its
// best mode within this size, like a camera application would, not in its absolute top mode.
#[cfg(any(target_os = "windows", target_os = "linux"))]
const MAX_CAPTURE_WIDTH: u32 = 1920;
#[cfg(any(target_os = "windows", target_os = "linux"))]
const MAX_CAPTURE_HEIGHT: u32 = 1080;
// Fork: an uncompressed mode (NV12/YUYV) is preferred over MJPEG at the same size only when it
// runs at least this fast; USB 2 cameras typically offer 1080p uncompressed at 5 fps only.
#[cfg(any(target_os = "windows", target_os = "linux"))]
const MIN_UNCOMPRESSED_FPS: u32 = 15;
// Fork: whether this build can decode MJPEG camera frames (TurboJPEG, see Cargo.toml). False
// only on Windows-on-ARM, where the library is not built; such a build never chooses an MJPEG
// mode and refuses MJPEG frames with a clear error instead of crashing.
#[cfg(any(target_os = "windows", target_os = "linux"))]
const MJPEG_SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    all(target_os = "windows", target_arch = "x86_64")
));
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

// Fork: keep Media Foundation started for the whole process (Windows).
//
// nokhwa (the camera library) counts its open camera objects and, when the count drops to
// zero, calls MFShutdown()/CoUninitialize() from whichever thread dropped the last camera
// (`MediaFoundationDevice::drop` -> `de_initialize_mf`). With several camera services
// starting and stopping concurrently - a local viewing all cameras, each new viewer making
// the others restart ("SWITCH"), a login listing opening and dropping every camera - one
// thread shuts the platform down while another is inside ReadSample()/ActivateObject(). That
// is an access violation inside Media Foundation: the remote process dies with no log line
// (found via real testing on 2026-10-08, every kill right after a concurrent camera service
// restart; the count is also not updated atomically). Windows reference-counts MFStartup/
// MFShutdown, so one extra MFStartup that is never matched keeps the platform alive no
// matter how often the library shuts "its" reference down. Called before every camera use.
#[cfg(target_os = "windows")]
mod mf_pin {
    #[link(name = "mfplat")]
    extern "system" {
        fn MFStartup(version: u32, flags: u32) -> i32;
    }
    // mfapi.h: MF_VERSION = (MF_SDK_VERSION 0x0002 << 16) | MF_API_VERSION 0x0070
    const MF_VERSION: u32 = 0x0002_0070;
    const MFSTARTUP_NOSOCKET: u32 = 0x1;

    pub fn pin() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let hr = unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) };
            if hr < 0 {
                hbb_common::log::warn!(
                    "Media Foundation could not be pinned (MFStartup HRESULT {hr:#010x}); \
                     concurrent camera restarts may still shut it down"
                );
            } else {
                hbb_common::log::info!("Media Foundation pinned for the process lifetime");
            }
        });
    }
}

#[cfg(target_os = "windows")]
fn pin_media_foundation() {
    mf_pin::pin();
}

#[cfg(not(target_os = "windows"))]
fn pin_media_foundation() {}

pub struct Cameras;

// pre-condition
pub fn primary_camera_exists() -> bool {
    Cameras::exists(PRIMARY_CAMERA_IDX)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Cameras {
    pub fn all_info() -> ResultType<Vec<DisplayInfo>> {
        pin_media_foundation();
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
        pin_media_foundation();
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
        pin_media_foundation();
        if CAMERA_UNUSABLE.lock().unwrap().contains(&idx) {
            bail!(
                "camera{} refused: it produced unusable frames in both its preferred and \
                 default modes",
                idx
            );
        }
        let safe_format = CAMERA_SAFE_FORMAT.lock().unwrap().contains(&idx);
        // Fork: open in the driver's first listed mode (what `RequestedFormatType::None`
        // picks) and, on Windows, re-choose from the full mode list below. Upstream asks for
        // `AbsoluteHighestResolution` on Windows, which on most cameras is an MJPEG-only mode
        // far above what the link (USB 2, a Wi-Fi camera adapter) carries cleanly - found via
        // real testing: a camera that advertised 1080p was opened at 2560x1440, and damaged
        // frames from a wireless camera killed the remote (see decode_mjpeg_rgba). Linux keeps
        // the first listed mode as upstream does; so does a camera in its "safe format" after
        // repeated failures (CAMERA_SAFE_FORMAT).
        let mut camera = match Camera::new(
            index,
            RequestedFormat::new::<RgbAFormat>(RequestedFormatType::None),
        ) {
            Ok(camera) => camera,
            Err(e) => bail!("create camera{} error:  {}", idx, e),
        };
        if cfg!(target_os = "linux") || safe_format {
            log::info!(
                "camera{idx}: opened in the driver's default mode {}",
                camera.camera_format()
            );
            return Ok(camera);
        }
        let formats = match camera.compatible_camera_formats() {
            Ok(formats) => formats,
            Err(e) => {
                log::warn!(
                    "camera{idx}: cannot list its modes ({e}); staying in {}",
                    camera.camera_format()
                );
                return Ok(camera);
            }
        };
        // One line per open with everything the driver offers - this is what a camera report
        // needs: which modes exist, which one is in use, MJPEG or not.
        let offered = Self::describe_formats(&formats);
        match Self::choose_format(&formats) {
            Some(wanted) if wanted != camera.camera_format() => {
                match camera.set_camera_requset(RequestedFormat::new::<RgbAFormat>(
                    RequestedFormatType::Exact(wanted),
                )) {
                    Ok(actual) => {
                        log::info!("camera{idx}: modes offered: {offered}; using {actual}")
                    }
                    Err(e) => log::warn!(
                        "camera{idx}: modes offered: {offered}; could not switch to {wanted} \
                         ({e}); staying in {}",
                        camera.camera_format()
                    ),
                }
            }
            Some(_) => log::info!(
                "camera{idx}: modes offered: {offered}; using {}",
                camera.camera_format()
            ),
            None => log::warn!(
                "camera{idx}: modes offered: {offered}; none of them is decodable, staying in {}",
                camera.camera_format()
            ),
        }
        Ok(camera)
    }

    // Fork: choose the capture mode the way a camera application does. Rules, in order: only
    // modes we can decode; resolutions within MAX_CAPTURE_WIDTH x MAX_CAPTURE_HEIGHT (if every
    // mode is above that, the smallest offered); the highest such resolution; at that
    // resolution an uncompressed mode (NV12/YUYV/...) when it runs at >= MIN_UNCOMPRESSED_FPS,
    // else MJPEG; the highest frame rate last. Uncompressed first because it needs no JPEG
    // decode at all and is what Windows' Frame Server hands out for a camera in shared mode.
    fn choose_format(formats: &[CameraFormat]) -> Option<CameraFormat> {
        let area = |f: &CameraFormat| f.width() as u64 * f.height() as u64;
        let decodable: Vec<CameraFormat> = formats
            .iter()
            .copied()
            .filter(|f| f.frame_rate() > 0 && f.width() > 0 && f.height() > 0)
            .filter(|f| {
                matches!(
                    f.format(),
                    FrameFormat::MJPEG
                        | FrameFormat::YUYV
                        | FrameFormat::NV12
                        | FrameFormat::GRAY
                        | FrameFormat::RAWRGB
                )
            })
            .filter(|f| MJPEG_SUPPORTED || f.format() != FrameFormat::MJPEG)
            .collect();
        let within: Vec<CameraFormat> = decodable
            .iter()
            .copied()
            .filter(|f| f.width() <= MAX_CAPTURE_WIDTH && f.height() <= MAX_CAPTURE_HEIGHT)
            .collect();
        let pool: Vec<CameraFormat> = if within.is_empty() {
            let smallest = decodable.iter().map(area).min()?;
            decodable
                .into_iter()
                .filter(|f| area(f) == smallest)
                .collect()
        } else {
            within
        };
        let best = pool.iter().map(area).max()?;
        let at_best: Vec<CameraFormat> = pool.into_iter().filter(|f| area(f) == best).collect();
        let uncompressed = at_best
            .iter()
            .copied()
            .filter(|f| f.format() != FrameFormat::MJPEG && f.frame_rate() >= MIN_UNCOMPRESSED_FPS)
            .max_by_key(|f| f.frame_rate());
        if uncompressed.is_some() {
            return uncompressed;
        }
        let mjpeg = at_best
            .iter()
            .copied()
            .filter(|f| f.format() == FrameFormat::MJPEG)
            .max_by_key(|f| f.frame_rate());
        mjpeg.or_else(|| at_best.into_iter().max_by_key(|f| f.frame_rate()))
    }

    // Fork: "1920x1080 MJPEG@30, 1280x720 NV12@30, ..." - one entry per resolution and format
    // with its best frame rate, so the log shows what a driver offers without every rate.
    fn describe_formats(formats: &[CameraFormat]) -> String {
        let mut best: Vec<(u32, u32, FrameFormat, u32)> = Vec::new();
        for f in formats {
            match best
                .iter_mut()
                .find(|(w, h, fmt, _)| *w == f.width() && *h == f.height() && *fmt == f.format())
            {
                Some(entry) => entry.3 = entry.3.max(f.frame_rate()),
                None => best.push((f.width(), f.height(), f.format(), f.frame_rate())),
            }
        }
        best.iter()
            .map(|(w, h, fmt, fps)| format!("{w}x{h} {fmt:?}@{fps}"))
            .collect::<Vec<_>>()
            .join(", ")
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

    // Fork: MJPEG via TurboJPEG, not via nokhwa's `decode_image` (= mozjpeg). Same decoder
    // engine and speed - mozjpeg is libjpeg-turbo plus encoder extras - but a different failure
    // mode: TurboJPEG reports a bad frame through a return code, whereas mozjpeg's error handler
    // unwinds the stack with `std::panic::resume_unwind`, which by design skips the panic hook,
    // and under this build's `panic = 'abort'` release profile an unwind is an immediate,
    // silent process abort. Found via real testing (2026-10-14): a remote died ~20 s after every
    // camera connect with nothing in the log; the same camera worked on USB and failed through a
    // wireless adapter, whose link delivers truncated/garbled JPEG frames that the Windows
    // Camera app simply drops. Here such a frame is an ordinary capture error (logged and
    // counted by the caller) and the next frame is shown.
    // Windows-on-ARM: no TurboJPEG in this build (see Cargo.toml), so an MJPEG frame is an
    // ordinary capture error; choose_format() avoids MJPEG modes on this target anyway.
    #[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
    fn decode_mjpeg_rgba(_data: &[u8], _width: usize, _height: usize) -> Result<Vec<u8>, String> {
        Err("MJPEG camera frames are not supported by this build (Windows on ARM); \
             the camera needs an uncompressed mode (NV12/YUY2)"
            .to_owned())
    }

    #[cfg(any(target_os = "linux", all(target_os = "windows", target_arch = "x86_64")))]
    fn decode_mjpeg_rgba(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
        // Cheap structural pre-check before the decoder sees the bytes: SOI at the start, EOI
        // near the end (drivers may pad after it). Rejects the common wireless damage -
        // truncated frames and non-JPEG payloads - for free.
        if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
            return Err(format!(
                "MJPEG frame of {} bytes does not start with a JPEG SOI marker",
                data.len()
            ));
        }
        let tail = &data[data.len().saturating_sub(64)..];
        if !tail.windows(2).any(|m| m == [0xFF, 0xD9]) {
            return Err(format!(
                "MJPEG frame of {} bytes is truncated (no EOI marker)",
                data.len()
            ));
        }
        let img = turbojpeg::decompress(data, turbojpeg::PixelFormat::RGBA)
            .map_err(|e| format!("libjpeg-turbo rejected the MJPEG frame: {e}"))?;
        if img.width != width || img.height != height {
            return Err(format!(
                "MJPEG frame decodes to {}x{}, camera mode says {width}x{height}",
                img.width, img.height
            ));
        }
        let row = width * 4;
        if img.pitch == row && img.pixels.len() == row * height {
            return Ok(img.pixels);
        }
        if img.pitch < row || img.pixels.len() < img.pitch * height {
            return Err(format!(
                "decoded MJPEG frame has an inconsistent layout (pitch {}, {} bytes for {width}x{height})",
                img.pitch,
                img.pixels.len()
            ));
        }
        // Padded rows: compact to the tightly packed RGBA the rest of the pipeline expects.
        let mut out = Vec::with_capacity(row * height);
        for y in 0..height {
            let start = y * img.pitch;
            out.extend_from_slice(&img.pixels[start..start + row]);
        }
        Ok(out)
    }

    // Fork: NV12 / YUYV through nokhwa's own converters, but on a buffer trimmed to the exact
    // size the mode implies. The converters insist on an exact length, and Windows - notably
    // the Frame Server that serves a camera in "shared" mode - often hands over padded
    // buffers, which made every frame of such a camera a decode error. validate_buffer() has
    // already guaranteed the buffer is at least that long.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn decode_nv12_rgba(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
        let expected = width * height * 3 / 2;
        nokhwa::utils::nv12_to_rgb(
            nokhwa::utils::Resolution::new(width as u32, height as u32),
            &data[..expected],
            true,
        )
        .map_err(|e| format!("NV12 conversion failed: {e}"))
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn decode_yuyv_rgba(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
        let expected = width * height * 2;
        nokhwa::utils::yuyv422_to_rgb(&data[..expected], true)
            .map_err(|e| format!("YUYV conversion failed: {e}"))
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
                let width = buffer.resolution().width() as usize;
                let height = buffer.resolution().height() as usize;
                let fmt = buffer.source_frame_format();
                let raw = buffer.buffer();
                // Fork: refuse a frame that doesn't match what the driver itself declared,
                // before anything decodes it. A driver that advertises one resolution/format
                // and delivers another (seen with a virtual "WiFi camera") otherwise gets fed
                // into decoding paths that assume the declared size. Here it becomes an
                // ordinary, logged capture error instead - and the camera is reopened in the
                // driver's default mode after repeated failures (note_capture_failure).
                if let Err(reason) = Self::validate_buffer(width, height, fmt, raw.len()) {
                    Cameras::note_capture_failure(self.index, &reason);
                    return Err(io::Error::new(io::ErrorKind::Other, reason));
                }
                // Fork: decode here rather than through `Buffer::decode_image` - see
                // decode_mjpeg_rgba (abort-proof MJPEG) and decode_nv12_rgba/decode_yuyv_rgba
                // (padded buffers). The remaining formats (GRAY, RAWRGB) are plain byte
                // expansions in nokhwa and stay on its path.
                let decoded = match fmt {
                    FrameFormat::MJPEG => Self::decode_mjpeg_rgba(raw, width, height),
                    FrameFormat::NV12 => Self::decode_nv12_rgba(raw, width, height),
                    FrameFormat::YUYV => Self::decode_yuyv_rgba(raw, width, height),
                    _ => buffer
                        .decode_image::<RgbAFormat>()
                        .map(|img| img.into_raw())
                        .map_err(|e| e.to_string()),
                };
                match decoded {
                    Ok(pixels) if pixels.len() == width * height * 4 => {
                        if !self.first_frame_ok {
                            self.first_frame_ok = true;
                            Cameras::note_capture_ok(self.index);
                        }
                        self.data = pixels;
                        crate::would_block_if_equal(&mut self.last_data, &self.data)?;
                        Ok(Frame::PixelBuffer(PixelBuffer::new(
                            &self.data,
                            Pixfmt::RGBA,
                            width,
                            height,
                        )))
                    }
                    Ok(pixels) => {
                        let reason = format!(
                            "Camera frame decode error: {fmt:?} frame decoded to {} bytes, expected {} for {width}x{height}",
                            pixels.len(),
                            width * height * 4
                        );
                        Cameras::note_capture_failure(self.index, &reason);
                        Err(io::Error::new(io::ErrorKind::Other, reason))
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
