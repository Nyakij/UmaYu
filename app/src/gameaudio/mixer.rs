//! Audio playback engine and mixer for game audio.
//!
//! Provides HCA/AWB/ACB decoding, software mixing with volume/fade/looping,
//! lock-free SPSC ring buffering, and real-time WASAPI output via `cpal`.

#![allow(dead_code)]

use std::cell::UnsafeCell;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cridecoder::acb::AfsArchive;
use cridecoder::hca::HcaDecoder;

/// Cygames CriWare HCA global encryption key (`75923756697503` / `0x450F27812E7F`).
pub const CRIWARE_HCA_KEY: u64 = 75923756697503;

/// Lock-free Single-Producer Single-Consumer (SPSC) ring buffer.
pub struct SpscRingBuffer {
    buffer: Box<[UnsafeCell<f32>]>,
    capacity: usize,
    mask: usize,
    read_pos: AtomicUsize,
    write_pos: AtomicUsize,
}

unsafe impl Send for SpscRingBuffer {}
unsafe impl Sync for SpscRingBuffer {}

impl SpscRingBuffer {
    /// Creates a new SPSC ring buffer with capacity rounded up to power of two.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two().max(1024);
        let mut vec = Vec::with_capacity(cap);
        for _ in 0..cap {
            vec.push(UnsafeCell::new(0.0));
        }
        Self {
            buffer: vec.into_boxed_slice(),
            capacity: cap,
            mask: cap - 1,
            read_pos: AtomicUsize::new(0),
            write_pos: AtomicUsize::new(0),
        }
    }

    /// Writes audio samples into the buffer. Returns the number of samples written.
    pub fn write(&self, data: &[f32]) -> usize {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Acquire);
        let available = self.capacity.saturating_sub(write.wrapping_sub(read));
        let count = data.len().min(available);
        if count == 0 {
            return 0;
        }

        for (i, &sample) in data[..count].iter().enumerate() {
            let idx = (write + i) & self.mask;
            unsafe {
                *self.buffer[idx].get() = sample;
            }
        }

        self.write_pos
            .store(write.wrapping_add(count), Ordering::Release);
        count
    }

    /// Reads audio samples from the buffer into the output slice.
    pub fn read(&self, out: &mut [f32]) -> usize {
        let read = self.read_pos.load(Ordering::Relaxed);
        let write = self.write_pos.load(Ordering::Acquire);
        let available = write.wrapping_sub(read);
        let count = out.len().min(available);

        for (i, item) in out[..count].iter_mut().enumerate() {
            let idx = (read + i) & self.mask;
            *item = unsafe { *self.buffer[idx].get() };
        }

        self.read_pos
            .store(read.wrapping_add(count), Ordering::Release);
        count
    }

    /// Returns the number of available samples ready to be read.
    pub fn available_read(&self) -> usize {
        let read = self.read_pos.load(Ordering::Relaxed);
        let write = self.write_pos.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Clears all buffered samples.
    pub fn clear(&self) {
        let write = self.write_pos.load(Ordering::Acquire);
        self.read_pos.store(write, Ordering::Release);
    }
}

/// Decoded uncompressed audio buffer with sample metadata.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Title or cue name of the audio track.
    pub title: String,
    /// Sampling rate in Hz (e.g. 44100, 48000).
    pub sample_rate: u32,
    /// Number of channels (1 = Mono, 2 = Stereo).
    pub channels: u16,
    /// Interleaved PCM 32-bit floating point samples normalized to [-1.0, 1.0].
    pub samples: Vec<f32>,
    /// Loop start index in sample points (interleaved offset), if enabled.
    pub loop_start: Option<usize>,
    /// Loop end index in sample points (interleaved offset), if enabled.
    pub loop_end: Option<usize>,
    /// Track duration in seconds.
    pub duration_secs: f32,
}

impl DecodedAudio {
    /// Resamples the audio to target sample rate using linear interpolation if needed.
    pub fn resample_to(&mut self, target_rate: u32) {
        if self.sample_rate == target_rate || self.sample_rate == 0 || target_rate == 0 {
            return;
        }

        let channels = self.channels as usize;
        let in_rate = self.sample_rate as f64;
        let out_rate = target_rate as f64;
        let in_frames = self.samples.len() / channels;
        let out_frames = ((in_frames as f64) * out_rate / in_rate).round() as usize;

        let mut resampled = Vec::with_capacity(out_frames * channels);
        for out_idx in 0..out_frames {
            let t = (out_idx as f64) * in_rate / out_rate;
            let idx0 = (t.floor() as usize).min(in_frames.saturating_sub(1));
            let idx1 = (idx0 + 1).min(in_frames.saturating_sub(1));
            let frac = (t - idx0 as f64) as f32;

            for ch in 0..channels {
                let s0 = self.samples[idx0 * channels + ch];
                let s1 = self.samples[idx1 * channels + ch];
                resampled.push(s0 + frac * (s1 - s0));
            }
        }

        // Adjust loop points proportionally
        let scale = out_rate / in_rate;
        self.loop_start = self.loop_start.map(|s| {
            let frame = (s / channels) as f64 * scale;
            (frame.round() as usize) * channels
        });
        self.loop_end = self.loop_end.map(|s| {
            let frame = (s / channels) as f64 * scale;
            (frame.round() as usize) * channels
        });

        self.samples = resampled;
        self.sample_rate = target_rate;
    }

    /// Converts mono track to stereo by duplicating channels if needed.
    pub fn ensure_stereo(&mut self) {
        if self.channels == 1 {
            let mut stereo = Vec::with_capacity(self.samples.len() * 2);
            for &sample in &self.samples {
                stereo.push(sample);
                stereo.push(sample);
            }
            self.samples = stereo;
            self.channels = 2;
            self.loop_start = self.loop_start.map(|s| s * 2);
            self.loop_end = self.loop_end.map(|s| s * 2);
        }
    }
}

/// Decodes an HCA stream from raw bytes using the specified subkey.
pub fn decode_hca_bytes(
    hca_bytes: &[u8],
    subkey: u16,
    title: Option<&str>,
) -> Result<DecodedAudio, String> {
    let cursor = Cursor::new(hca_bytes);
    let mut decoder = HcaDecoder::from_reader(cursor)
        .map_err(|e| format!("Failed to create HCA decoder: {e}"))?;

    decoder.set_encryption_key(CRIWARE_HCA_KEY, subkey as u64);

    let info = decoder.info().clone();
    let samples = decoder
        .decode_all()
        .map_err(|e| format!("Failed to decode HCA audio: {e}"))?;

    let channels = info.channel_count as u16;
    let sample_rate = info.sampling_rate;
    let total_frames = if channels > 0 {
        samples.len() / (channels as usize)
    } else {
        0
    };
    let duration_secs = if sample_rate > 0 {
        total_frames as f32 / sample_rate as f32
    } else {
        0.0
    };

    let (loop_start, loop_end) = if info.loop_enabled {
        let ch = channels as usize;
        let start_sample = info.loop_start_block as usize * info.samples_per_block * ch;
        let end_sample = if info.loop_end_block > 0 {
            (info.loop_end_block as usize * info.samples_per_block * ch).min(samples.len())
        } else {
            samples.len()
        };
        (Some(start_sample), Some(end_sample))
    } else {
        (None, None)
    };

    let title_str = title.unwrap_or("Track").to_string();

    Ok(DecodedAudio {
        title: title_str,
        sample_rate,
        channels,
        samples,
        loop_start,
        loop_end,
        duration_secs,
    })
}

/// Decodes a wave from a standalone AWB (AFS2 container) file.
pub fn decode_awb_file(awb_path: &Path, cue_index: usize) -> Result<DecodedAudio, String> {
    let file = File::open(awb_path).map_err(|e| format!("Cannot open AWB file: {e}"))?;
    let mut archive = AfsArchive::new(file).map_err(|e| format!("Invalid AFS2 archive: {e}"))?;

    let subkey = archive.subkey;
    if cue_index >= archive.files.len() {
        return Err(format!(
            "Cue index {} out of bounds (total {})",
            cue_index,
            archive.files.len()
        ));
    }

    let entry = archive.files[cue_index];
    let hca_data = archive
        .file_data(entry)
        .map_err(|e| format!("Failed to extract HCA from AWB: {e}"))?;

    let filename = awb_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    let title = format!("{filename} #{cue_index}");

    decode_hca_bytes(&hca_data, subkey, Some(&title))
}

/// Decodes a wave from an ACB file (using either embedded or external AWB).
pub fn decode_acb_file(acb_path: &Path, cue_index: usize) -> Result<DecodedAudio, String> {
    let file = File::open(acb_path).map_err(|e| format!("Cannot open ACB file: {e}"))?;
    let tracks = cridecoder::acb::extract_acb_to_memory(file, Some(acb_path))
        .map_err(|e| format!("Failed to parse ACB tracks: {e}"))?;

    if tracks.is_empty() {
        return Err("No tracks found in ACB file".to_string());
    }

    let track = if cue_index < tracks.len() {
        &tracks[cue_index]
    } else {
        &tracks[0]
    };

    decode_hca_bytes(&track.data, track.subkey, Some(&track.name))
}

/// Decodes an audio file by inspecting its header magic (AFS2, @UTF, or raw HCA).
pub fn decode_audio_file(file_path: &Path, cue_index: usize) -> Result<DecodedAudio, String> {
    let mut header = [0u8; 4];
    {
        let mut f = File::open(file_path).map_err(|e| format!("Cannot open file: {e}"))?;
        f.read_exact(&mut header)
            .map_err(|e| format!("Failed to read header: {e}"))?;
    }

    match &header {
        b"AFS2" => decode_awb_file(file_path, cue_index),
        b"@UTF" => decode_acb_file(file_path, cue_index),
        b"HCA\0" => {
            let mut data = Vec::new();
            let mut f = File::open(file_path).map_err(|e| format!("Cannot open HCA file: {e}"))?;
            f.read_to_end(&mut data)
                .map_err(|e| format!("Failed to read HCA: {e}"))?;
            decode_hca_bytes(&data, 0, file_path.file_stem().and_then(|s| s.to_str()))
        }
        _ => Err(format!(
            "Unsupported audio container magic: {:?}",
            String::from_utf8_lossy(&header)
        )),
    }
}

/// Information about a single sound track / wave cue inside an audio container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueInfo {
    pub index: usize,
    pub name: String,
    pub extension: String,
    pub subkey: u16,
    pub size_bytes: usize,
}

/// Prepares an ACB file and its companion AWB (if present) in a cached working folder
/// so that `cridecoder`'s internal external-AWB resolver can locate the `.awb` file beside the `.acb`.
pub fn prepare_acb_file(
    game_dir: &Path,
    acb_entry: &crate::gameaudio::meta::ManifestEntry,
    awb_entry: Option<&crate::gameaudio::meta::ManifestEntry>,
) -> Result<PathBuf, String> {
    let cache_dir = std::env::var_os("APPDATA")
        .map(|d| {
            PathBuf::from(d)
                .join("UmaYu")
                .join("cache")
                .join("acb_prep")
        })
        .unwrap_or_else(|| std::env::temp_dir().join("umayu_acb_prep"));

    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create ACB preparation directory: {e}"))?;

    let base_name = Path::new(&acb_entry.name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");

    let prep_acb = cache_dir.join(format!("{base_name}.acb"));
    let src_acb = acb_entry.file_path(game_dir);

    if !src_acb.exists() {
        return Err(format!(
            "Source ACB file does not exist: {}",
            src_acb.display()
        ));
    }

    if !prep_acb.exists() {
        if std::fs::hard_link(&src_acb, &prep_acb).is_err() {
            let _ = std::fs::copy(&src_acb, &prep_acb);
        }
    }

    if let Some(awb) = awb_entry {
        let src_awb = awb.file_path(game_dir);
        let prep_awb = cache_dir.join(format!("{base_name}.awb"));
        if src_awb.exists() && !prep_awb.exists() {
            if std::fs::hard_link(&src_awb, &prep_awb).is_err() {
                let _ = std::fs::copy(&src_awb, &prep_awb);
            }
        }
    }

    Ok(prep_acb)
}

/// Inspects an audio file and returns the list of cues / tracks inside it.
pub fn inspect_audio_file(file_path: &Path) -> Result<Vec<CueInfo>, String> {
    let mut header = [0u8; 4];
    {
        let mut f = File::open(file_path).map_err(|e| format!("Cannot open file: {e}"))?;
        f.read_exact(&mut header)
            .map_err(|e| format!("Failed to read header: {e}"))?;
    }

    match &header {
        b"@UTF" => {
            let file = File::open(file_path).map_err(|e| format!("Cannot open ACB file: {e}"))?;
            let tracks = cridecoder::acb::extract_acb_to_memory(file, Some(file_path))
                .map_err(|e| format!("Failed to parse ACB tracks: {e}"))?;
            let cues = tracks
                .into_iter()
                .enumerate()
                .map(|(idx, t)| CueInfo {
                    index: idx,
                    name: if t.name.is_empty() {
                        format!("Track #{idx}")
                    } else {
                        t.name
                    },
                    extension: t.extension,
                    subkey: t.subkey,
                    size_bytes: t.data.len(),
                })
                .collect();
            Ok(cues)
        }
        b"AFS2" => {
            let file = File::open(file_path).map_err(|e| format!("Cannot open AWB file: {e}"))?;
            let archive =
                AfsArchive::new(file).map_err(|e| format!("Invalid AFS2 archive: {e}"))?;
            let stem = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("track");
            let cues = (0..archive.files.len())
                .map(|idx| {
                    let entry = archive.files[idx];
                    CueInfo {
                        index: idx,
                        name: format!("{stem} #{idx}"),
                        extension: "hca".to_string(),
                        subkey: archive.subkey,
                        size_bytes: entry.size as usize,
                    }
                })
                .collect();
            Ok(cues)
        }
        b"HCA\0" => {
            let size = std::fs::metadata(file_path)
                .map(|m| m.len() as usize)
                .unwrap_or(0);
            let name = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Track")
                .to_string();
            Ok(vec![CueInfo {
                index: 0,
                name,
                extension: "hca".to_string(),
                subkey: 0,
                size_bytes: size,
            }])
        }
        _ => Err(format!(
            "Unsupported audio container magic: {:?}",
            String::from_utf8_lossy(&header)
        )),
    }
}

/// Playback state of the audio engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Individual performer's singing mode in live playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SingingMode {
    /// Follows part triggers (0 = off, 1 = main, 2 = extra).
    Auto,
    /// Force singing main vocal (wave 0) regardless of trigger.
    ForceMain,
    /// Force singing extra vocal (wave 1) regardless of trigger.
    ForceExtra,
    /// Merge and sing both main and extra vocals.
    ForceMerge,
    /// Muted.
    Mute,
}

/// A performer channel in the live multi-track mixer.
#[derive(Debug, Clone)]
pub struct LivePerformerAudio {
    pub chara_id: i32,
    pub name: String,
    pub main_audio: Option<DecodedAudio>,
    pub extra_audio: Option<DecodedAudio>,
    pub mode: SingingMode,
    pub solo: bool,
    pub volume: f32,
    pub main_vocal_enabled: bool,
}

/// Real-time control update for a single performer, keyed by stage slot index.
///
/// Slot index (lineup order) is the stable identity: the same character may
/// stand in several slots, so `chara_id` cannot key these updates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformerControlUpdate {
    pub slot: usize,
    pub mode: SingingMode,
    pub solo: bool,
    pub volume: f32,
    pub main_vocal_enabled: bool,
}

/// Full multi-track live performance session.
#[derive(Debug, Clone)]
pub struct LivePerformance {
    pub music_id: i32,
    pub title: String,
    pub oke_audio: DecodedAudio,
    pub performers: Vec<LivePerformerAudio>,
    pub score: crate::gameaudio::unity::LiveScore,
    pub mute_oke: bool,
    pub mute_vocals: bool,
}

/// Control command sent from UI thread to mixer thread.
pub enum MixerCommand {
    LoadTrack {
        audio: DecodedAudio,
        autoplay: bool,
    },
    LoadLive {
        live: Box<LivePerformance>,
        autoplay: bool,
    },
    Play,
    Pause,
    Stop,
    Seek(f32),
    SetVolume(f32),
    SetLoop(bool),
    SetOutputRate(u32),
    UpdateLiveControls {
        mute_oke: bool,
        mute_vocals: bool,
        performer_controls: Vec<PerformerControlUpdate>,
    },
}

/// Shared status observables exposed for UI polling.
pub struct SharedAudioStatus {
    pub state: Mutex<PlaybackState>,
    pub position_bits: AtomicU32,
    pub duration_bits: AtomicU32,
    pub volume_bits: AtomicU32,
    pub track_title: Mutex<Option<String>>,
    pub loop_enabled: AtomicBool,
    pub track_ended: AtomicBool,
}

impl SharedAudioStatus {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(PlaybackState::Stopped),
            position_bits: AtomicU32::new(0.0f32.to_bits()),
            duration_bits: AtomicU32::new(0.0f32.to_bits()),
            volume_bits: AtomicU32::new(1.0f32.to_bits()),
            track_title: Mutex::new(None),
            loop_enabled: AtomicBool::new(false),
            track_ended: AtomicBool::new(false),
        }
    }

    pub fn state(&self) -> PlaybackState {
        *self.state.lock().unwrap()
    }

    pub fn position_secs(&self) -> f32 {
        f32::from_bits(self.position_bits.load(Ordering::Relaxed))
    }

    pub fn duration_secs(&self) -> f32 {
        f32::from_bits(self.duration_bits.load(Ordering::Relaxed))
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    pub fn track_title(&self) -> Option<String> {
        self.track_title.lock().unwrap().clone()
    }

    pub fn is_loop(&self) -> bool {
        self.loop_enabled.load(Ordering::Relaxed)
    }

    pub fn take_track_ended(&self) -> bool {
        self.track_ended.swap(false, Ordering::Relaxed)
    }
}

/// Returns names of all active audio output devices on the host system.
pub fn output_device_names() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                let trimmed = name.trim().to_string();
                if !trimmed.is_empty() && !names.contains(&trimmed) {
                    names.push(trimmed);
                }
            }
        }
    }
    names.sort();
    names
}

/// Finds an audio output device by name (case-insensitive), or None if not found or empty.
fn find_output_device_by_name(host: &cpal::Host, name: Option<&str>) -> Option<cpal::Device> {
    if let Some(target_name) = name {
        let trimmed = target_name.trim();
        if !trimmed.is_empty() {
            if let Ok(devices) = host.output_devices() {
                for d in devices {
                    if let Ok(d_name) = d.name() {
                        if d_name.eq_ignore_ascii_case(trimmed) {
                            return Some(d);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Builds and starts a stereo cpal output stream on the given device using the provided ring buffer.
fn build_device_stream(
    device: &cpal::Device,
    ring: Arc<SpscRingBuffer>,
) -> Result<(cpal::Stream, u32), String> {
    let default_config = device
        .default_output_config()
        .map_err(|e| format!("Failed to get default output config: {e}"))?;
    let sample_rate = default_config.sample_rate().0;
    let stream_config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let ring_consumer = ring;
    let stream = device
        .build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let read = ring_consumer.read(data);
                if read < data.len() {
                    data[read..].fill(0.0);
                }
            },
            |_err| {},
            None,
        )
        .map_err(|e| format!("Failed to build audio stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start audio stream: {e}"))?;

    Ok((stream, sample_rate))
}

/// Audio playback engine managing mixer thread and cpal WASAPI stream.
pub struct AudioEngine {
    tx_cmd: Sender<MixerCommand>,
    status: Arc<SharedAudioStatus>,
    ring: Arc<SpscRingBuffer>,
    output_sample_rate: u32,
    active_device: Option<String>,
    _cpal_stream: Option<cpal::Stream>,
}

impl AudioEngine {
    /// Initializes audio engine with specified device name (or default if None / not found).
    pub fn new_with_device(name: Option<&str>) -> Self {
        let status = Arc::new(SharedAudioStatus::new());
        let (tx_cmd, rx_cmd) = channel();
        let ring = Arc::new(SpscRingBuffer::new(65536));

        let host = cpal::default_host();
        let device_opt =
            find_output_device_by_name(&host, name).or_else(|| host.default_output_device());

        let mut cpal_stream = None;
        let mut output_sample_rate = 44100u32;
        let mut active_device = None;

        if let Some(device) = device_opt {
            active_device = device.name().ok();
            if let Ok((stream, sample_rate)) = build_device_stream(&device, ring.clone()) {
                cpal_stream = Some(stream);
                output_sample_rate = sample_rate;
            }
        }

        let status_clone = status.clone();
        let ring_producer = ring.clone();
        thread::spawn(move || {
            run_mixer_thread(rx_cmd, status_clone, ring_producer, output_sample_rate);
        });

        Self {
            tx_cmd,
            status,
            ring,
            output_sample_rate,
            active_device,
            _cpal_stream: cpal_stream,
        }
    }

    /// Initializes audio device with default output endpoint and mixer worker thread.
    pub fn new() -> Self {
        Self::new_with_device(None)
    }

    /// Switches audio output to another device by name, or system default if None or empty.
    pub fn swap_output_device(&mut self, name: Option<&str>) -> Result<(), String> {
        let host = cpal::default_host();
        let device = match name {
            Some(n) if !n.trim().is_empty() => find_output_device_by_name(&host, Some(n))
                .ok_or_else(|| format!("Device not found: {n}"))?,
            _ => host
                .default_output_device()
                .ok_or_else(|| "Default audio device not available".to_string())?,
        };

        let (new_stream, new_sample_rate) = build_device_stream(&device, self.ring.clone())?;
        self._cpal_stream = None;
        self._cpal_stream = Some(new_stream);
        self.active_device = device.name().ok();

        if new_sample_rate != self.output_sample_rate {
            self.output_sample_rate = new_sample_rate;
            let _ = self
                .tx_cmd
                .send(MixerCommand::SetOutputRate(new_sample_rate));
        }

        Ok(())
    }

    /// Returns the name of the currently active output device, if known.
    pub fn active_device(&self) -> Option<&str> {
        self.active_device.as_deref()
    }

    /// Returns shared audio status for UI polling.
    pub fn status(&self) -> Arc<SharedAudioStatus> {
        self.status.clone()
    }

    /// Loads and optionally starts playing a decoded audio track.
    pub fn load_track(&self, audio: DecodedAudio, autoplay: bool) {
        let _ = self
            .tx_cmd
            .send(MixerCommand::LoadTrack { audio, autoplay });
    }

    /// Loads a multi-track live performance session.
    pub fn load_live(&self, live: LivePerformance, autoplay: bool) {
        let _ = self.tx_cmd.send(MixerCommand::LoadLive {
            live: Box::new(live),
            autoplay,
        });
    }

    /// Updates live mixer real-time controls (mutes, singing modes, solo, volumes).
    ///
    /// Performer controls are keyed by stage slot index in lineup order.
    pub fn update_live_controls(
        &self,
        mute_oke: bool,
        mute_vocals: bool,
        performer_controls: Vec<PerformerControlUpdate>,
    ) {
        let _ = self.tx_cmd.send(MixerCommand::UpdateLiveControls {
            mute_oke,
            mute_vocals,
            performer_controls,
        });
    }

    /// Resumes playback.
    pub fn play(&self) {
        let _ = self.tx_cmd.send(MixerCommand::Play);
    }

    /// Pauses playback.
    pub fn pause(&self) {
        let _ = self.tx_cmd.send(MixerCommand::Pause);
    }

    /// Stops playback and rewinds to beginning.
    pub fn stop(&self) {
        let _ = self.tx_cmd.send(MixerCommand::Stop);
    }

    /// Seeks to a position in seconds.
    pub fn seek(&self, position_secs: f32) {
        let _ = self.tx_cmd.send(MixerCommand::Seek(position_secs));
    }

    /// Sets playback volume in range [0.0, 3.0].
    pub fn set_volume(&self, volume: f32) {
        let _ = self
            .tx_cmd
            .send(MixerCommand::SetVolume(volume.clamp(0.0, 3.0)));
    }

    /// Toggles seamless track looping.
    pub fn set_loop(&self, enabled: bool) {
        let _ = self.tx_cmd.send(MixerCommand::SetLoop(enabled));
    }
}

/// Applies slot-keyed real-time control updates to performer channels.
///
/// Each update targets the performer at its lineup index; updates pointing
/// past the end of the lineup are ignored. Index keying keeps duplicated
/// characters in different slots independently controllable.
pub(crate) fn apply_performer_controls(
    performers: &mut [LivePerformerAudio],
    controls: &[PerformerControlUpdate],
) {
    for upd in controls {
        if let Some(p) = performers.get_mut(upd.slot) {
            p.mode = upd.mode;
            p.solo = upd.solo;
            p.volume = upd.volume;
            p.main_vocal_enabled = upd.main_vocal_enabled;
        }
    }
}

/// Internal mixer worker thread running audio sample generation loop.
fn run_mixer_thread(
    rx_cmd: Receiver<MixerCommand>,
    status: Arc<SharedAudioStatus>,
    ring: Arc<SpscRingBuffer>,
    mut output_sample_rate: u32,
) {
    let mut current_track: Option<DecodedAudio> = None;
    let mut current_live: Option<LivePerformance> = None;
    let mut current_sample_idx = 0usize;
    let mut is_playing = false;
    let mut volume = 1.0f32;
    let mut is_loop = false;

    // Fade envelope for click-free track transitions
    let mut fade_factor = 1.0f32;
    let mut fade_step = 1.0f32 / (output_sample_rate as f32 * 0.02).max(1.0); // 20ms fade
    let mut pending_swap: Option<(Option<DecodedAudio>, Option<Box<LivePerformance>>, bool)> = None;

    let chunk_frames = 1024usize;
    let mut chunk_buf = vec![0.0f32; chunk_frames * 2];

    loop {
        // 1. Process commands
        while let Ok(cmd) = rx_cmd.try_recv() {
            match cmd {
                MixerCommand::LoadTrack {
                    mut audio,
                    autoplay,
                } => {
                    audio.resample_to(output_sample_rate);
                    audio.ensure_stereo();

                    if is_playing {
                        pending_swap = Some((Some(audio), None, autoplay));
                    } else {
                        current_live = None;
                        current_sample_idx = 0;
                        ring.clear();
                        status
                            .duration_bits
                            .store(f32::to_bits(audio.duration_secs), Ordering::Relaxed);
                        status
                            .position_bits
                            .store(f32::to_bits(0.0), Ordering::Relaxed);
                        *status.track_title.lock().unwrap() = Some(audio.title.clone());
                        current_track = Some(audio);
                        fade_factor = 0.0;
                        is_playing = autoplay;
                        *status.state.lock().unwrap() = if autoplay {
                            PlaybackState::Playing
                        } else {
                            PlaybackState::Stopped
                        };
                    }
                }
                MixerCommand::LoadLive { mut live, autoplay } => {
                    live.oke_audio.resample_to(output_sample_rate);
                    live.oke_audio.ensure_stereo();

                    for p in &mut live.performers {
                        if let Some(m) = &mut p.main_audio {
                            m.resample_to(output_sample_rate);
                            m.ensure_stereo();
                        }
                        if let Some(e) = &mut p.extra_audio {
                            e.resample_to(output_sample_rate);
                            e.ensure_stereo();
                        }
                    }

                    if is_playing {
                        pending_swap = Some((None, Some(live), autoplay));
                    } else {
                        current_track = None;
                        current_sample_idx = 0;
                        ring.clear();
                        status.duration_bits.store(
                            f32::to_bits(live.oke_audio.duration_secs),
                            Ordering::Relaxed,
                        );
                        status
                            .position_bits
                            .store(f32::to_bits(0.0), Ordering::Relaxed);
                        *status.track_title.lock().unwrap() = Some(live.title.clone());
                        current_live = Some(*live);
                        fade_factor = 0.0;
                        is_playing = autoplay;
                        *status.state.lock().unwrap() = if autoplay {
                            PlaybackState::Playing
                        } else {
                            PlaybackState::Stopped
                        };
                    }
                }
                MixerCommand::UpdateLiveControls {
                    mute_oke,
                    mute_vocals,
                    performer_controls,
                } => {
                    if let Some(live) = &mut current_live {
                        live.mute_oke = mute_oke;
                        live.mute_vocals = mute_vocals;
                        apply_performer_controls(&mut live.performers, &performer_controls);
                    }
                    // Keep a live that is still fading in for a swap in sync,
                    // so control changes made during the transition are kept.
                    if let Some((_, Some(live), _)) = &mut pending_swap {
                        live.mute_oke = mute_oke;
                        live.mute_vocals = mute_vocals;
                        apply_performer_controls(&mut live.performers, &performer_controls);
                    }
                }
                MixerCommand::Play => {
                    if current_track.is_some() || current_live.is_some() {
                        is_playing = true;
                        *status.state.lock().unwrap() = PlaybackState::Playing;
                    }
                }
                MixerCommand::Pause => {
                    is_playing = false;
                    *status.state.lock().unwrap() = PlaybackState::Paused;
                }
                MixerCommand::Stop => {
                    is_playing = false;
                    current_sample_idx = 0;
                    ring.clear();
                    status
                        .position_bits
                        .store(f32::to_bits(0.0), Ordering::Relaxed);
                    *status.state.lock().unwrap() = PlaybackState::Stopped;
                }
                MixerCommand::Seek(pos_secs) => {
                    let total_samples = if let Some(track) = &current_track {
                        track.samples.len()
                    } else if let Some(live) = &current_live {
                        live.oke_audio.samples.len()
                    } else {
                        0
                    };

                    if total_samples > 0 {
                        let target_frame = (pos_secs * output_sample_rate as f32).max(0.0) as usize;
                        current_sample_idx = (target_frame * 2).min(total_samples);
                        ring.clear();
                        status
                            .position_bits
                            .store(f32::to_bits(pos_secs), Ordering::Relaxed);
                    }
                }
                MixerCommand::SetVolume(v) => {
                    let clamped = v.clamp(0.0, 3.0);
                    volume = clamped;
                    status
                        .volume_bits
                        .store(f32::to_bits(clamped), Ordering::Relaxed);
                }
                MixerCommand::SetLoop(lp) => {
                    is_loop = lp;
                    status.loop_enabled.store(lp, Ordering::Relaxed);
                }
                MixerCommand::SetOutputRate(new_rate) => {
                    if new_rate > 0 && new_rate != output_sample_rate {
                        let old_rate = output_sample_rate;
                        output_sample_rate = new_rate;
                        fade_step = 1.0f32 / (output_sample_rate as f32 * 0.02).max(1.0);
                        let scale = new_rate as f64 / old_rate as f64;

                        if let Some(track) = &mut current_track {
                            track.resample_to(new_rate);
                            let frame = (current_sample_idx / 2) as f64 * scale;
                            current_sample_idx =
                                (frame.round() as usize * 2).min(track.samples.len());
                        } else if let Some(live) = &mut current_live {
                            live.oke_audio.resample_to(new_rate);
                            for p in &mut live.performers {
                                if let Some(m) = &mut p.main_audio {
                                    m.resample_to(new_rate);
                                }
                                if let Some(e) = &mut p.extra_audio {
                                    e.resample_to(new_rate);
                                }
                            }
                            let frame = (current_sample_idx / 2) as f64 * scale;
                            current_sample_idx =
                                (frame.round() as usize * 2).min(live.oke_audio.samples.len());
                        }

                        if let Some((next_track, next_live, _)) = &mut pending_swap {
                            if let Some(t) = next_track {
                                t.resample_to(new_rate);
                            }
                            if let Some(l) = next_live {
                                l.oke_audio.resample_to(new_rate);
                                for p in &mut l.performers {
                                    if let Some(m) = &mut p.main_audio {
                                        m.resample_to(new_rate);
                                    }
                                    if let Some(e) = &mut p.extra_audio {
                                        e.resample_to(new_rate);
                                    }
                                }
                            }
                        }

                        ring.clear();
                        let current_secs =
                            (current_sample_idx / 2) as f32 / output_sample_rate as f32;
                        status
                            .position_bits
                            .store(f32::to_bits(current_secs), Ordering::Relaxed);
                    }
                }
            }
        }

        // 2. Audio sample generation
        if is_playing || pending_swap.is_some() {
            let available_space = ring.capacity.saturating_sub(ring.available_read());
            if available_space >= chunk_buf.len() {
                let mut generated_samples = 0;

                for frame_idx in 0..chunk_frames {
                    // Apply fade factor
                    if pending_swap.is_some() {
                        fade_factor = (fade_factor - fade_step).max(0.0);
                    } else if fade_factor < 1.0 {
                        fade_factor = (fade_factor + fade_step).min(1.0);
                    }

                    // A: Single-track generation
                    if let Some(track) = &current_track {
                        if current_sample_idx >= track.samples.len()
                            || (track.loop_end.is_some()
                                && current_sample_idx >= track.loop_end.unwrap())
                        {
                            if is_loop || track.loop_start.is_some() {
                                current_sample_idx = track.loop_start.unwrap_or(0);
                            } else {
                                is_playing = false;
                                *status.state.lock().unwrap() = PlaybackState::Stopped;
                                status.track_ended.store(true, Ordering::Relaxed);
                                break;
                            }
                        }

                        let l = track.samples[current_sample_idx] * volume * fade_factor;
                        let r = track.samples[current_sample_idx + 1] * volume * fade_factor;
                        current_sample_idx += 2;

                        chunk_buf[frame_idx * 2] = l.clamp(-1.0, 1.0);
                        chunk_buf[frame_idx * 2 + 1] = r.clamp(-1.0, 1.0);
                        generated_samples += 2;
                    }
                    // B: Live multi-track generation
                    else if let Some(live) = &current_live {
                        let total_len = live.oke_audio.samples.len();
                        if current_sample_idx >= total_len {
                            if is_loop {
                                current_sample_idx = 0;
                            } else {
                                is_playing = false;
                                *status.state.lock().unwrap() = PlaybackState::Stopped;
                                status.track_ended.store(true, Ordering::Relaxed);
                                break;
                            }
                        }

                        let frame_pos = current_sample_idx / 2;
                        let time_ms =
                            ((frame_pos as f64 / output_sample_rate as f64) * 1000.0) as u32;

                        // Oke track
                        let (oke_l, oke_r) = if !live.mute_oke && current_sample_idx + 1 < total_len
                        {
                            (
                                live.oke_audio.samples[current_sample_idx],
                                live.oke_audio.samples[current_sample_idx + 1],
                            )
                        } else {
                            (0.0, 0.0)
                        };

                        // Vocal tracks
                        let (mut vocal_l, mut vocal_r) = (0.0f32, 0.0f32);
                        if !live.mute_vocals && !live.performers.is_empty() {
                            let has_any_solo = live.performers.iter().any(|p| p.solo);
                            let trigger = live.score.part_at(time_ms);
                            let num_members = live.performers.len();

                            let mut active_singers = Vec::with_capacity(num_members);
                            for (stage_idx, performer) in live.performers.iter().enumerate() {
                                if has_any_solo && !performer.solo {
                                    continue;
                                }

                                let csv_idx = crate::gameaudio::unity::stage_pos_to_center_out(
                                    stage_idx,
                                    num_members,
                                );
                                let part_info = trigger
                                    .and_then(|t| t.members.get(csv_idx))
                                    .copied()
                                    .unwrap_or_default();

                                let track_choice = match performer.mode {
                                    SingingMode::Mute => 0,
                                    SingingMode::ForceMain => 1,
                                    SingingMode::ForceExtra => 2,
                                    SingingMode::ForceMerge => 3,
                                    SingingMode::Auto => part_info.track,
                                };

                                if track_choice > 0 {
                                    active_singers.push((
                                        performer,
                                        track_choice,
                                        part_info.volume,
                                    ));
                                }
                            }

                            let k = active_singers.len();
                            let volume_rate = trigger.map(|t| t.volume_rate).unwrap_or(1.0);
                            let norm = if volume_rate == 1.0 || k <= 1 {
                                1.0f32
                            } else {
                                (1.0f32 / (k as f32).sqrt()) * volume_rate
                            };

                            for (performer, track_choice, part_vol) in active_singers {
                                let p_vol = performer.volume
                                    * norm
                                    * if part_vol < 900.0 {
                                        (part_vol / 100.0).clamp(0.0, 2.0)
                                    } else {
                                        1.0
                                    };

                                let mut s_l = 0.0f32;
                                let mut s_r = 0.0f32;

                                if (track_choice == 1 || track_choice == 3)
                                    && performer.main_vocal_enabled
                                {
                                    if let Some(main) = &performer.main_audio {
                                        if current_sample_idx + 1 < main.samples.len() {
                                            s_l += main.samples[current_sample_idx];
                                            s_r += main.samples[current_sample_idx + 1];
                                        }
                                    }
                                }

                                if track_choice == 2 || track_choice == 3 {
                                    let extra_to_use =
                                        performer.extra_audio.as_ref().or_else(|| {
                                            if performer.main_vocal_enabled {
                                                performer.main_audio.as_ref()
                                            } else {
                                                None
                                            }
                                        });
                                    if let Some(extra) = extra_to_use {
                                        if current_sample_idx + 1 < extra.samples.len() {
                                            if track_choice == 3 {
                                                if performer.main_vocal_enabled {
                                                    s_l = (s_l + extra.samples[current_sample_idx])
                                                        * 0.7;
                                                    s_r = (s_r
                                                        + extra.samples[current_sample_idx + 1])
                                                        * 0.7;
                                                } else {
                                                    s_l += extra.samples[current_sample_idx];
                                                    s_r += extra.samples[current_sample_idx + 1];
                                                }
                                            } else {
                                                s_l += extra.samples[current_sample_idx];
                                                s_r += extra.samples[current_sample_idx + 1];
                                            }
                                        }
                                    }
                                }

                                vocal_l += s_l * p_vol;
                                vocal_r += s_r * p_vol;
                            }
                        }

                        let out_l = (oke_l + vocal_l) * volume * fade_factor;
                        let out_r = (oke_r + vocal_r) * volume * fade_factor;
                        current_sample_idx += 2;

                        chunk_buf[frame_idx * 2] = out_l.clamp(-1.0, 1.0);
                        chunk_buf[frame_idx * 2 + 1] = out_r.clamp(-1.0, 1.0);
                        generated_samples += 2;
                    }

                    if pending_swap.is_some() && fade_factor <= 0.0 {
                        break;
                    }
                }

                if generated_samples > 0 {
                    ring.write(&chunk_buf[..generated_samples]);
                    // Report audio the output device has already consumed:
                    // samples still queued in the ring have not been heard yet.
                    let played_frames =
                        (current_sample_idx / 2).saturating_sub(ring.available_read() / 2);
                    let played_secs = played_frames as f32 / output_sample_rate as f32;
                    status
                        .position_bits
                        .store(f32::to_bits(played_secs), Ordering::Relaxed);
                }

                if let Some((next_track, next_live, next_autoplay)) = pending_swap.take() {
                    if fade_factor <= 0.0 {
                        current_sample_idx = 0;
                        ring.clear();
                        status
                            .position_bits
                            .store(f32::to_bits(0.0), Ordering::Relaxed);

                        if let Some(track) = next_track {
                            status
                                .duration_bits
                                .store(f32::to_bits(track.duration_secs), Ordering::Relaxed);
                            *status.track_title.lock().unwrap() = Some(track.title.clone());
                            current_track = Some(track);
                            current_live = None;
                        } else if let Some(live) = next_live {
                            status.duration_bits.store(
                                f32::to_bits(live.oke_audio.duration_secs),
                                Ordering::Relaxed,
                            );
                            *status.track_title.lock().unwrap() = Some(live.title.clone());
                            current_live = Some(*live);
                            current_track = None;
                        }

                        fade_factor = 0.0;
                        is_playing = next_autoplay;
                        *status.state.lock().unwrap() = if next_autoplay {
                            PlaybackState::Playing
                        } else {
                            PlaybackState::Stopped
                        };
                    } else {
                        pending_swap = Some((next_track, next_live, next_autoplay));
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(5));
    }
}

/// Renders a full multi-track LivePerformance session to 16-bit PCM WAV bytes.
pub fn render_live_mix_pcm16(live: &mut LivePerformance) -> Result<Vec<u8>, String> {
    let sample_rate = live.oke_audio.sample_rate.max(44100);
    live.oke_audio.resample_to(sample_rate);
    live.oke_audio.ensure_stereo();

    for p in &mut live.performers {
        if let Some(m) = &mut p.main_audio {
            m.resample_to(sample_rate);
            m.ensure_stereo();
        }
        if let Some(e) = &mut p.extra_audio {
            e.resample_to(sample_rate);
            e.ensure_stereo();
        }
    }

    let total_frames = live.oke_audio.samples.len() / 2;
    if total_frames == 0 {
        return Err("Live oke track is empty".to_string());
    }

    let mut mixed_samples = Vec::with_capacity(total_frames * 2);
    let num_members = live.performers.len();
    let has_any_solo = live.performers.iter().any(|p| p.solo);

    for frame_idx in 0..total_frames {
        let sample_idx = frame_idx * 2;
        let time_ms = ((frame_idx as f64 / sample_rate as f64) * 1000.0) as u32;

        let (oke_l, oke_r) = if !live.mute_oke && sample_idx + 1 < live.oke_audio.samples.len() {
            (
                live.oke_audio.samples[sample_idx],
                live.oke_audio.samples[sample_idx + 1],
            )
        } else {
            (0.0, 0.0)
        };

        let (mut vocal_l, mut vocal_r) = (0.0f32, 0.0f32);
        if !live.mute_vocals && !live.performers.is_empty() {
            let trigger = live.score.part_at(time_ms);
            let mut active_singers = Vec::with_capacity(num_members);

            for (stage_idx, performer) in live.performers.iter().enumerate() {
                if has_any_solo && !performer.solo {
                    continue;
                }

                let csv_idx =
                    crate::gameaudio::unity::stage_pos_to_center_out(stage_idx, num_members);
                let part_info = trigger
                    .and_then(|t| t.members.get(csv_idx))
                    .copied()
                    .unwrap_or_default();

                let track_choice = match performer.mode {
                    SingingMode::Mute => 0,
                    SingingMode::ForceMain => 1,
                    SingingMode::ForceExtra => 2,
                    SingingMode::ForceMerge => 3,
                    SingingMode::Auto => part_info.track,
                };

                if track_choice > 0 {
                    active_singers.push((performer, track_choice, part_info.volume));
                }
            }

            let k = active_singers.len();
            let volume_rate = trigger.map(|t| t.volume_rate).unwrap_or(1.0);
            let norm = if volume_rate == 1.0 || k <= 1 {
                1.0f32
            } else {
                (1.0f32 / (k as f32).sqrt()) * volume_rate
            };

            for (performer, track_choice, part_vol) in active_singers {
                let p_vol = performer.volume
                    * norm
                    * if part_vol < 900.0 {
                        (part_vol / 100.0).clamp(0.0, 2.0)
                    } else {
                        1.0
                    };
                let mut s_l = 0.0f32;
                let mut s_r = 0.0f32;

                if (track_choice == 1 || track_choice == 3) && performer.main_vocal_enabled {
                    if let Some(main) = &performer.main_audio {
                        if sample_idx + 1 < main.samples.len() {
                            s_l += main.samples[sample_idx];
                            s_r += main.samples[sample_idx + 1];
                        }
                    }
                }

                if track_choice == 2 || track_choice == 3 {
                    let extra_to_use = performer.extra_audio.as_ref().or_else(|| {
                        if performer.main_vocal_enabled {
                            performer.main_audio.as_ref()
                        } else {
                            None
                        }
                    });
                    if let Some(extra) = extra_to_use {
                        if sample_idx + 1 < extra.samples.len() {
                            if track_choice == 3 {
                                if performer.main_vocal_enabled {
                                    s_l = (s_l + extra.samples[sample_idx]) * 0.7;
                                    s_r = (s_r + extra.samples[sample_idx + 1]) * 0.7;
                                } else {
                                    s_l += extra.samples[sample_idx];
                                    s_r += extra.samples[sample_idx + 1];
                                }
                            } else {
                                s_l += extra.samples[sample_idx];
                                s_r += extra.samples[sample_idx + 1];
                            }
                        }
                    }
                }

                vocal_l += s_l * p_vol;
                vocal_r += s_r * p_vol;
            }
        }

        let l = (oke_l + vocal_l).clamp(-1.0, 1.0);
        let r = (oke_r + vocal_r).clamp(-1.0, 1.0);
        mixed_samples.push(l);
        mixed_samples.push(r);
    }

    let mut out_bytes = Vec::new();
    crate::gameaudio::wav::write_wav_pcm16(&mut out_bytes, 2, sample_rate, &mixed_samples)
        .map_err(|e| format!("Failed to encode WAV: {e}"))?;
    Ok(out_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn performer(chara_id: i32, name: &str) -> LivePerformerAudio {
        LivePerformerAudio {
            chara_id,
            name: name.to_string(),
            main_audio: None,
            extra_audio: None,
            mode: SingingMode::Auto,
            solo: false,
            volume: 1.0,
            main_vocal_enabled: true,
        }
    }

    fn control(slot: usize, mode: SingingMode) -> PerformerControlUpdate {
        PerformerControlUpdate {
            slot,
            mode,
            solo: false,
            volume: 1.0,
            main_vocal_enabled: true,
        }
    }

    #[test]
    fn updates_target_the_exact_slot_not_the_first_matching_chara() {
        // The same Uma standing in two slots must stay independently
        // controllable; the old chara_id-keyed update only ever touched slot 0.
        let mut performers = vec![
            performer(1001, "Special Week"),
            performer(1001, "Special Week"),
            performer(1002, "Silence Suzuka"),
        ];
        apply_performer_controls(&mut performers, &[control(1, SingingMode::ForceMain)]);
        assert_eq!(performers[0].mode, SingingMode::Auto);
        assert_eq!(performers[1].mode, SingingMode::ForceMain);
        assert_eq!(performers[2].mode, SingingMode::Auto);
    }

    #[test]
    fn switching_main_to_another_slot_updates_only_those_slots() {
        // Mirrors the UI single-main rule: the new lead is forced, the old
        // lead reverts to Auto, everyone else is untouched.
        let mut performers = vec![
            performer(1001, "Special Week"),
            performer(1002, "Silence Suzuka"),
            performer(1003, "Tokai Teio"),
        ];
        performers[0].mode = SingingMode::ForceMain;
        apply_performer_controls(
            &mut performers,
            &[
                control(0, SingingMode::Auto),
                control(1, SingingMode::ForceMain),
            ],
        );
        assert_eq!(performers[0].mode, SingingMode::Auto);
        assert_eq!(performers[1].mode, SingingMode::ForceMain);
        assert_eq!(performers[2].mode, SingingMode::Auto);
    }

    #[test]
    fn slots_past_lineup_end_or_without_voice_assets_are_ignored() {
        // A slot with no decoded voice (main_audio/extra_audio None) still
        // accepts mode changes, and stale indices beyond the lineup are
        // dropped instead of panicking.
        let mut performers = vec![performer(0, "No vocal data")];
        apply_performer_controls(
            &mut performers,
            &[
                control(0, SingingMode::Mute),
                control(7, SingingMode::ForceMain),
            ],
        );
        assert_eq!(performers[0].mode, SingingMode::Mute);
        assert!(performers[0].main_audio.is_none());
    }
}
