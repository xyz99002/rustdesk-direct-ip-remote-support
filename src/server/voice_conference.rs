//! Audio conferencing for concurrent voice calls to the same remote (fork-only, phase 2).
//!
//! When only one connection has an active voice call, this module does nothing (fast path) -
//! audio behaves exactly as upstream: the remote's own mic broadcasts via `audio_service`'s
//! singleton `GenericService` to that one subscriber, and that connection's mic plays on the
//! remote's speaker via its own independent decode/output pipeline in `src/client.rs`'s
//! `start_audio_thread`, completely untouched by this module.
//!
//! Once a *second* different connection also has an active voice call, every in-call connection
//! is unsubscribed from `audio_service`'s plain broadcast (see [`set_broadcast_subscription`]) and
//! instead gets a *personalized* mix built here: the remote's own mic plus every *other* in-call
//! connection's mic, excluding its own - so nobody hears themselves echoed back, and multiple
//! callers can hear each other. The mix is re-encoded as an ordinary `AudioFrame` on that
//! recipient's existing connection, indistinguishable from today's message format - no client-side
//! (Flutter) changes are needed at all.
//!
//! Mixing happens at a fixed internal format ([`MIX_SAMPLE_RATE`]/[`MIX_CHANNELS`]) regardless of
//! what rate each local negotiated for its own mic upload, using the existing
//! `crate::common::audio_resample`/`audio_rechannel` helpers already used elsewhere in the audio
//! pipeline. The final per-recipient encode step resamples back down to whatever format
//! `audio_service`'s own broadcast last announced (tracked via [`on_remote_mic_frame`]), so a
//! recipient's client never needs to know anything changed.

use super::*;
use magnum_opus::{Channels, Decoder as AudioDecoder, Encoder as AudioEncoder};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const MIX_SAMPLE_RATE: u32 = 48000;
const MIX_CHANNELS: u16 = 2;
const FRAME_MS: u64 = 10;
// A source (remote mic, or a given local's mic) whose last update is older than this is treated
// as silence in the mix rather than repeating stale audio - handles a caller going quiet/a
// connection lagging without any new jitter-buffer logic beyond this staleness check.
const STALE_AFTER: Duration = Duration::from_millis(100);

#[derive(Default, Clone)]
struct MixSource {
    /// Already normalized to MIX_SAMPLE_RATE/MIX_CHANNELS.
    pcm: Vec<f32>,
    updated: Option<Instant>,
}

impl MixSource {
    fn fresh_pcm(&self, now: Instant) -> Option<&[f32]> {
        match self.updated {
            Some(t) if now.duration_since(t) < STALE_AFTER && !self.pcm.is_empty() => {
                Some(&self.pcm)
            }
            _ => None,
        }
    }
}

struct ConferenceMember {
    inner: ConnInner,
    server: super::ServerPtrWeak,
    /// Whether this connection had audio enabled at the moment it joined the conference -
    /// restored (not force-enabled) when the conference ends.
    audio_enabled: bool,
    decoder: Option<AudioDecoder>,
    decoder_channels: u16,
    decode_scratch: Vec<f32>,
    encoder: Option<AudioEncoder>,
    encoder_format: (u32, u16),
    mix_source: MixSource,
}

lazy_static::lazy_static! {
    static ref MEMBERS: Mutex<HashMap<i32, ConferenceMember>> = Default::default();
    static ref REMOTE_MIC: Mutex<MixSource> = Default::default();
    // What audio_service's own broadcast last announced - the format every recipient's client
    // already expects, so the per-recipient re-encode targets this rather than a fixed rate.
    static ref BROADCAST_FORMAT: Mutex<(u32, u16)> = Mutex::new((MIX_SAMPLE_RATE, MIX_CHANNELS));
}

static TICKER_STARTED: AtomicBool = AtomicBool::new(false);

fn opus_channels(channels: u16) -> Channels {
    if channels > 1 {
        Channels::Stereo
    } else {
        Channels::Mono
    }
}

fn ensure_ticker_started() {
    if TICKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_millis(FRAME_MS));
        tick();
    });
}

fn set_broadcast_subscription(m: &ConferenceMember, enabled: bool) {
    if let Some(s) = m.server.upgrade() {
        s.write()
            .unwrap()
            .subscribe(super::audio_service::NAME, m.inner.clone(), enabled);
    }
}

/// Called once a connection's voice call is accepted. Starts the background mixing ticker (if
/// not already running) and registers this connection as a conference member. If this is the
/// second concurrently in-call connection, every member (including this one) is unsubscribed from
/// `audio_service`'s plain broadcast - from this point on they only receive this module's
/// personalized mixes, never both (which would double up the remote's own mic audio).
pub fn register(conn_id: i32, inner: ConnInner, server: super::ServerPtrWeak, audio_enabled: bool) {
    ensure_ticker_started();
    let mut members = MEMBERS.lock().unwrap();
    members.insert(
        conn_id,
        ConferenceMember {
            inner,
            server,
            audio_enabled,
            decoder: None,
            decoder_channels: MIX_CHANNELS,
            decode_scratch: Vec::new(),
            encoder: None,
            encoder_format: *BROADCAST_FORMAT.lock().unwrap(),
            mix_source: MixSource::default(),
        },
    );
    if members.len() == 2 {
        for m in members.values() {
            set_broadcast_subscription(m, false);
        }
    }
}

/// Called when a connection's voice call closes (or the connection itself tears down). Returns
/// `true` if no connection is in a voice call anymore (the caller uses this to decide whether
/// it's safe to reset the remote's mic-capture device back to normal PC audio - resetting it
/// while another connection is still mid-call would kill that connection's audio too). If this
/// was the conference's second-to-last member, every remaining member is resubscribed to
/// `audio_service`'s plain broadcast, restoring normal single-call behavior.
pub fn unregister(conn_id: i32) -> bool {
    let mut members = MEMBERS.lock().unwrap();
    let was_conferencing = members.len() >= 2;
    members.remove(&conn_id);
    if was_conferencing && members.len() < 2 {
        for m in members.values() {
            set_broadcast_subscription(m, m.audio_enabled);
        }
    }
    members.is_empty()
}

/// Called whenever a connection's negotiated mic-upload `AudioFormat` arrives (same moment
/// upstream's own `start_audio_thread` is (re)created). (Re)creates this member's decoder (native
/// format) and encoder (targeting whatever `audio_service`'s broadcast currently uses).
pub fn on_local_format(conn_id: i32, sample_rate: u32, channels: u16) {
    let mut members = MEMBERS.lock().unwrap();
    let Some(m) = members.get_mut(&conn_id) else {
        return;
    };
    match AudioDecoder::new(sample_rate, opus_channels(channels)) {
        Ok(d) => {
            m.decoder = Some(d);
            m.decoder_channels = channels;
            m.decode_scratch = vec![0.; sample_rate as usize * channels as usize];
        }
        Err(e) => {
            log::error!("voice_conference: failed to create decoder for conn {conn_id}: {e}");
        }
    }
    let (out_rate, out_channels) = *BROADCAST_FORMAT.lock().unwrap();
    match AudioEncoder::new(out_rate, opus_channels(out_channels), magnum_opus::Application::LowDelay) {
        Ok(e) => {
            m.encoder = Some(e);
            m.encoder_format = (out_rate, out_channels);
        }
        Err(e) => {
            log::error!("voice_conference: failed to create encoder for conn {conn_id}: {e}");
        }
    }
}

/// Called on every inbound `AudioFrame` from a connection's mic upload. No-ops unless a
/// conference is actually in progress (fast path - avoids the decode/resample cost entirely for
/// an ordinary single-caller session).
pub fn on_local_frame(conn_id: i32, frame: &AudioFrame) {
    let mut members = MEMBERS.lock().unwrap();
    if members.len() < 2 {
        return;
    }
    let Some(m) = members.get_mut(&conn_id) else {
        return;
    };
    let Some(d) = m.decoder.as_mut() else {
        return;
    };
    let Ok(n) = d.decode_float(&frame.data, &mut m.decode_scratch, false) else {
        return;
    };
    let samples = n * m.decoder_channels as usize;
    let mut pcm = m.decode_scratch[..samples].to_vec();
    if let Some(rate) = decoder_sample_rate(m) {
        if rate != MIX_SAMPLE_RATE {
            pcm = crate::common::audio_resample(&pcm, rate, MIX_SAMPLE_RATE, m.decoder_channels);
        }
    }
    if m.decoder_channels != MIX_CHANNELS {
        pcm = crate::common::audio_rechannel(
            pcm,
            MIX_SAMPLE_RATE,
            MIX_SAMPLE_RATE,
            m.decoder_channels,
            MIX_CHANNELS,
        );
    }
    m.mix_source.pcm = pcm;
    m.mix_source.updated = Some(Instant::now());
}

// The decoder itself doesn't expose its construction sample rate, so it's recovered from the
// scratch buffer sizing convention used in `on_local_format` (`sample_rate * channels`).
fn decoder_sample_rate(m: &ConferenceMember) -> Option<u32> {
    if m.decoder_channels == 0 {
        return None;
    }
    Some(m.decode_scratch.len() as u32 / m.decoder_channels as u32)
}

/// Called from `audio_service`'s capture path with the remote's own latest mic PCM, already at
/// `sample_rate`/`channels` (whatever `audio_service` is currently encoding at). Always updates
/// the tracked broadcast format (used to pick new members' encoder format), but only bothers
/// normalizing/storing the PCM itself when a conference is actually in progress.
pub fn on_remote_mic_frame(pcm: &[f32], sample_rate: u32, channels: u16) {
    *BROADCAST_FORMAT.lock().unwrap() = (sample_rate, channels);
    if MEMBERS.lock().unwrap().len() < 2 {
        return;
    }
    let mut data = pcm.to_vec();
    if sample_rate != MIX_SAMPLE_RATE {
        data = crate::common::audio_resample(&data, sample_rate, MIX_SAMPLE_RATE, channels);
    }
    if channels != MIX_CHANNELS {
        data = crate::common::audio_rechannel(
            data,
            MIX_SAMPLE_RATE,
            MIX_SAMPLE_RATE,
            channels,
            MIX_CHANNELS,
        );
    }
    let mut remote_mic = REMOTE_MIC.lock().unwrap();
    remote_mic.pcm = data;
    remote_mic.updated = Some(Instant::now());
}

fn add_into(dst: &mut [f32], src: &[f32]) {
    let n = dst.len().min(src.len());
    for i in 0..n {
        dst[i] += src[i];
    }
}

fn tick() {
    let mut members = MEMBERS.lock().unwrap();
    if members.len() < 2 {
        return;
    }
    let now = Instant::now();
    let remote_mic = REMOTE_MIC.lock().unwrap();
    let remote_pcm = remote_mic.fresh_pcm(now).map(|p| p.to_vec());
    drop(remote_mic);

    let contributions: Vec<(i32, Option<Vec<f32>>)> = members
        .iter()
        .map(|(id, m)| (*id, m.mix_source.fresh_pcm(now).map(|p| p.to_vec())))
        .collect();

    let frame_len = (MIX_SAMPLE_RATE as usize / (1000 / FRAME_MS as usize)) * MIX_CHANNELS as usize;

    for (id, member) in members.iter_mut() {
        let mut mix = vec![0.0f32; frame_len];
        if let Some(pcm) = &remote_pcm {
            add_into(&mut mix, pcm);
        }
        for (other_id, pcm) in &contributions {
            if other_id != id {
                if let Some(pcm) = pcm {
                    add_into(&mut mix, pcm);
                }
            }
        }
        for s in mix.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }

        let (out_rate, out_channels) = member.encoder_format;
        let mut out_pcm = mix;
        if out_rate != MIX_SAMPLE_RATE {
            out_pcm = crate::common::audio_resample(&out_pcm, MIX_SAMPLE_RATE, out_rate, MIX_CHANNELS);
        }
        if out_channels != MIX_CHANNELS {
            out_pcm = crate::common::audio_rechannel(
                out_pcm,
                out_rate,
                out_rate,
                MIX_CHANNELS,
                out_channels,
            );
        }

        let Some(encoder) = member.encoder.as_mut() else {
            continue;
        };
        if let Ok(encoded) = encoder.encode_vec_float(&out_pcm, out_pcm.len() * 6) {
            let mut msg_out = Message::new();
            msg_out.set_audio_frame(AudioFrame {
                data: encoded.into(),
                ..Default::default()
            });
            member.inner.send(Arc::new(msg_out));
        }
    }
}
