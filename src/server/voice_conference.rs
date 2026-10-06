//! Audio conferencing for concurrent voice calls to the same remote (fork-only, phase 2).
//!
//! Every connection with an accepted voice call is a *member*. For each frame the remote's own
//! mic capture produces (`audio_service::send_f32`), this module:
//!
//! 1. takes exactly one frame's worth of each member's decoded mic audio out of that member's
//!    ring buffer (zero-padded if it hasn't arrived yet),
//! 2. plays the sum of every member's mic on the remote's speaker, through ONE playback - the
//!    same proven `start_audio_thread` path upstream uses per connection, just a single shared
//!    instance fed PCM directly (`MediaData::AudioPcm`, no encode/decode round trip) - so this
//!    never depends on N concurrent output streams coexisting on the device,
//! 3. once there are at least two members, sends each one a personalized stream: the remote's
//!    mic plus every *other* member's mic, never its own, so nobody hears themselves echoed
//!    back - delivered with the service's own `send_to`. A lone member instead gets the plain
//!    capture-encoded frame (its mix would be the remote's mic alone), sent to it only.
//!
//! While any call is active the capture *is* the microphone, so nobody outside the call
//! receives audio at all - there is no computer audio to give them, and the mic isn't theirs
//! to hear (stock RustDesk keeps broadcasting it to non-call Desktop viewers; deliberately not
//! kept). Ordinary system-sound sharing resumes when the last call ends.
//!
//! Everything is driven by the real capture clock and mixed/encoded in the capture's exact
//! format, so frame sizes are always ones the encoder accepts (the capture encoder already
//! accepts them), there is no separate timer thread, no output resampling, and nobody is ever
//! unsubscribed/resubscribed. A caller's mic audio is decoded (`on_local_frame`), converted to
//! the capture format once, and queued in a bounded ring buffer that absorbs network jitter -
//! the same idea as `client.rs`'s `AudioBuffer`.
//!
//! With no members, nothing here runs: `audio_service`'s broadcast behaves exactly as upstream.

use super::*;
use crate::client::{start_audio_thread, MediaData, MediaSender};
use magnum_opus::{
    Application::LowDelay, Channels, Decoder as AudioDecoder, Encoder as AudioEncoder,
};
use std::collections::{HashMap, HashSet, VecDeque};

/// Upper bound on how much of a caller's mic audio may wait for the next capture frame. Bounds
/// added latency; the oldest samples are dropped past this.
const MAX_BUFFERED_MS: usize = 200;

struct Member {
    decoder: Option<AudioDecoder>,
    decoder_rate: u32,
    decoder_channels: u16,
    decode_scratch: Vec<f32>,
    /// Decoded mic PCM, already in the remote's capture format, waiting to be mixed.
    ring: VecDeque<f32>,
    /// Encoder for this member's personalized stream, at the capture format.
    encoder: Option<AudioEncoder>,
}

struct Speaker {
    sender: MediaSender,
}

#[derive(Default)]
struct State {
    members: HashMap<i32, Member>,
    /// (sample_rate, channels) of the remote's own capture, as last seen in `on_capture_frame` -
    /// the one format everything is mixed and encoded in.
    capture_format: Option<(u32, u16)>,
    /// Single playback on the remote's speaker for the sum of every member's mic.
    speaker: Option<Speaker>,
}

lazy_static::lazy_static! {
    static ref STATE: Mutex<State> = Default::default();
}

fn opus_channels(channels: u16) -> Channels {
    if channels > 1 {
        Channels::Stereo
    } else {
        Channels::Mono
    }
}

fn add_into(dst: &mut [f32], src: &[f32]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d += *s;
    }
}

fn clamp(v: &mut [f32]) {
    for s in v.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }
}

fn new_speaker(sample_rate: u32, channels: u16) -> Option<Speaker> {
    let sender = start_audio_thread();
    let format = AudioFormat {
        sample_rate,
        channels: channels as _,
        ..Default::default()
    };
    allow_err!(sender.send(MediaData::AudioFormat(format)));
    Some(Speaker { sender })
}

/// Called once a connection's voice call is accepted, whatever its connection type.
pub fn register(conn_id: i32) {
    let mut st = STATE.lock().unwrap();
    st.members.entry(conn_id).or_insert_with(|| Member {
        decoder: None,
        decoder_rate: 0,
        decoder_channels: 0,
        decode_scratch: Vec::new(),
        ring: VecDeque::new(),
        encoder: None,
    });
}

/// Called when a connection's voice call closes or the connection tears down. Returns `true` if
/// no connection is in a voice call anymore - the caller uses this to decide whether it's safe to
/// reset the remote's mic-capture device back to normal PC audio. A no-op if never registered.
pub fn unregister(conn_id: i32) -> bool {
    let mut st = STATE.lock().unwrap();
    st.members.remove(&conn_id);
    let empty = st.members.is_empty();
    if empty {
        // Dropping the sender ends the playback thread and releases the output device.
        st.speaker = None;
    }
    empty
}

/// Called whenever a connection's negotiated mic-upload `AudioFormat` arrives. Returns `true`
/// if the connection is a member - the caller must then NOT open its own per-connection
/// playback for it (the conference's shared playback covers it, and an idle extra output
/// stream is exactly the concurrent-stream dependence this design avoids).
pub fn on_local_format(conn_id: i32, sample_rate: u32, channels: u16) -> bool {
    let mut st = STATE.lock().unwrap();
    let Some(m) = st.members.get_mut(&conn_id) else {
        return false;
    };
    match AudioDecoder::new(sample_rate, opus_channels(channels)) {
        Ok(d) => {
            m.decoder = Some(d);
            m.decoder_rate = sample_rate;
            m.decoder_channels = channels;
            m.decode_scratch = vec![0.; sample_rate as usize * channels as usize];
            m.ring.clear();
        }
        Err(e) => {
            log::error!("voice_conference: failed to create decoder for conn {conn_id}: {e}");
        }
    }
    true
}

/// Called on every inbound `AudioFrame` from a connection's mic upload. Returns `true` if the
/// frame was taken by the conference (the connection is a member) - the caller must then NOT
/// also play it through its own per-connection playback, or it would be heard twice on the
/// remote's speaker. Returns `false` for non-members, leaving upstream behavior untouched.
pub fn on_local_frame(conn_id: i32, frame: &AudioFrame) -> bool {
    let mut st = STATE.lock().unwrap();
    let capture = st.capture_format;
    let Some(m) = st.members.get_mut(&conn_id) else {
        return false;
    };
    let Some(d) = m.decoder.as_mut() else {
        return true;
    };
    let Ok(n) = d.decode_float(&frame.data, &mut m.decode_scratch, false) else {
        return true;
    };
    let samples = n * m.decoder_channels as usize;
    let mut pcm = m.decode_scratch[..samples].to_vec();
    let (rate, channels) = capture.unwrap_or((m.decoder_rate, m.decoder_channels));
    if m.decoder_rate != rate {
        pcm = crate::common::audio_resample(&pcm, m.decoder_rate, rate, m.decoder_channels);
    }
    if m.decoder_channels != channels {
        pcm = crate::common::audio_rechannel(pcm, rate, rate, m.decoder_channels, channels);
    }
    m.ring.extend(pcm);
    let max = rate as usize * channels as usize * MAX_BUFFERED_MS / 1000;
    if m.ring.len() > max {
        let excess = m.ring.len() - max;
        m.ring.drain(..excess);
    }
    true
}

/// Called from `audio_service::send_f32` with every frame of the remote's own capture, before
/// its zero gate (callers must keep hearing each other while the remote's mic is silent).
/// `data` is already in (`sample_rate`, `channels`), the capture encoder's format; it may hold
/// several 10ms frames (Android batches).
///
/// Returns how the caller must route its plain (capture-encoded) frame:
/// - `None`: no call is active - broadcast to every subscriber, exactly as upstream (this is
///   ordinary system-sound sharing for Desktop viewers).
/// - `Some(ids)`: a call is active, so the capture is the microphone and it belongs to the
///   call. Send the plain frame **only** to `ids` (members that did not get a personalized
///   frame - in practice a lone caller) and, if `ids` is empty, don't even encode it. Nobody
///   outside the call receives anything until the last call ends. Stock RustDesk keeps
///   broadcasting the microphone to non-call Desktop viewers for the duration of a call -
///   verified on a stock build; deliberately not kept, it's a privacy breach.
pub fn on_capture_frame(
    data: &[f32],
    sample_rate: u32,
    channels: u16,
    sp: &GenericService,
) -> Option<HashSet<i32>> {
    let mut served = HashSet::new();
    let mut st = STATE.lock().unwrap();
    if st.capture_format != Some((sample_rate, channels)) {
        // Capture (re)started in a new format: everything queued/encoded so far is in the old one.
        st.capture_format = Some((sample_rate, channels));
        for m in st.members.values_mut() {
            m.ring.clear();
            m.encoder = None;
        }
        st.speaker = None;
    }
    if st.members.is_empty() {
        return None;
    }
    let frame_len = (sample_rate as usize / 100) * channels as usize; // 10ms
    if frame_len == 0 {
        return Some(HashSet::new());
    }
    let st = &mut *st;
    for frame in data.chunks_exact(frame_len) {
        // 1. Exactly one frame's worth from every member, zero-padded if it hasn't arrived yet.
        let mut chunks: Vec<(i32, Vec<f32>)> = Vec::with_capacity(st.members.len());
        for (id, m) in st.members.iter_mut() {
            let take = frame_len.min(m.ring.len());
            let mut chunk: Vec<f32> = m.ring.drain(..take).collect();
            chunk.resize(frame_len, 0.0);
            chunks.push((*id, chunk));
        }

        // 2. Remote's speaker: every member summed, through one shared playback.
        let mut sum = vec![0.0f32; frame_len];
        for (_, chunk) in &chunks {
            add_into(&mut sum, chunk);
        }
        clamp(&mut sum);
        if st.speaker.is_none() {
            st.speaker = new_speaker(sample_rate, channels);
        }
        if let Some(speaker) = st.speaker.as_mut() {
            // PCM straight into the playback thread - no encode/decode round trip.
            allow_err!(speaker.sender.send(MediaData::AudioPcm(sum)));
        }

        // 3. Each member: remote's mic plus every OTHER member, never its own. Only worth a
        // per-member encode once there are at least two members - a lone caller's mix would
        // be the remote's mic alone, which is exactly what the plain broadcast already carries
        // (one capture encode for everyone, as upstream), so leave it on that.
        if st.members.len() < 2 {
            continue;
        }
        for (id, m) in st.members.iter_mut() {
            let mut mix = frame.to_vec();
            for (other, chunk) in &chunks {
                if other != id {
                    add_into(&mut mix, chunk);
                }
            }
            clamp(&mut mix);
            if m.encoder.is_none() {
                m.encoder = match AudioEncoder::new(sample_rate, opus_channels(channels), LowDelay)
                {
                    Ok(e) => Some(e),
                    Err(e) => {
                        log::error!("voice_conference: failed to create encoder for conn {id}: {e}");
                        None
                    }
                };
            }
            let Some(encoder) = m.encoder.as_mut() else {
                continue;
            };
            match encoder.encode_vec_float(&mix, mix.len() * 6) {
                Ok(encoded) => {
                    let mut msg = Message::new();
                    msg.set_audio_frame(AudioFrame {
                        data: encoded.into(),
                        ..Default::default()
                    });
                    sp.send_to(msg, *id);
                    served.insert(*id);
                }
                Err(e) => log::warn!("voice_conference: encode for conn {id} failed: {e}"),
            }
        }
    }
    // Members that got no personalized frame (a lone caller) still need the plain one; nobody
    // outside the call gets anything while a call is active.
    let plain: HashSet<i32> = st
        .members
        .keys()
        .filter(|id| !served.contains(id))
        .copied()
        .collect();
    Some(plain)
}
