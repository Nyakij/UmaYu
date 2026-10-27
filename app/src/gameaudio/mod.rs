//! Game Audio subsystem for UmaYu.
//!
//! Provides game installation detection, manifest parsing (`meta`),
//! master database loading (`master.mdb`), audio decoding (`HCA`/`AWB`/`ACB`),
//! real-time playback mixing via `cpal`, and WAV file export.

#![allow(dead_code, unused_imports)]

pub mod export;
pub mod gamedir;
pub mod master;
pub mod meta;
pub mod mixer;
pub mod texture;
pub mod unity;
pub mod wav;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

pub use export::{
    ExportCategory, default_export_base_dir, open_folder_in_explorer, resolve_category_dir,
    resolve_export_base_dir,
};
pub use gamedir::{GameInstallation, GameRegion};
pub use master::MasterData;
pub use meta::ManifestEntry;
pub use mixer::{
    AudioEngine, CueInfo, DecodedAudio, LivePerformance, LivePerformerAudio,
    PerformerControlUpdate, PlaybackState, SharedAudioStatus, SingingMode, decode_acb_file,
    decode_audio_file, decode_awb_file, decode_hca_bytes, inspect_audio_file, prepare_acb_file,
    render_live_mix_pcm16,
};
pub use texture::{
    DecodedImage, TextureKey, TextureManager, TextureSlot, find_chara_icon_entry,
    find_jacket_entry, find_packaged_uma_art,
};
pub use unity::{
    LiveScore, LyricsLoadState, LyricsTrigger, MemberPart, PartTrigger, center_out_to_stage_pos,
    extract_text_asset_from_bundle, load_live_score, parse_lyrics_csv, parse_part_csv,
    stage_pos_to_center_out,
};
pub use wav::{save_wav_file, write_wav_pcm16};

/// Version preference for Jukebox playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JukeboxVersion {
    Short,
    GameSize,
}

/// Status of the game audio data loading pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameAudioStatus {
    Disabled,
    Idle,
    Scanning,
    Loading,
    Ready,
    Error(String),
}

/// Precomputed disk availability and track readiness cache.
#[derive(Debug, Clone, Default)]
pub struct AudioReadinessCache {
    /// Asset manifest entry IDs physically present on disk.
    pub on_disk: HashSet<i64>,
    /// Jukebox readiness mapping: music_id -> (short_ready, game_ready).
    pub juke_ready: HashMap<i32, (bool, bool)>,
    /// Live performance readiness set: music_ids whose oke accompaniment is on disk.
    pub live_ready: HashSet<i32>,
    /// Live character vocal readiness: (music_id, chara_id) -> bool.
    pub live_chara_ready: HashMap<(i32, i32), bool>,
    /// True while background scanner thread is checking disk files.
    pub is_scanning: bool,
    /// True once the initial background disk scan has finished.
    pub is_ready: bool,
}

/// A candidate Uma for a live stage lineup, with local voice asset availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveCharaCandidate {
    pub chara_id: i32,
    /// True when this character's voice AWB for the live track is on disk.
    pub voice_ready: bool,
}

/// An independent game data catalog for a specific game region (Global or JP).
#[derive(Clone)]
pub struct GameAudioCatalog {
    pub region: GameRegion,
    pub game_path: String,
    pub status: GameAudioStatus,
    pub manifest_entries: Arc<Vec<ManifestEntry>>,
    pub name_index: HashMap<String, usize>,
    pub cuesheet_index: HashMap<String, (Option<usize>, Option<usize>)>,
    pub master: MasterData,
    pub readiness: AudioReadinessCache,
    pub load_generation: u64,
    pub scan_generation: u64,
    pub last_error: Option<String>,
}

impl Default for GameAudioCatalog {
    fn default() -> Self {
        Self::new(GameRegion::Global, String::new())
    }
}

impl GameAudioCatalog {
    pub fn new(region: GameRegion, game_path: String) -> Self {
        Self {
            region,
            game_path,
            status: GameAudioStatus::Idle,
            manifest_entries: Arc::new(Vec::new()),
            name_index: HashMap::new(),
            cuesheet_index: HashMap::new(),
            master: MasterData::default(),
            readiness: AudioReadinessCache::default(),
            load_generation: 0,
            scan_generation: 0,
            last_error: None,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.game_path.is_empty()
    }

    pub fn is_ready(&self) -> bool {
        self.status == GameAudioStatus::Ready
    }

    pub fn clear(&mut self) {
        self.manifest_entries = Arc::new(Vec::new());
        self.name_index.clear();
        self.cuesheet_index.clear();
        self.master = MasterData::default();
        self.readiness = AudioReadinessCache::default();
        self.status = if self.is_configured() {
            GameAudioStatus::Idle
        } else {
            GameAudioStatus::Disabled
        };
        self.last_error = None;
    }

    pub fn build_indices(&mut self) {
        let mut name_index = HashMap::with_capacity(self.manifest_entries.len());
        let mut cuesheet_index = HashMap::with_capacity(self.manifest_entries.len() / 2);

        for (idx, entry) in self.manifest_entries.iter().enumerate() {
            let name_lower = entry.name.to_lowercase();
            name_index.insert(name_lower.clone(), idx);

            if let Some(filename) = name_lower.split('/').last() {
                if let Some(stem) = filename.strip_suffix(".awb") {
                    let entry_slot = cuesheet_index
                        .entry(stem.to_string())
                        .or_insert((None, None));
                    entry_slot.0 = Some(idx);
                } else if let Some(stem) = filename.strip_suffix(".acb") {
                    let entry_slot = cuesheet_index
                        .entry(stem.to_string())
                        .or_insert((None, None));
                    entry_slot.1 = Some(idx);
                }
            }
        }

        self.name_index = name_index;
        self.cuesheet_index = cuesheet_index;
    }

    pub fn find_companion_awb(&self, acb_name: &str) -> Option<&ManifestEntry> {
        if let Some(base) = acb_name.strip_suffix(".acb") {
            let awb_name = format!("{base}.awb").to_lowercase();
            if let Some(&idx) = self.name_index.get(&awb_name) {
                return self.manifest_entries.get(idx);
            }
            self.manifest_entries
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(&awb_name))
        } else {
            None
        }
    }

    pub fn inspect_entry_cues(&self, entry: &ManifestEntry) -> Result<Vec<CueInfo>, String> {
        let game_dir = Path::new(&self.game_path);
        if entry.name.ends_with(".acb") {
            let companion_awb = self.find_companion_awb(&entry.name);
            let prep_acb = mixer::prepare_acb_file(game_dir, entry, companion_awb)?;
            mixer::inspect_audio_file(&prep_acb)
        } else {
            let path = entry.file_path(game_dir);
            mixer::inspect_audio_file(&path)
        }
    }

    pub fn find_cuesheet_entry(&self, cuesheet_name: &str) -> Option<&ManifestEntry> {
        let cs_lower = cuesheet_name.trim().to_lowercase();

        if let Some(&(awb_opt, acb_opt)) = self.cuesheet_index.get(&cs_lower) {
            if let Some(idx) = awb_opt.or(acb_opt) {
                return self.manifest_entries.get(idx);
            }
        }

        let awb_exact = format!("{cs_lower}.awb");
        let acb_exact = format!("{cs_lower}.acb");
        if let Some(&idx) = self
            .name_index
            .get(&awb_exact)
            .or_else(|| self.name_index.get(&acb_exact))
        {
            return self.manifest_entries.get(idx);
        }

        let awb_suffix = format!("/{cs_lower}.awb");
        let acb_suffix = format!("/{cs_lower}.acb");

        if let Some(entry) = self.manifest_entries.iter().find(|e| {
            let n = e.name.to_lowercase();
            n.ends_with(&awb_suffix) || n == awb_exact
        }) {
            return Some(entry);
        }

        self.manifest_entries.iter().find(|e| {
            let n = e.name.to_lowercase();
            n.ends_with(&acb_suffix) || n == acb_exact
        })
    }

    pub fn find_live_oke_entry(&self, music_id: i32, sfx: u8) -> Option<ManifestEntry> {
        let p1 = format!("sound/l/snd_bgm_live_{music_id}_oke_{sfx:02}.awb");
        let p2 = format!("sound/l/snd_bgm_live_{music_id:04}_oke_{sfx:02}.awb");
        let p3 = format!("sound/l/snd_bgm_live_{music_id}_oke_{sfx:02}_01.awb");
        let p4 = format!("sound/l/snd_bgm_live_{music_id:04}_oke_{sfx:02}_01.awb");
        let other_sfx = if sfx == 1 { 2 } else { 1 };
        let p5 = format!("sound/l/snd_bgm_live_{music_id}_oke_{other_sfx:02}.awb");
        let p6 = format!("sound/l/snd_bgm_live_{music_id:04}_oke_{other_sfx:02}.awb");

        for p in &[&p1, &p2, &p3, &p4, &p5, &p6] {
            if let Some(&idx) = self.name_index.get(*p) {
                return self.manifest_entries.get(idx).cloned();
            }
        }

        for p in &[&p1, &p2, &p3, &p4, &p5, &p6] {
            if let Some(e) = self
                .manifest_entries
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(p))
            {
                return Some(e.clone());
            }
        }

        let needle = format!("live_{music_id}_oke");
        let needle4 = format!("live_{music_id:04}_oke");
        self.manifest_entries
            .iter()
            .find(|e| {
                let n = e.name.to_lowercase();
                (n.contains(&needle) || n.contains(&needle4)) && n.ends_with(".awb")
            })
            .cloned()
    }

    pub fn find_live_chara_entry(&self, music_id: i32, chara_id: i32) -> Option<ManifestEntry> {
        let p1 = format!("sound/l/snd_bgm_live_{music_id}_chara_{chara_id}_01.awb");
        let p2 = format!("sound/l/snd_bgm_live_{music_id:04}_chara_{chara_id}_01.awb");
        let p3 = format!("sound/l/snd_bgm_live_{music_id}_chara_{chara_id:04}_01.awb");
        let p4 = format!("sound/l/snd_bgm_live_{music_id:04}_chara_{chara_id:04}_01.awb");
        let p5 = format!("sound/l/snd_bgm_live_{music_id}_chara_{chara_id}.awb");

        for p in &[&p1, &p2, &p3, &p4, &p5] {
            if let Some(&idx) = self.name_index.get(*p) {
                return self.manifest_entries.get(idx).cloned();
            }
        }

        for p in &[&p1, &p2, &p3, &p4, &p5] {
            if let Some(e) = self
                .manifest_entries
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(p))
            {
                return Some(e.clone());
            }
        }

        let needle = format!("live_{music_id}_chara_{chara_id}");
        let needle4 = format!("live_{music_id:04}_chara_{chara_id}");
        self.manifest_entries
            .iter()
            .find(|e| {
                let n = e.name.to_lowercase();
                (n.contains(&needle) || n.contains(&needle4)) && n.ends_with(".awb")
            })
            .cloned()
    }

    pub fn is_live_ready(&self, music_id: i32) -> bool {
        if self.readiness.is_ready {
            self.readiness.live_ready.contains(&music_id)
        } else if !self.game_path.is_empty() {
            let game_dir = Path::new(&self.game_path);
            if let Some(oke) = self
                .find_live_oke_entry(music_id, 1)
                .or_else(|| self.find_live_oke_entry(music_id, 2))
            {
                oke.exists_on_disk(game_dir)
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn is_live_chara_ready(&self, music_id: i32, chara_id: i32) -> bool {
        if self.readiness.is_ready {
            self.readiness
                .live_chara_ready
                .get(&(music_id, chara_id))
                .copied()
                .unwrap_or(false)
        } else if !self.game_path.is_empty() {
            let game_dir = Path::new(&self.game_path);
            if let Some(chara) = self.find_live_chara_entry(music_id, chara_id) {
                chara.exists_on_disk(game_dir)
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn is_jukebox_ready(&self, music_id: i32) -> (bool, bool) {
        if self.readiness.is_ready {
            self.readiness
                .juke_ready
                .get(&music_id)
                .copied()
                .unwrap_or((false, false))
        } else {
            (false, false)
        }
    }

    pub fn is_entry_on_disk(&self, entry_id: i64) -> bool {
        if self.readiness.is_ready {
            self.readiness.on_disk.contains(&entry_id)
        } else {
            false
        }
    }

    /// Lists Uma candidates for a live lineup: characters permitted by
    /// `live_permission_data` first (preserving table order), then characters
    /// whose voice AWB appears in the asset manifest even when the permission
    /// table misses them. Never fabricates ids without a data source.
    pub fn live_chara_candidates(&self, music_id: i32) -> Vec<LiveCharaCandidate> {
        let mut ids: Vec<i32> = self
            .master
            .live_perm_index
            .get(&music_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .to_vec();

        for entry in self.manifest_entries.iter() {
            if let Some((mid, cid)) = parse_live_chara_entry_name(&entry.name) {
                if mid == music_id && !ids.contains(&cid) {
                    ids.push(cid);
                }
            }
        }

        ids.into_iter()
            .map(|cid| LiveCharaCandidate {
                chara_id: cid,
                voice_ready: self.is_live_chara_ready(music_id, cid),
            })
            .collect()
    }
}

/// Strips `prefix` from `s` (ASCII case-insensitive) or returns None.
fn strip_prefix_ignore_ascii_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() {
        if let Some((head, rest)) = s.split_at_checked(prefix.len()) {
            if head.eq_ignore_ascii_case(prefix) {
                return Some(rest);
            }
        }
    }
    None
}

/// Checks an ASCII case-insensitive suffix match.
fn ends_with_ignore_ascii_case(s: &str, suffix: &str) -> bool {
    match s.len().checked_sub(suffix.len()) {
        Some(start) => s
            .get(start..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix)),
        None => false,
    }
}

/// Parses `sound/l/snd_bgm_live_{mid}_chara_{cid}[_01].awb` asset names into
/// `(music_id, chara_id)`, tolerating zero-padded id segments.
pub(crate) fn parse_live_chara_entry_name(name: &str) -> Option<(i32, i32)> {
    let rest = strip_prefix_ignore_ascii_case(name, "sound/l/snd_bgm_live_")?;
    let rest = if ends_with_ignore_ascii_case(rest, ".awb") {
        &rest[..rest.len() - 4]
    } else {
        rest
    };
    let (mid_s, cid_s) = rest.split_once("_chara_")?;
    let mid = mid_s.split('_').next()?.trim_start_matches('0');
    let cid = cid_s.split('_').next()?.trim_start_matches('0');
    Some((mid.parse().ok()?, cid.parse().ok()?))
}

/// Finds a performer's voice AWB manifest entry for a live music id, trying
/// unpadded/padded id name variants. Shared by playback and WAV export so
/// both resolve the exact same voice asset.
fn find_live_chara_manifest_entry(
    manifest: &[ManifestEntry],
    music_id: i32,
    chara_id: i32,
) -> Option<ManifestEntry> {
    let p1 = format!("sound/l/snd_bgm_live_{music_id}_chara_{chara_id}_01.awb");
    let p2 = format!("sound/l/snd_bgm_live_{music_id:04}_chara_{chara_id}_01.awb");
    let p3 = format!("sound/l/snd_bgm_live_{music_id}_chara_{chara_id:04}_01.awb");
    let p4 = format!("sound/l/snd_bgm_live_{music_id:04}_chara_{chara_id:04}_01.awb");
    let p5 = format!("sound/l/snd_bgm_live_{music_id}_chara_{chara_id}.awb");

    for p in &[&p1, &p2, &p3, &p4, &p5] {
        if let Some(e) = manifest.iter().find(|e| e.name.eq_ignore_ascii_case(p)) {
            return Some(e.clone());
        }
    }

    let needle = format!("live_{music_id}_chara_{chara_id}");
    let needle4 = format!("live_{music_id:04}_chara_{chara_id}");
    manifest
        .iter()
        .find(|e| {
            let n = e.name.to_lowercase();
            (n.contains(&needle) || n.contains(&needle4)) && n.ends_with(".awb")
        })
        .cloned()
}

/// Messages sent from background worker threads to the main UI thread.
pub enum GameAudioMsg {
    InstallationsDetected(Vec<GameInstallation>),
    DataLoaded {
        region: GameRegion,
        generation: u64,
        manifest: Result<Vec<ManifestEntry>, String>,
        master: Result<MasterData, String>,
    },
    ReadinessScanned {
        region: GameRegion,
        generation: u64,
        cache: AudioReadinessCache,
    },
    AudioDecoded {
        track: Result<DecodedAudio, String>,
        autoplay: bool,
    },
    LiveDecoded {
        music_id: i32,
        live: Result<Box<LivePerformance>, String>,
        autoplay: bool,
    },
    WavExported {
        result: Result<PathBuf, String>,
    },
}

/// Consolidated state of Game Audio subsystem within the App.
pub struct GameAudioState {
    pub enabled: bool,
    pub active_region: GameRegion,
    pub global: GameAudioCatalog,
    pub jp: GameAudioCatalog,
    pub game_path: String,
    pub custom_key: String,
    pub output_device: String,
    pub output_devices: Vec<String>,
    pub volume: f32,
    pub live_vocal_default_volume: f32,
    pub high_volume_warning_acknowledged: bool,
    pub export_dir: String,
    pub detected_installs: Vec<GameInstallation>,
    pub status: GameAudioStatus,
    pub manifest_entries: Arc<Vec<ManifestEntry>>,
    pub name_index: HashMap<String, usize>,
    pub cuesheet_index: HashMap<String, (Option<usize>, Option<usize>)>,
    pub master: MasterData,
    pub readiness: AudioReadinessCache,
    pub engine: Option<AudioEngine>,
    pub current_live_score: Option<LiveScore>,
    pub loading_live_id: Option<i32>,
    pub is_exporting: bool,
    pub toast_msg: Option<String>,
    pub last_error: Option<String>,
    tx: Sender<GameAudioMsg>,
    rx: Receiver<GameAudioMsg>,
}

impl Default for GameAudioState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameAudioState {
    /// Initializes GameAudioState from saved settings.
    pub fn new() -> Self {
        let enabled = crate::settings::get("gameaudio_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let global_path = crate::settings::get("game_path")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let jp_path = crate::settings::get("gameaudio_jp_path")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let active_region_str = crate::settings::get("gameaudio_active_region")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "global".to_string());
        let active_region = GameRegion::from_key(&active_region_str);

        let custom_key = crate::settings::get("gameaudio_key")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let output_device = crate::settings::get("gameaudio_output_device")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let volume = crate::settings::get("gameaudio_volume")
            .and_then(|v| v.as_f64().map(|f| f as f32))
            .unwrap_or(1.0f32)
            .clamp(0.0, 3.0);
        let high_volume_warning_acknowledged =
            crate::settings::get("gameaudio_high_volume_warning_acknowledged")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        let export_dir = crate::settings::get("gameaudio_export_dir")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let live_vocal_default_volume = crate::settings::get("live_vocal_default_volume")
            .and_then(|v| v.as_f64().map(|f| f as f32))
            .filter(|v| v.is_finite())
            .unwrap_or(1.0f32)
            .clamp(0.0, 1.0);

        let (tx, rx) = channel();

        let engine = if enabled {
            let dev = if output_device.is_empty() {
                None
            } else {
                Some(output_device.as_str())
            };
            let eng = AudioEngine::new_with_device(dev);
            eng.set_volume(volume);
            Some(eng)
        } else {
            None
        };

        let mut state = Self {
            enabled,
            active_region,
            global: GameAudioCatalog::new(GameRegion::Global, global_path),
            jp: GameAudioCatalog::new(GameRegion::Jp, jp_path),
            game_path: String::new(),
            custom_key,
            output_device,
            output_devices: mixer::output_device_names(),
            volume,
            live_vocal_default_volume,
            high_volume_warning_acknowledged,
            export_dir,
            detected_installs: Vec::new(),
            status: if enabled {
                GameAudioStatus::Idle
            } else {
                GameAudioStatus::Disabled
            },
            manifest_entries: Arc::new(Vec::new()),
            name_index: HashMap::new(),
            cuesheet_index: HashMap::new(),
            master: MasterData::default(),
            readiness: AudioReadinessCache::default(),
            engine,
            current_live_score: None,
            loading_live_id: None,
            is_exporting: false,
            toast_msg: None,
            last_error: None,
            tx,
            rx,
        };

        state.sync_active_views();

        if state.enabled {
            let mut any_loaded = false;
            if state.global.is_configured() {
                state.start_load_region(GameRegion::Global);
                any_loaded = true;
            }
            if state.jp.is_configured() {
                state.start_load_region(GameRegion::Jp);
                any_loaded = true;
            }
            if !any_loaded {
                state.start_detection();
            }
        }

        state
    }

    /// Synchronizes top-level convenience views with the currently active catalog.
    pub fn sync_active_views(&mut self) {
        let active = match self.active_region {
            GameRegion::Global => &self.global,
            GameRegion::Jp => &self.jp,
        };
        self.game_path = active.game_path.clone();
        self.manifest_entries = Arc::clone(&active.manifest_entries);
        self.name_index = active.name_index.clone();
        self.cuesheet_index = active.cuesheet_index.clone();
        self.master = active.master.clone();
        self.readiness = active.readiness.clone();
        self.status = if !self.enabled {
            GameAudioStatus::Disabled
        } else {
            active.status.clone()
        };
    }

    /// Returns reference to catalog for a region.
    pub fn catalog(&self, region: GameRegion) -> &GameAudioCatalog {
        match region {
            GameRegion::Global => &self.global,
            GameRegion::Jp => &self.jp,
        }
    }

    /// Returns mutable reference to catalog for a region.
    pub fn catalog_mut(&mut self, region: GameRegion) -> &mut GameAudioCatalog {
        match region {
            GameRegion::Global => &mut self.global,
            GameRegion::Jp => &mut self.jp,
        }
    }

    /// Returns reference to the active catalog.
    pub fn active_catalog(&self) -> &GameAudioCatalog {
        self.catalog(self.active_region)
    }

    /// Returns mutable reference to the active catalog.
    pub fn active_catalog_mut(&mut self) -> &mut GameAudioCatalog {
        self.catalog_mut(self.active_region)
    }

    /// Switches the active region (Global or JP) and saves to settings.
    pub fn set_active_region(&mut self, region: GameRegion) {
        if self.active_region == region {
            return;
        }
        self.active_region = region;
        crate::settings::set(
            "gameaudio_active_region",
            serde_json::Value::String(region.key().to_string()),
        );
        self.sync_active_views();
    }

    /// Builds fast O(1) asset name and cuesheet lookup indices for the active catalog.
    pub fn build_indices(&mut self) {
        self.active_catalog_mut().build_indices();
        self.sync_active_views();
    }

    /// Helper to initialize an AudioEngine honoring saved output device and volume.
    fn create_engine(&self) -> AudioEngine {
        let dev = if self.output_device.is_empty() {
            None
        } else {
            Some(self.output_device.as_str())
        };
        let eng = AudioEngine::new_with_device(dev);
        eng.set_volume(self.volume);
        eng
    }

    /// Sets enabled state and persists it to settings.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        crate::settings::set("gameaudio_enabled", serde_json::Value::Bool(enabled));
        if enabled {
            self.global.status = if self.global.is_configured() {
                GameAudioStatus::Idle
            } else {
                GameAudioStatus::Idle
            };
            self.jp.status = if self.jp.is_configured() {
                GameAudioStatus::Idle
            } else {
                GameAudioStatus::Idle
            };
            if self.engine.is_none() {
                self.engine = Some(self.create_engine());
            }
            let mut any_loaded = false;
            if self.global.is_configured() {
                self.start_load_region(GameRegion::Global);
                any_loaded = true;
            }
            if self.jp.is_configured() {
                self.start_load_region(GameRegion::Jp);
                any_loaded = true;
            }
            if !any_loaded {
                self.start_detection();
            }
        } else {
            self.global.clear();
            self.jp.clear();
            self.engine = None;
        }
        self.sync_active_views();
    }

    /// Updates output audio playback device and persists selection to settings.
    pub fn set_output_device(&mut self, name: String) {
        self.output_device = name.clone();
        if name.is_empty() {
            crate::settings::set("gameaudio_output_device", serde_json::Value::Null);
        } else {
            crate::settings::set(
                "gameaudio_output_device",
                serde_json::Value::String(name.clone()),
            );
        }

        if let Some(engine) = &mut self.engine {
            let target = if name.is_empty() {
                None
            } else {
                Some(name.as_str())
            };
            match engine.swap_output_device(target) {
                Ok(()) => {
                    let dev_name = engine.active_device().unwrap_or("Default");
                    self.toast_msg = Some(tf!("Audio output switched to {}", dev_name));
                    self.last_error = None;
                }
                Err(e) => {
                    self.last_error = Some(e);
                }
            }
        }
    }

    /// Returns the current base export directory (resolved against cwd if relative or fallback to default).
    pub fn export_base_dir(&self) -> PathBuf {
        export::resolve_export_base_dir(if self.export_dir.is_empty() {
            None
        } else {
            Some(&self.export_dir)
        })
    }

    /// Returns the resolved directory for an export category (files, jukebox, live).
    pub fn export_category_dir(&self, category: ExportCategory) -> PathBuf {
        export::resolve_category_dir(&self.export_base_dir(), category)
    }

    /// Returns the display string of the default export directory for UI hint.
    pub fn default_export_dir_string(&self) -> String {
        export::default_export_base_dir().display().to_string()
    }

    /// Updates custom export directory and persists it to settings.
    pub fn set_export_dir(&mut self, path: String) {
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            crate::settings::set("gameaudio_export_dir", serde_json::Value::Null);
            self.export_dir.clear();
        } else {
            crate::settings::set(
                "gameaudio_export_dir",
                serde_json::Value::String(trimmed.clone()),
            );
            self.export_dir = trimmed;
        }
    }

    /// Opens the current export base directory in Windows Explorer.
    pub fn open_export_dir(&self) -> Result<(), String> {
        export::open_folder_in_explorer(&self.export_base_dir())
    }

    /// Refreshes the cached list of available active audio output devices.
    pub fn refresh_output_devices(&mut self) {
        self.output_devices = mixer::output_device_names();
    }

    /// Returns the currently active audio output device name.
    pub fn active_device(&self) -> Option<&str> {
        self.engine.as_ref().and_then(|e| e.active_device())
    }

    /// Updates persistent directory path for a specific game region and persists it to settings.
    pub fn set_region_path(&mut self, region: GameRegion, path: String) {
        let trimmed = path.trim().to_string();
        let is_enabled = self.enabled;
        let cat = self.catalog_mut(region);
        cat.game_path = trimmed.clone();
        cat.readiness = AudioReadinessCache::default();
        match region {
            GameRegion::Global => {
                crate::settings::set("game_path", serde_json::Value::String(trimmed));
            }
            GameRegion::Jp => {
                if trimmed.is_empty() {
                    crate::settings::set("gameaudio_jp_path", serde_json::Value::Null);
                } else {
                    crate::settings::set("gameaudio_jp_path", serde_json::Value::String(trimmed));
                }
            }
        }
        let has_path = !cat.game_path.is_empty();
        if is_enabled && has_path {
            self.start_load_region(region);
        } else {
            let cat = self.catalog_mut(region);
            cat.status = if is_enabled {
                GameAudioStatus::Idle
            } else {
                GameAudioStatus::Disabled
            };
        }
        self.sync_active_views();
    }

    /// Updates the game persistent directory path for the active region and persists it to settings.
    pub fn set_game_path(&mut self, path: String) {
        self.set_region_path(self.active_region, path);
    }

    /// Updates custom hex encryption key and persists it to settings.
    pub fn set_custom_key(&mut self, key: String) {
        self.custom_key = key.clone();
        if key.is_empty() {
            crate::settings::set("gameaudio_key", serde_json::Value::Null);
        } else {
            crate::settings::set("gameaudio_key", serde_json::Value::String(key));
        }
        if self.enabled {
            if self.global.is_configured() {
                self.start_load_region(GameRegion::Global);
            }
            if self.jp.is_configured() {
                self.start_load_region(GameRegion::Jp);
            }
        }
    }

    /// Starts scanning standard locations for game installations in a background thread.
    pub fn start_detection(&mut self) {
        let tx = self.tx.clone();
        self.status = GameAudioStatus::Scanning;

        thread::spawn(move || {
            let installs = gamedir::detect_installations();
            let _ = tx.send(GameAudioMsg::InstallationsDetected(installs));
        });
    }

    /// Starts loading manifest and master databases for the active region in a background thread.
    pub fn start_load(&mut self) {
        self.start_load_region(self.active_region);
    }

    /// Starts loading manifest and master databases for a specific region in a background thread.
    pub fn start_load_region(&mut self, region: GameRegion) {
        let cat = self.catalog_mut(region);
        let path = PathBuf::from(&cat.game_path);
        if path.as_os_str().is_empty() {
            cat.status = GameAudioStatus::Idle;
            self.sync_active_views();
            return;
        }

        cat.load_generation += 1;
        let gen_id = cat.load_generation;
        cat.status = GameAudioStatus::Loading;
        let custom_key = if self.custom_key.is_empty() {
            None
        } else {
            Some(self.custom_key.clone())
        };

        let tx = self.tx.clone();
        thread::spawn(move || {
            let meta_file = path.join("meta");
            let master_file = path.join("master").join("master.mdb");

            // Filter for audio assets, music scores, jacket icons and chara icons to keep loading fast and memory light
            let manifest_res = meta::load_manifest(
                &meta_file,
                custom_key.as_deref(),
                Some(
                    "n LIKE 'sound/%' OR n LIKE 'live/musicscores/%' OR n LIKE '%jacket%' OR n LIKE '%chr_icon%'",
                ),
            );

            let master_res = master::load_master_data(&master_file);

            let _ = tx.send(GameAudioMsg::DataLoaded {
                region,
                generation: gen_id,
                manifest: manifest_res,
                master: master_res,
            });
        });
        self.sync_active_views();
    }

    /// Decodes an audio file in background and starts playback.
    pub fn play_file(&mut self, file_path: PathBuf, cue_index: usize) {
        if self.engine.is_none() {
            self.engine = Some(self.create_engine());
        }

        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = decode_audio_file(&file_path, cue_index);
            let _ = tx.send(GameAudioMsg::AudioDecoded {
                track: res,
                autoplay: true,
            });
        });
    }

    /// Decodes an asset entry from game storage and plays it.
    pub fn play_manifest_entry(&mut self, entry: &ManifestEntry, cue_index: usize) {
        let game_dir = Path::new(&self.game_path);
        let path = entry.file_path(game_dir);
        self.play_file(path, cue_index);
    }

    /// Finds the companion AWB manifest entry for an ACB file if it exists in the active catalog.
    pub fn find_companion_awb(&self, acb_name: &str) -> Option<&ManifestEntry> {
        self.active_catalog().find_companion_awb(acb_name)
    }

    /// Inspects track cues inside a manifest entry using active game directory.
    pub fn inspect_entry_cues(&self, entry: &ManifestEntry) -> Result<Vec<CueInfo>, String> {
        self.active_catalog().inspect_entry_cues(entry)
    }

    /// Finds the manifest entry for a cuesheet name in the active catalog.
    pub fn find_cuesheet_entry(&self, cuesheet_name: &str) -> Option<&ManifestEntry> {
        self.active_catalog().find_cuesheet_entry(cuesheet_name)
    }

    /// Decodes a manifest asset entry with an optional custom title and starts playback.
    pub fn play_manifest_cue_with_title(
        &mut self,
        entry: &ManifestEntry,
        cue_index: usize,
        custom_title: Option<String>,
    ) {
        if self.engine.is_none() {
            self.engine = Some(self.create_engine());
        }

        let game_dir = PathBuf::from(&self.game_path);
        let entry_clone = entry.clone();
        let companion_awb = self.find_companion_awb(&entry.name).cloned();
        let tx = self.tx.clone();

        thread::spawn(move || {
            let mut res = (|| -> Result<DecodedAudio, String> {
                if entry_clone.name.ends_with(".acb") {
                    let prep_acb =
                        mixer::prepare_acb_file(&game_dir, &entry_clone, companion_awb.as_ref())?;
                    mixer::decode_acb_file(&prep_acb, cue_index)
                } else {
                    let path = entry_clone.file_path(&game_dir);
                    mixer::decode_audio_file(&path, cue_index)
                }
            })();
            if let Ok(audio) = &mut res {
                if let Some(t) = custom_title {
                    audio.title = t;
                }
            }
            let _ = tx.send(GameAudioMsg::AudioDecoded {
                track: res,
                autoplay: true,
            });
        });
    }

    /// Decodes a manifest asset entry (preparing companion AWB if needed) and starts playback.
    pub fn play_manifest_cue(&mut self, entry: &ManifestEntry, cue_index: usize) {
        self.play_manifest_cue_with_title(entry, cue_index, None);
    }

    /// Plays a Jukebox music track with short/game-size fallback logic.
    pub fn play_jukebox_song(
        &mut self,
        song: &master::JukeboxMusicData,
        preferred: JukeboxVersion,
    ) {
        let game_dir = PathBuf::from(&self.game_path);

        let short_entry = song
            .bgm_cuesheet_name_short
            .as_deref()
            .and_then(|cs| self.find_cuesheet_entry(cs))
            .cloned();
        let short_ready = short_entry
            .as_ref()
            .map(|e| e.exists_on_disk(&game_dir))
            .unwrap_or(false);

        let game_entry = song
            .bgm_cuesheet_name_gamesize
            .as_deref()
            .and_then(|cs| self.find_cuesheet_entry(cs))
            .cloned();
        let game_ready = game_entry
            .as_ref()
            .map(|e| e.exists_on_disk(&game_dir))
            .unwrap_or(false);

        match preferred {
            JukeboxVersion::Short => {
                if short_ready {
                    if let Some(entry) = short_entry {
                        let title = format!("{} (Short)", song.title);
                        self.play_manifest_cue_with_title(&entry, 0, Some(title));
                    }
                } else if game_ready {
                    if let Some(entry) = game_entry {
                        self.toast_msg = Some(
                            t!("Short version not downloaded. Playing Game-size version instead.")
                                .to_string(),
                        );
                        let title = format!("{} (Game-size)", song.title);
                        self.play_manifest_cue_with_title(&entry, 0, Some(title));
                    }
                } else {
                    self.toast_msg =
                        Some(t!("Audio file is not downloaded in game yet.").to_string());
                }
            }
            JukeboxVersion::GameSize => {
                if game_ready {
                    if let Some(entry) = game_entry {
                        let title = format!("{} (Game-size)", song.title);
                        self.play_manifest_cue_with_title(&entry, 0, Some(title));
                    }
                } else if short_ready {
                    if let Some(entry) = short_entry {
                        self.toast_msg = Some(
                            t!("Game-size version not downloaded. Playing Short version instead.")
                                .to_string(),
                        );
                        let title = format!("{} (Short)", song.title);
                        self.play_manifest_cue_with_title(&entry, 0, Some(title));
                    }
                } else {
                    self.toast_msg =
                        Some(t!("Audio file is not downloaded in game yet.").to_string());
                }
            }
        }
    }

    /// Exports a Jukebox song version to a WAV file.
    pub fn export_jukebox_song_wav(
        &mut self,
        song: &master::JukeboxMusicData,
        version: JukeboxVersion,
    ) {
        let cuesheet = match version {
            JukeboxVersion::Short => song.bgm_cuesheet_name_short.as_deref(),
            JukeboxVersion::GameSize => song.bgm_cuesheet_name_gamesize.as_deref(),
        };

        let ver_suffix = match version {
            JukeboxVersion::Short => "Short",
            JukeboxVersion::GameSize => "GameSize",
        };

        let entry = match cuesheet
            .and_then(|cs| self.find_cuesheet_entry(cs))
            .cloned()
        {
            Some(e) => e,
            None => {
                self.toast_msg = Some(t!("Cuesheet not found in manifest.").to_string());
                return;
            }
        };

        let game_dir = PathBuf::from(&self.game_path);
        if !entry.exists_on_disk(&game_dir) {
            self.toast_msg =
                Some(t!("Cannot export: file is not downloaded in game yet.").to_string());
            return;
        }

        // Clean filename for safety across operating systems
        let safe_title: String = song
            .title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let safe_title = safe_title.trim();

        let export_dir = self.export_category_dir(ExportCategory::Jukebox);
        let out_path = export_dir.join(format!("{safe_title}_{ver_suffix}.wav"));

        self.export_manifest_cue_wav(&entry, 0, out_path);
    }

    /// Exports a specific cue from a manifest entry to a WAV file.
    pub fn export_manifest_cue_wav(
        &mut self,
        entry: &ManifestEntry,
        cue_index: usize,
        out_path: PathBuf,
    ) {
        self.is_exporting = true;
        let game_dir = PathBuf::from(&self.game_path);
        let entry_clone = entry.clone();
        let companion_awb = self.find_companion_awb(&entry.name).cloned();
        let tx = self.tx.clone();

        thread::spawn(move || {
            let res = (|| -> Result<PathBuf, String> {
                let audio = if entry_clone.name.ends_with(".acb") {
                    let prep_acb =
                        mixer::prepare_acb_file(&game_dir, &entry_clone, companion_awb.as_ref())?;
                    mixer::decode_acb_file(&prep_acb, cue_index)?
                } else {
                    let path = entry_clone.file_path(&game_dir);
                    mixer::decode_audio_file(&path, cue_index)?
                };
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                save_wav_file(&out_path, audio.channels, audio.sample_rate, &audio.samples)
                    .map_err(|e| format!("WAV write error: {e}"))?;
                Ok(out_path)
            })();
            let _ = tx.send(GameAudioMsg::WavExported { result: res });
        });
    }

    /// Exports a decoded track to a WAV file in a background thread.
    pub fn export_file_wav(&mut self, file_path: PathBuf, cue_index: usize, out_path: PathBuf) {
        self.is_exporting = true;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = (|| -> Result<PathBuf, String> {
                let audio = decode_audio_file(&file_path, cue_index)?;
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                save_wav_file(&out_path, audio.channels, audio.sample_rate, &audio.samples)
                    .map_err(|e| format!("WAV write error: {e}"))?;
                Ok(out_path)
            })();
            let _ = tx.send(GameAudioMsg::WavExported { result: res });
        });
    }

    /// Resumes playback.
    pub fn play(&self) {
        if let Some(engine) = &self.engine {
            engine.play();
        }
    }

    /// Pauses playback.
    pub fn pause(&self) {
        if let Some(engine) = &self.engine {
            engine.pause();
        }
    }

    /// Stops playback and rewinds to 0.
    pub fn stop(&mut self) {
        self.loading_live_id = None;
        if let Some(engine) = &self.engine {
            engine.stop();
        }
    }

    /// Seeks to target position in seconds.
    pub fn seek(&self, position_secs: f32) {
        if let Some(engine) = &self.engine {
            engine.seek(position_secs);
        }
    }

    /// Adjusts playback volume in range [0.0, 3.0] and persists to settings.
    pub fn set_volume(&mut self, volume: f32) {
        let clamped = volume.clamp(0.0, 3.0);
        self.volume = clamped;
        crate::settings::set("gameaudio_volume", serde_json::json!(clamped));
        if let Some(engine) = &self.engine {
            engine.set_volume(clamped);
        }
    }

    /// Adjusts default vocal volume in range [0.0, 1.0] and persists to settings.
    pub fn set_live_vocal_default_volume(&mut self, volume: f32) {
        let clamped = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.live_vocal_default_volume = clamped;
        crate::settings::set("live_vocal_default_volume", serde_json::json!(clamped));
    }

    /// Acknowledges high volume (>200%) warning and persists to settings.
    pub fn acknowledge_high_volume_warning(&mut self) {
        self.high_volume_warning_acknowledged = true;
        crate::settings::set(
            "gameaudio_high_volume_warning_acknowledged",
            serde_json::json!(true),
        );
    }

    /// Toggles seamless track looping.
    pub fn set_loop(&self, enabled: bool) {
        if let Some(engine) = &self.engine {
            engine.set_loop(enabled);
        }
    }

    /// Returns current playback state (Stopped, Playing, Paused).
    pub fn playback_state(&self) -> PlaybackState {
        self.engine
            .as_ref()
            .map(|e| e.status().state())
            .unwrap_or(PlaybackState::Stopped)
    }

    /// Returns current playback position in seconds.
    pub fn position_secs(&self) -> f32 {
        self.engine
            .as_ref()
            .map(|e| e.status().position_secs())
            .unwrap_or(0.0)
    }

    /// Returns current track duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        self.engine
            .as_ref()
            .map(|e| e.status().duration_secs())
            .unwrap_or(0.0)
    }

    /// Returns current volume setting [0.0, 3.0].
    pub fn volume(&self) -> f32 {
        self.engine
            .as_ref()
            .map(|e| e.status().volume())
            .unwrap_or(self.volume)
    }

    /// Returns active track title if one is loaded.
    pub fn current_title(&self) -> Option<String> {
        self.engine.as_ref().and_then(|e| e.status().track_title())
    }

    /// Returns whether seamless looping is currently enabled.
    pub fn is_loop(&self) -> bool {
        self.engine
            .as_ref()
            .map(|e| e.status().is_loop())
            .unwrap_or(false)
    }

    /// Polls messages from background threads. Call once per frame in the main loop.
    pub fn poll(&mut self) {
        let mut events = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            events.push(msg);
        }

        let mut auto_global = None;
        let mut auto_jp = None;

        for msg in events {
            match msg {
                GameAudioMsg::InstallationsDetected(installs) => {
                    if self.global.game_path.is_empty() {
                        if let Some(first_valid) = installs
                            .iter()
                            .find(|i| i.region == GameRegion::Global && i.is_valid())
                        {
                            auto_global = Some(first_valid.path.to_string_lossy().into_owned());
                        }
                    }
                    if self.jp.game_path.is_empty() {
                        if let Some(first_valid) = installs
                            .iter()
                            .find(|i| i.region == GameRegion::Jp && i.is_valid())
                        {
                            auto_jp = Some(first_valid.path.to_string_lossy().into_owned());
                        }
                    }
                    self.detected_installs = installs;
                    if self.status == GameAudioStatus::Scanning {
                        self.status = GameAudioStatus::Idle;
                    }
                }
                GameAudioMsg::DataLoaded {
                    region,
                    generation,
                    manifest,
                    master,
                } => {
                    let cat = self.catalog_mut(region);
                    if cat.load_generation == generation {
                        match (manifest, master) {
                            (Ok(m), Ok(mast)) => {
                                cat.manifest_entries = Arc::new(m);
                                cat.master = mast;
                                cat.build_indices();
                                cat.status = GameAudioStatus::Ready;
                                cat.last_error = None;
                                self.start_readiness_scan_region(region);
                            }
                            (Err(e), _) => {
                                let err_str = format!("Manifest error ({region:?}): {e}");
                                cat.status = GameAudioStatus::Error(err_str.clone());
                                cat.last_error = Some(err_str);
                            }
                            (_, Err(e)) => {
                                let err_str = format!("Master DB error ({region:?}): {e}");
                                cat.status = GameAudioStatus::Error(err_str.clone());
                                cat.last_error = Some(err_str);
                            }
                        }
                        self.sync_active_views();
                    }
                }
                GameAudioMsg::ReadinessScanned {
                    region,
                    generation,
                    cache,
                } => {
                    let cat = self.catalog_mut(region);
                    if cat.scan_generation == generation {
                        cat.readiness = cache;
                        self.sync_active_views();
                    }
                }
                GameAudioMsg::AudioDecoded { track, autoplay } => {
                    self.current_live_score = None;
                    match track {
                        Ok(audio) => {
                            if let Some(engine) = &self.engine {
                                engine.load_track(audio, autoplay);
                            }
                        }
                        Err(e) => {
                            self.last_error = Some(e);
                        }
                    }
                }
                GameAudioMsg::LiveDecoded {
                    music_id,
                    live,
                    autoplay,
                } => {
                    let is_expected = self.loading_live_id == Some(music_id);
                    if is_expected {
                        self.loading_live_id = None;
                    }
                    match live {
                        Ok(live_box) => {
                            if is_expected {
                                self.current_live_score = Some(live_box.score.clone());
                                if let Some(engine) = &self.engine {
                                    engine.load_live(*live_box, autoplay);
                                }
                            }
                        }
                        Err(e) => {
                            if is_expected {
                                self.last_error = Some(e.clone());
                                self.toast_msg = Some(e);
                            }
                        }
                    }
                }
                GameAudioMsg::WavExported { result } => {
                    self.is_exporting = false;
                    match result {
                        Ok(p) => {
                            self.toast_msg = Some(tf!("Exported: {}", p.display()));
                        }
                        Err(e) => {
                            self.last_error = Some(e);
                        }
                    }
                }
            }
        }

        if let Some(p) = auto_global {
            self.set_region_path(GameRegion::Global, p);
        }
        if let Some(p) = auto_jp {
            self.set_region_path(GameRegion::Jp, p);
        }
    }

    /// Finds the instrumental (oke) audio asset entry for a live music track in the active catalog.
    pub fn find_live_oke_entry(&self, music_id: i32, sfx: u8) -> Option<ManifestEntry> {
        self.active_catalog().find_live_oke_entry(music_id, sfx)
    }

    /// Finds the vocal track asset entry for a specific character in the active catalog.
    pub fn find_live_chara_entry(&self, music_id: i32, chara_id: i32) -> Option<ManifestEntry> {
        self.active_catalog()
            .find_live_chara_entry(music_id, chara_id)
    }

    /// Checks whether the base live resources (oke track) exist on disk for the active catalog.
    pub fn is_live_ready(&self, music_id: i32) -> bool {
        self.active_catalog().is_live_ready(music_id)
    }

    /// Checks whether a specific character's vocal track is downloaded on disk in the active catalog.
    pub fn is_live_chara_ready(&self, music_id: i32, chara_id: i32) -> bool {
        self.active_catalog()
            .is_live_chara_ready(music_id, chara_id)
    }

    /// Checks whether short and game-size versions of a jukebox song exist on disk in the active catalog.
    pub fn is_jukebox_ready(&self, music_id: i32) -> (bool, bool) {
        self.active_catalog().is_jukebox_ready(music_id)
    }

    /// Checks whether a manifest asset entry exists on disk from precomputed cache in the active catalog.
    pub fn is_entry_on_disk(&self, entry_id: i64) -> bool {
        self.active_catalog().is_entry_on_disk(entry_id)
    }

    /// Starts background thread to scan filesystem and cache asset availability for active region.
    pub fn start_readiness_scan(&mut self) {
        self.start_readiness_scan_region(self.active_region);
    }

    /// Starts background thread to scan filesystem and cache asset availability for a specific region.
    pub fn start_readiness_scan_region(&mut self, region: GameRegion) {
        let cat = self.catalog_mut(region);
        if cat.game_path.is_empty() || cat.manifest_entries.is_empty() || cat.readiness.is_scanning
        {
            return;
        }

        cat.scan_generation += 1;
        let gen_id = cat.scan_generation;
        cat.readiness.is_scanning = true;
        let manifest = Arc::clone(&cat.manifest_entries);
        let master = cat.master.clone();
        let cuesheet_index = cat.cuesheet_index.clone();
        let name_index = cat.name_index.clone();
        let game_dir = PathBuf::from(&cat.game_path);
        let tx = self.tx.clone();

        thread::spawn(move || {
            let mut on_disk = HashSet::with_capacity(manifest.len());
            for entry in manifest.iter() {
                if entry.state == 1 || entry.exists_on_disk(&game_dir) {
                    on_disk.insert(entry.id);
                }
            }

            // Precompute Jukebox readiness
            let mut juke_ready = HashMap::with_capacity(master.jukebox_songs.len());
            for s in &master.jukebox_songs {
                let s_ok = s
                    .bgm_cuesheet_name_short
                    .as_deref()
                    .and_then(|cs| {
                        let cs_lower = cs.trim().to_lowercase();
                        cuesheet_index
                            .get(&cs_lower)
                            .and_then(|&(awb, acb)| awb.or(acb))
                    })
                    .map(|idx| on_disk.contains(&manifest[idx].id))
                    .unwrap_or(false);

                let g_ok = s
                    .bgm_cuesheet_name_gamesize
                    .as_deref()
                    .and_then(|cs| {
                        let cs_lower = cs.trim().to_lowercase();
                        cuesheet_index
                            .get(&cs_lower)
                            .and_then(|&(awb, acb)| awb.or(acb))
                    })
                    .map(|idx| on_disk.contains(&manifest[idx].id))
                    .unwrap_or(false);

                juke_ready.insert(s.music_id, (s_ok, g_ok));
            }

            // Precompute Live readiness (oke track)
            let mut live_ready = HashSet::with_capacity(master.live_songs.len());
            for s in &master.live_songs {
                let p1 = format!("sound/l/snd_bgm_live_{}_oke_01.awb", s.music_id);
                let p2 = format!("sound/l/snd_bgm_live_{:04}_oke_01.awb", s.music_id);
                let p3 = format!("sound/l/snd_bgm_live_{}_oke_01_01.awb", s.music_id);
                let p4 = format!("sound/l/snd_bgm_live_{:04}_oke_01_01.awb", s.music_id);
                let p5 = format!("sound/l/snd_bgm_live_{}_oke_02.awb", s.music_id);
                let p6 = format!("sound/l/snd_bgm_live_{:04}_oke_02.awb", s.music_id);

                let mut is_ok = false;
                for p in &[&p1, &p2, &p3, &p4, &p5, &p6] {
                    if let Some(&idx) = name_index.get(*p) {
                        if on_disk.contains(&manifest[idx].id) {
                            is_ok = true;
                            break;
                        }
                    }
                }
                if is_ok {
                    live_ready.insert(s.music_id);
                }
            }

            // Precompute Live character vocal readiness
            let mut live_chara_ready = HashMap::with_capacity(master.live_permissions.len());
            for p in &master.live_permissions {
                let p1 = format!(
                    "sound/l/snd_bgm_live_{}_chara_{}_01.awb",
                    p.music_id, p.chara_id
                );
                let p2 = format!(
                    "sound/l/snd_bgm_live_{:04}_chara_{}_01.awb",
                    p.music_id, p.chara_id
                );
                let p3 = format!(
                    "sound/l/snd_bgm_live_{}_chara_{:04}_01.awb",
                    p.music_id, p.chara_id
                );
                let p4 = format!(
                    "sound/l/snd_bgm_live_{:04}_chara_{:04}_01.awb",
                    p.music_id, p.chara_id
                );
                let p5 = format!(
                    "sound/l/snd_bgm_live_{}_chara_{}.awb",
                    p.music_id, p.chara_id
                );

                let mut is_ok = false;
                for pat in &[&p1, &p2, &p3, &p4, &p5] {
                    if let Some(&idx) = name_index.get(*pat) {
                        if on_disk.contains(&manifest[idx].id) {
                            is_ok = true;
                            break;
                        }
                    }
                }
                live_chara_ready.insert((p.music_id, p.chara_id), is_ok);
            }

            // Voice readiness for characters discovered from the manifest
            // itself, covering lives whose permission table is missing or
            // out of date.
            for entry in manifest.iter() {
                if let Some((mid, cid)) = parse_live_chara_entry_name(&entry.name) {
                    let ok = on_disk.contains(&entry.id);
                    live_chara_ready
                        .entry((mid, cid))
                        .and_modify(|v| *v = *v || ok)
                        .or_insert(ok);
                }
            }

            let cache = AudioReadinessCache {
                on_disk,
                juke_ready,
                live_ready,
                live_chara_ready,
                is_scanning: false,
                is_ready: true,
            };

            let _ = tx.send(GameAudioMsg::ReadinessScanned {
                region,
                generation: gen_id,
                cache,
            });
        });
        self.sync_active_views();
    }

    /// Updates real-time mixer controls for the active live performance.
    ///
    /// Performer controls are keyed by stage slot index in lineup order.
    pub fn update_live_mixer(
        &self,
        mute_oke: bool,
        mute_vocals: bool,
        performer_controls: Vec<PerformerControlUpdate>,
    ) {
        if let Some(engine) = &self.engine {
            engine.update_live_controls(mute_oke, mute_vocals, performer_controls);
        }
    }

    /// Starts asynchronous decoding and playback of a multi-track live performance.
    pub fn play_live_song(
        &mut self,
        music_id: i32,
        title: String,
        sfx: u8,
        lineup: Vec<PerformerSlotConfig>,
        mute_oke: bool,
        mute_vocals: bool,
        autoplay: bool,
    ) {
        self.loading_live_id = Some(music_id);
        if self.engine.is_none() {
            self.engine = Some(self.create_engine());
        }

        let game_dir = PathBuf::from(&self.game_path);
        let oke_entry = self.find_live_oke_entry(music_id, sfx);
        let manifest = Arc::clone(&self.manifest_entries);
        let tx = self.tx.clone();

        thread::spawn(move || {
            let oke_e = match oke_entry {
                Some(e) => e,
                None => {
                    let _ = tx.send(GameAudioMsg::LiveDecoded {
                        music_id,
                        live: Err(format!("Oke audio track not found for Live #{music_id}")),
                        autoplay: false,
                    });
                    return;
                }
            };

            let oke_path = oke_e.file_path(&game_dir);
            if !oke_path.is_file() {
                let _ = tx.send(GameAudioMsg::LiveDecoded {
                    music_id,
                    live: Err(format!(
                        "Oke audio file not downloaded on disk: {}",
                        oke_e.name
                    )),
                    autoplay: false,
                });
                return;
            }

            let oke_audio = match decode_audio_file(&oke_path, 0) {
                Ok(a) => a,
                Err(e) => {
                    let _ = tx.send(GameAudioMsg::LiveDecoded {
                        music_id,
                        live: Err(format!("Failed to decode Oke audio: {e}")),
                        autoplay: false,
                    });
                    return;
                }
            };

            let score = unity::load_live_score(&game_dir, music_id, lineup.len(), &manifest);

            let mut performers = Vec::with_capacity(lineup.len());
            for slot in lineup {
                let chara_entry =
                    find_live_chara_manifest_entry(&manifest, music_id, slot.chara_id);

                let (main_audio, extra_audio) = if let Some(e) = chara_entry {
                    let chara_path = e.file_path(&game_dir);
                    if chara_path.is_file() {
                        let main = decode_audio_file(&chara_path, 0).ok();
                        let extra = decode_audio_file(&chara_path, 1).ok();
                        (main, extra)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                performers.push(LivePerformerAudio {
                    chara_id: slot.chara_id,
                    name: slot.name,
                    main_audio,
                    extra_audio,
                    mode: slot.mode,
                    solo: slot.solo,
                    volume: slot.volume,
                    main_vocal_enabled: slot.main_vocal_enabled,
                });
            }

            let live = LivePerformance {
                music_id,
                title,
                oke_audio,
                performers,
                score,
                mute_oke,
                mute_vocals,
            };

            let _ = tx.send(GameAudioMsg::LiveDecoded {
                music_id,
                live: Ok(Box::new(live)),
                autoplay,
            });
        });
    }

    /// Asynchronously exports a multi-track live mix to a WAV file.
    pub fn export_live_song_wav(
        &mut self,
        music_id: i32,
        title: String,
        sfx: u8,
        lineup: Vec<PerformerSlotConfig>,
        mute_oke: bool,
        mute_vocals: bool,
    ) {
        self.is_exporting = true;
        let game_dir = PathBuf::from(&self.game_path);
        let oke_entry = self.find_live_oke_entry(music_id, sfx);
        let manifest = Arc::clone(&self.manifest_entries);
        let tx = self.tx.clone();

        // Capture resolved destination path before spawning background worker
        let export_dir = self.export_category_dir(ExportCategory::Live);
        let safe_title = title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let out_path = export_dir.join(format!("{safe_title}_Live_{music_id}.wav"));

        thread::spawn(move || {
            let oke_e = match oke_entry {
                Some(e) => e,
                None => {
                    let _ = tx.send(GameAudioMsg::WavExported {
                        result: Err(format!("Oke audio track not found for Live #{music_id}")),
                    });
                    return;
                }
            };

            let oke_path = oke_e.file_path(&game_dir);
            if !oke_path.is_file() {
                let _ = tx.send(GameAudioMsg::WavExported {
                    result: Err(format!("Oke file not downloaded: {}", oke_e.name)),
                });
                return;
            }

            let oke_audio = match decode_audio_file(&oke_path, 0) {
                Ok(a) => a,
                Err(e) => {
                    let _ = tx.send(GameAudioMsg::WavExported {
                        result: Err(format!("Failed to decode Oke audio: {e}")),
                    });
                    return;
                }
            };

            let score = unity::load_live_score(&game_dir, music_id, lineup.len(), &manifest);

            let mut performers = Vec::with_capacity(lineup.len());
            for slot in lineup {
                let chara_entry =
                    find_live_chara_manifest_entry(&manifest, music_id, slot.chara_id);

                let (main_audio, extra_audio) = if let Some(e) = chara_entry {
                    let chara_path = e.file_path(&game_dir);
                    if chara_path.is_file() {
                        let main = decode_audio_file(&chara_path, 0).ok();
                        let extra = decode_audio_file(&chara_path, 1).ok();
                        (main, extra)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                performers.push(LivePerformerAudio {
                    chara_id: slot.chara_id,
                    name: slot.name,
                    main_audio,
                    extra_audio,
                    mode: slot.mode,
                    solo: slot.solo,
                    volume: slot.volume,
                    main_vocal_enabled: slot.main_vocal_enabled,
                });
            }

            let mut live = LivePerformance {
                music_id,
                title: title.clone(),
                oke_audio,
                performers,
                score,
                mute_oke,
                mute_vocals,
            };

            let wav_bytes = match render_live_mix_pcm16(&mut live) {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(GameAudioMsg::WavExported {
                        result: Err(format!("Failed to render live mix: {e}")),
                    });
                    return;
                }
            };

            let _ = std::fs::create_dir_all(&export_dir);
            let res = std::fs::write(&out_path, wav_bytes)
                .map(|_| out_path)
                .map_err(|e| format!("Failed to write WAV file: {e}"));

            let _ = tx.send(GameAudioMsg::WavExported { result: res });
        });
    }
}

/// Slot configuration for a stage performer during live playback.
#[derive(Debug, Clone)]
pub struct PerformerSlotConfig {
    pub chara_id: i32,
    pub name: String,
    pub mode: SingingMode,
    pub solo: bool,
    pub volume: f32,
    pub main_vocal_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_entry(id: i64, name: &str, state: i32) -> ManifestEntry {
        ManifestEntry {
            id,
            name: name.to_string(),
            dependencies: None,
            group: 0,
            length: 1,
            checksum: 0,
            hash_name: format!("hash{id}"),
            kind: 0,
            state,
            priority: 0,
        }
    }

    fn catalog_with(
        manifest: Vec<ManifestEntry>,
        perms: Vec<(i32, i32)>,
        chara_ready: Vec<((i32, i32), bool)>,
    ) -> GameAudioCatalog {
        let mut cat = GameAudioCatalog::new(GameRegion::Global, String::new());
        cat.manifest_entries = Arc::new(manifest);
        for (mid, cid) in perms {
            cat.master.live_perm_index.entry(mid).or_default().push(cid);
            cat.master
                .chara_names
                .entry(cid)
                .or_insert_with(|| format!("Chara {cid}"));
        }
        cat.readiness.is_ready = true;
        for (key, ok) in chara_ready {
            cat.readiness.live_chara_ready.insert(key, ok);
        }
        cat
    }

    #[test]
    fn parses_live_chara_entry_name_variants() {
        assert_eq!(
            parse_live_chara_entry_name("sound/l/snd_bgm_live_9_chara_1001_01.awb"),
            Some((9, 1001))
        );
        assert_eq!(
            parse_live_chara_entry_name("sound/l/snd_bgm_live_0090_chara_0101_01.awb"),
            Some((90, 101))
        );
        assert_eq!(
            parse_live_chara_entry_name("sound/l/snd_bgm_live_129_chara_1077.awb"),
            Some((129, 1077))
        );
        assert_eq!(
            parse_live_chara_entry_name("sound/l/snd_bgm_live_9_oke_01.awb"),
            None
        );
        assert_eq!(parse_live_chara_entry_name("sound/b/bgm_100.awb"), None);
    }

    #[test]
    fn live_candidates_merge_permissions_and_manifest_without_duplicates() {
        // Permission list first (table order), then manifest-only characters
        // sorted by discovery; no fabricated ids when both sources are empty.
        let cat = catalog_with(
            vec![
                manifest_entry(1, "sound/l/snd_bgm_live_9_chara_1002_01.awb", 1),
                manifest_entry(2, "sound/l/snd_bgm_live_9_chara_1001_01.awb", 0),
                manifest_entry(3, "sound/l/snd_bgm_live_8_chara_1095_01.awb", 1),
            ],
            vec![(9, 1001)],
            vec![((9, 1001), true), ((9, 1002), false), ((8, 1095), true)],
        );

        let cands = cat.live_chara_candidates(9);
        assert_eq!(cands.len(), 2);
        assert_eq!(
            cands[0],
            LiveCharaCandidate {
                chara_id: 1001,
                voice_ready: true
            }
        );
        assert_eq!(
            cands[1],
            LiveCharaCandidate {
                chara_id: 1002,
                voice_ready: false
            }
        );
        assert_eq!(
            cands[0].chara_id, 1001,
            "permission entries keep table order"
        );

        let empty = catalog_with(vec![], vec![], vec![]);
        assert!(empty.live_chara_candidates(9).is_empty());
    }

    #[test]
    fn chara_entry_lookup_resolves_padded_and_unpadded_ids() {
        let manifest = vec![
            manifest_entry(1, "sound/l/snd_bgm_live_0090_chara_0101_01.awb", 1),
            manifest_entry(2, "sound/l/snd_bgm_live_129_chara_1077.awb", 1),
        ];
        assert_eq!(
            find_live_chara_manifest_entry(&manifest, 90, 101).map(|e| e.name.clone()),
            Some("sound/l/snd_bgm_live_0090_chara_0101_01.awb".to_string())
        );
        assert_eq!(
            find_live_chara_manifest_entry(&manifest, 129, 1077).map(|e| e.name.clone()),
            Some("sound/l/snd_bgm_live_129_chara_1077.awb".to_string())
        );
        assert!(find_live_chara_manifest_entry(&manifest, 9, 1001).is_none());
    }
}
