//! Music tab: game audio library (Files browser, Jukebox, and Live music player).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Margin, RichText, Stroke, Vec2, vec2,
};
use egui_extras::{Column, TableBuilder};

use crate::App;
use crate::collection_kit as kit;
use crate::gameaudio::mixer::{CueInfo, PlaybackState, SingingMode};
use crate::gameaudio::{
    JukeboxVersion, LyricsLoadState, ManifestEntry, PerformerControlUpdate, PerformerSlotConfig,
};
use crate::theme::pal;
use crate::ui_util::{card, empty_state, primary_button, segmented, semibold};

/// The sub-tabs under the Music tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MusicSubTab {
    Files,
    Jukebox,
    Live,
}

/// Request sent to the background filtering thread.
struct FilterRequest {
    entries: Arc<Vec<ManifestEntry>>,
    search: String,
    category: String,
    only_downloaded: bool,
    game_dir: PathBuf,
}

/// Request sent to background cue inspection thread.
struct CueInspectRequest {
    entry: ManifestEntry,
    companion_awb: Option<ManifestEntry>,
    game_dir: PathBuf,
}

/// State for the Music tab.
pub(crate) struct MusicTabState {
    pub sub_tab: MusicSubTab,
    pub search: String,
    pub category: String,
    pub only_downloaded: bool,
    pub selected_entry: Option<ManifestEntry>,
    pub cues: Option<Vec<CueInfo>>,
    pub is_loading_cues: bool,
    pub cues_error: Option<String>,
    pub filtered_entries: Vec<ManifestEntry>,
    pub is_filtering: bool,
    pub juke_search: String,
    pub juke_only_downloaded: bool,
    pub juke_hide_hidden: bool,
    pub juke_selected_id: Option<i32>,
    pub juke_version: JukeboxVersion,
    pub live_search: String,
    pub live_only_downloaded: bool,
    pub live_sidebar_open: bool,
    pub live_selected_id: Option<i32>,
    pub live_sfx: u8,
    pub live_mute_oke: bool,
    pub live_mute_vocals: bool,
    pub live_vocal_volume: f32,
    pub live_show_lyrics: bool,
    pub live_lyrics_anchor: Option<(i32, Option<usize>)>,
    pub live_lineup: Vec<PerformerSlotConfig>,
    pub textures: crate::gameaudio::TextureManager,
    last_filter_key: (String, String, bool, usize),
    tx_filter: Sender<FilterRequest>,
    rx_filter: Receiver<Vec<ManifestEntry>>,
    tx_cue: Sender<CueInspectRequest>,
    rx_cue: Receiver<(i64, Result<Vec<CueInfo>, String>)>,
}

impl Default for MusicTabState {
    fn default() -> Self {
        let (tx_filter, rx_filter_worker) = channel::<FilterRequest>();
        let (tx_filter_worker_out, rx_filter) = channel::<Vec<ManifestEntry>>();

        thread::spawn(move || {
            while let Ok(req) = rx_filter_worker.recv() {
                let search_lower = req.search.trim().to_lowercase();
                let cat = req.category.as_str();
                let only_dl = req.only_downloaded;
                let game_dir = req.game_dir;

                // Collect set of companion AWB names to avoid duplicate rows for ACB+AWB pairs
                let mut companion_awbs = std::collections::HashSet::new();
                for e in req.entries.iter() {
                    if let Some(base) = e.name.strip_suffix(".acb") {
                        companion_awbs.insert(format!("{base}.awb"));
                    }
                }

                let filtered: Vec<ManifestEntry> = req
                    .entries
                    .iter()
                    .filter(|e| {
                        // Only audio assets under sound/ in Files sub-tab
                        if !e.name.starts_with("sound/") {
                            return false;
                        }

                        // Hide companion AWB files from main list since their parent ACB wraps them
                        if companion_awbs.contains(&e.name) {
                            return false;
                        }

                        // Category filter
                        if cat != "all" {
                            let prefix = format!("sound/{cat}/");
                            if !e.name.starts_with(&prefix) {
                                return false;
                            }
                        }

                        // Download state filter
                        if only_dl && e.state != 1 && !e.exists_on_disk(&game_dir) {
                            return false;
                        }

                        // Text search
                        if !search_lower.is_empty() {
                            let name_lower = e.name.to_lowercase();
                            if !name_lower.contains(&search_lower) {
                                return false;
                            }
                        }

                        true
                    })
                    .cloned()
                    .collect();

                let _ = tx_filter_worker_out.send(filtered);
            }
        });

        let (tx_cue, rx_cue_worker) = channel::<CueInspectRequest>();
        let (tx_cue_worker_out, rx_cue) = channel::<(i64, Result<Vec<CueInfo>, String>)>();

        thread::spawn(move || {
            while let Ok(req) = rx_cue_worker.recv() {
                let entry_id = req.entry.id;
                let res = (|| -> Result<Vec<CueInfo>, String> {
                    if req.entry.name.ends_with(".acb") {
                        let prep = crate::gameaudio::mixer::prepare_acb_file(
                            &req.game_dir,
                            &req.entry,
                            req.companion_awb.as_ref(),
                        )?;
                        crate::gameaudio::mixer::inspect_audio_file(&prep)
                    } else {
                        let path = req.entry.file_path(&req.game_dir);
                        crate::gameaudio::mixer::inspect_audio_file(&path)
                    }
                })();
                let _ = tx_cue_worker_out.send((entry_id, res));
            }
        });

        Self {
            sub_tab: MusicSubTab::Files,
            search: String::new(),
            category: "all".to_string(),
            only_downloaded: false,
            selected_entry: None,
            cues: None,
            is_loading_cues: false,
            cues_error: None,
            filtered_entries: Vec::new(),
            is_filtering: false,
            juke_search: String::new(),
            juke_only_downloaded: false,
            juke_hide_hidden: true,
            juke_selected_id: None,
            juke_version: JukeboxVersion::Short,
            live_search: String::new(),
            live_only_downloaded: false,
            live_sidebar_open: true,
            live_selected_id: None,
            live_sfx: 1,
            live_mute_oke: false,
            live_mute_vocals: false,
            live_vocal_volume: 1.0,
            live_show_lyrics: true,
            live_lyrics_anchor: None,
            live_lineup: Vec::new(),
            textures: crate::gameaudio::TextureManager::default(),
            last_filter_key: (String::new(), String::new(), false, 0),
            tx_filter,
            rx_filter,
            tx_cue,
            rx_cue,
        }
    }
}

impl MusicTabState {
    /// Polls background channel messages once per frame.
    pub fn poll(&mut self) {
        while let Ok(filtered) = self.rx_filter.try_recv() {
            self.filtered_entries = filtered;
            self.is_filtering = false;
        }

        while let Ok((entry_id, res)) = self.rx_cue.try_recv() {
            if let Some(curr) = &self.selected_entry {
                if curr.id == entry_id {
                    self.is_loading_cues = false;
                    match res {
                        Ok(cues) => {
                            self.cues = Some(cues);
                            self.cues_error = None;
                        }
                        Err(e) => {
                            self.cues = None;
                            self.cues_error = Some(e);
                        }
                    }
                }
            }
        }
    }

    /// Triggers a background filtering task if query inputs changed.
    pub fn trigger_filter(&mut self, entries: Arc<Vec<ManifestEntry>>, game_path: &str) {
        let key = (
            self.search.clone(),
            self.category.clone(),
            self.only_downloaded,
            entries.len(),
        );

        if key != self.last_filter_key || (self.filtered_entries.is_empty() && !entries.is_empty())
        {
            self.last_filter_key = key;
            self.is_filtering = true;
            let _ = self.tx_filter.send(FilterRequest {
                entries,
                search: self.search.clone(),
                category: self.category.clone(),
                only_downloaded: self.only_downloaded,
                game_dir: PathBuf::from(game_path),
            });
        }
    }

    /// Selects an audio entry and triggers background cue inspection.
    pub fn select_entry(
        &mut self,
        entry: ManifestEntry,
        companion: Option<ManifestEntry>,
        game_path: &str,
    ) {
        self.selected_entry = Some(entry.clone());
        self.cues = None;
        self.cues_error = None;
        self.is_loading_cues = true;

        let _ = self.tx_cue.send(CueInspectRequest {
            entry,
            companion_awb: companion,
            game_dir: PathBuf::from(game_path),
        });
    }
}

impl App {
    /// Main entry point for rendering the Music tab.
    pub(crate) fn music_tab(&mut self, ui: &mut egui::Ui) {
        self.music.poll();
        self.music.textures.poll(ui.ctx());

        // Ensure background filter runs when manifest is loaded
        let manifest_arc = Arc::clone(&self.gameaudio.manifest_entries);
        self.music
            .trigger_filter(manifest_arc, &self.gameaudio.game_path);

        egui::Panel::top("music_bar")
            .frame(egui::Frame::new().inner_margin(Margin {
                left: 14,
                right: 18,
                top: 12,
                bottom: 10,
            }))
            .show(ui, |ui| self.music_bar(ui));

        // Reserve bottom panel for mini-player so CentralPanel takes the exact remaining height
        let has_active_audio = self.gameaudio.current_title().is_some()
            || self.gameaudio.playback_state() != PlaybackState::Stopped;

        if has_active_audio {
            egui::Panel::bottom("music_mini_player")
                .frame(egui::Frame::new().inner_margin(Margin {
                    left: 14,
                    right: 18,
                    top: 2,
                    bottom: 8,
                }))
                .show(ui, |ui| {
                    self.music_mini_player(ui);
                });
        }

        // Central area rendering current sub-tab
        egui::CentralPanel::default()
            .frame(crate::page_frame())
            .show(ui, |ui| {
                // Request timely UI update while audio is actively playing
                if self.gameaudio.playback_state() == PlaybackState::Playing {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                }

                let p = pal(ui);
                if !self.gameaudio.active_catalog().is_configured() {
                    let reg_label = self.gameaudio.active_region.label();
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(format!("⚠ {reg_label} {}", t!("game directory is not configured.")))
                                .size(18.0)
                                .color(p.warn)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(t!("Please configure the game Persistent directory in Settings → Game audio."))
                                .color(p.weak),
                        );
                        ui.add_space(16.0);
                        if ui.button(t!("⚙ Open Settings")).clicked() {
                            self.open_settings(crate::settings_page::SettingsPage::GameAudio);
                        }
                    });
                } else if let crate::gameaudio::GameAudioStatus::Error(err) = &self.gameaudio.status {
                    let reg_label = self.gameaudio.active_region.label();
                    let err_msg = err.clone();
                    let mut reload_clicked = false;
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(format!("⚠ {reg_label} {}: {err_msg}", t!("Error")))
                                .size(16.0)
                                .color(p.warn)
                                .strong(),
                        );
                        ui.add_space(16.0);
                        if ui.button(t!("🔄 Reload")).clicked() {
                            reload_clicked = true;
                        }
                    });
                    if reload_clicked {
                        self.gameaudio.start_load();
                    }
                } else if self.gameaudio.status == crate::gameaudio::GameAudioStatus::Loading {
                    let reg_label = self.gameaudio.active_region.label();
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!("{}: {reg_label}...", t!("Loading game data")))
                                .size(16.0)
                                .color(p.accent),
                        );
                    });
                } else {
                    match self.music.sub_tab {
                        MusicSubTab::Files => self.music_files_tab(ui),
                        MusicSubTab::Jukebox => self.music_jukebox_tab(ui),
                        MusicSubTab::Live => self.music_live_tab(ui),
                    }
                }
            });
    }

    /// Switches the active Game Audio region and resets sub-tab selections.
    pub(crate) fn switch_gameaudio_region(&mut self, region: crate::gameaudio::GameRegion) {
        if self.gameaudio.active_region == region {
            return;
        }
        self.gameaudio.set_active_region(region);
        self.music.selected_entry = None;
        self.music.cues = None;
        self.music.cues_error = None;
        self.music.is_loading_cues = false;
        self.music.filtered_entries.clear();
        self.music.last_filter_key = (String::new(), String::new(), false, 0);
        self.music.juke_selected_id = None;
        self.music.live_selected_id = None;
        self.music.live_lineup.clear();
        self.music.textures.clear();
        self.music.trigger_filter(
            Arc::clone(&self.gameaudio.manifest_entries),
            &self.gameaudio.game_path,
        );

        if !self.gameaudio.active_catalog().is_configured() {
            let label = region.label();
            self.toast(
                format!(
                    "{label} {}: {}",
                    t!("game directory is not configured."),
                    t!("Please configure it in Settings → Game audio.")
                ),
                crate::ui_util::ToastKind::Warn,
            );
        }
    }

    /// Top bar with sub-tabs, region switch, and mascot chibi.
    fn music_bar(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            ui.label(semibold(t!("🎵 Music"), 22.0));
            ui.add_space(4.0);
            crate::ui_util::badge(ui, t!("Beta"), p.accent);
            ui.add_space(10.0);

            let options = [
                (
                    MusicSubTab::Files,
                    tf!("📁 Files  {}", self.music.filtered_entries.len()),
                    t!("Browse and play game audio files (ACB/AWB)"),
                ),
                (
                    MusicSubTab::Jukebox,
                    tf!("📻 Jukebox  {}", self.gameaudio.master.jukebox_songs.len()),
                    t!("Game jukebox songs collection"),
                ),
                (
                    MusicSubTab::Live,
                    tf!("🎤 Live  {}", self.gameaudio.master.live_songs.len()),
                    t!("Multi-track live stage player with synchronized karaoke lyrics and customizable character lineup."),
                ),
            ];
            segmented(ui, &mut self.music.sub_tab, &options);

            ui.add_space(10.0);

            let mut current_reg = self.gameaudio.active_region;
            let reg_options = [
                (
                    crate::gameaudio::GameRegion::Global,
                    "Global".to_string(),
                    t!("Switch to Global game data"),
                ),
                (
                    crate::gameaudio::GameRegion::Jp,
                    "JP".to_string(),
                    t!("Switch to JP game data"),
                ),
            ];
            segmented(ui, &mut current_reg, &reg_options);
            if current_reg != self.gameaudio.active_region {
                self.switch_gameaudio_region(current_reg);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                crate::ui_util::header_chibi(ui);
                if self.gameaudio.is_exporting {
                    ui.label(RichText::new(t!("⏳ Exporting WAV...")).color(p.accent).size(13.0));
                }
                if self.gameaudio.readiness.is_scanning {
                    ui.label(RichText::new(t!("↻ Scanning disk...")).color(p.weak).size(12.0));
                } else if self.gameaudio.status == crate::gameaudio::GameAudioStatus::Ready {
                    if ui
                        .small_button(t!("↻ Rescan"))
                        .on_hover_text(t!("Rescan game files on disk to update downloaded status"))
                        .clicked()
                    {
                        self.gameaudio.start_readiness_scan();
                    }
                }
                if ui
                    .small_button(t!("📂 Export folder"))
                    .on_hover_text(t!("Open the WAV audio export directory in Windows Explorer"))
                    .clicked()
                {
                    if let Err(e) = self.gameaudio.open_export_dir() {
                        self.toast(e, crate::ui_util::ToastKind::Warn);
                    }
                }
            });
        });

        let explain = match self.music.sub_tab {
            MusicSubTab::Files => t!(
                "Browse CRIWARE audio files from game storage (ACB cue sheets and AWB containers). Select a file to inspect cues, play, or export WAV."
            ),
            MusicSubTab::Jukebox => t!(
                "Jukebox music collection: short and game-size versions with cuesheet linkage and instant playback."
            ),
            MusicSubTab::Live => t!(
                "Live performance stage player: multi-track mixer with karaoke lyrics and member line-up."
            ),
        };
        ui.label(RichText::new(explain).size(13.0).color(p.weak));

        ui.add_space(5.0);
        let reg_label = self.gameaudio.active_region.label();
        egui::Frame::new()
            .fill(p.warn.gamma_multiply(if ui.visuals().dark_mode { 0.10 } else { 0.06 }))
            .stroke(Stroke::new(1.0, p.warn.gamma_multiply(if ui.visuals().dark_mode { 0.45 } else { 0.35 })))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin { left: 8, right: 8, top: 4, bottom: 4 })
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("💡").color(p.warn).size(12.5));
                    ui.label(
                        RichText::new(tf!(
                            "Recommendation: Open the {} game and download full in-game data so audio tracks, vocals, lyrics, and jacket/character art are fully available.",
                            reg_label,
                        ))
                        .size(12.0)
                        .color(p.warn),
                    )
                    .on_hover_text(t!(
                        "Missing game assets may cause certain songs, vocals, lyrics, or jacket/character images to be unavailable in Music."
                    ));
                });
            });
    }

    /// Sub-tab Files: left filter sidebar, middle file table, right cue detail.
    fn music_files_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // 1. Sidebar filters
            egui::Frame::new()
                .inner_margin(Margin::same(10))
                .fill(pal(ui).input.gamma_multiply(0.4))
                .corner_radius(CornerRadius::same(10))
                .stroke(Stroke::new(1.0, pal(ui).border))
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        vec2(250.0, ui.available_height()),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.music_files_sidebar(ui);
                        },
                    );
                });

            ui.add_space(8.0);

            // 2. Middle file list & Right cue panel
            let total_w = ui.available_width();
            let has_selection = self.music.selected_entry.is_some();
            let list_w = if has_selection {
                (total_w * 0.52).max(360.0)
            } else {
                total_w
            };

            ui.allocate_ui_with_layout(
                vec2(list_w, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| {
                    self.music_files_list(ui);
                },
            );

            if has_selection {
                ui.add_space(8.0);
                egui::Frame::new()
                    .inner_margin(Margin::same(12))
                    .fill(pal(ui).surface)
                    .corner_radius(CornerRadius::same(10))
                    .stroke(Stroke::new(1.0, pal(ui).border))
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), ui.available_height()),
                            Layout::top_down(Align::Min),
                            |ui| {
                                self.music_files_detail(ui);
                            },
                        );
                    });
            }
        });
    }

    /// Sidebar controls for search, category, and download filters.
    fn music_files_sidebar(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let focus = self.focus_search.take();

        ui.label(semibold(t!("Filters"), 14.0));
        ui.add_space(6.0);

        // Search box with Ctrl+F focus
        kit::search_box(
            ui,
            &mut self.music.search,
            t!("🔍 Search file name (Ctrl+F)"),
            focus,
        );

        ui.add_space(12.0);
        crate::ui_util::section_label(ui, "CATEGORY", "Filter by game audio folder category");
        ui.add_space(4.0);

        let categories = [
            ("all", "📋 All", "All sound files"),
            ("b", "🎵 BGM", "Background music"),
            ("c", "🎬 Cutin", "Cut-in scene sounds"),
            ("j", "🔔 Jingle", "Short jingles and fanfares"),
            ("l", "🎤 Live", "Live music archives"),
            ("s", "⚙ System", "System sound effects"),
            ("v", "🗣 Voice", "Character voice lines"),
        ];

        for (code, label, hover) in categories {
            let selected = self.music.category == code;
            let text =
                RichText::new(t!(label))
                    .size(13.5)
                    .color(if selected { p.text } else { p.weak });

            let btn = egui::Button::new(text)
                .fill(if selected {
                    p.raised
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(if selected {
                    Stroke::new(1.0, p.accent.gamma_multiply(0.7))
                } else {
                    Stroke::NONE
                })
                .corner_radius(CornerRadius::same(6))
                .min_size(vec2(ui.available_width(), 26.0));

            if ui.add(btn).on_hover_text(t!(hover)).clicked() {
                self.music.category = code.to_string();
            }
        }

        ui.add_space(14.0);
        crate::ui_util::section_label(ui, "STORAGE", "Filter by local file download status");
        ui.add_space(4.0);

        ui.checkbox(&mut self.music.only_downloaded, t!("Downloaded files only"))
            .on_hover_text(t!(
                "Only show audio files that are downloaded and present on disk"
            ));

        ui.add_space(16.0);

        // Stats summary badge
        let matching = self.music.filtered_entries.len();
        let total = self.gameaudio.manifest_entries.len();
        let stats_text = tf!("Showing {} of {} files", matching, total);
        ui.label(RichText::new(stats_text).size(12.0).color(p.weak));

        if !self.music.search.is_empty()
            || self.music.category != "all"
            || self.music.only_downloaded
        {
            ui.add_space(8.0);
            if ui
                .button(
                    RichText::new(t!("✕ Reset filters"))
                        .size(12.5)
                        .color(p.accent),
                )
                .clicked()
            {
                self.music.search.clear();
                self.music.category = "all".to_string();
                self.music.only_downloaded = false;
            }
        }
    }

    /// Middle column: Table of matching audio files.
    fn music_files_list(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);

        if self.music.filtered_entries.is_empty() {
            if self.music.is_filtering {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label(t!("Filtering audio files..."));
                });
            } else {
                empty_state(
                    ui,
                    "🎵",
                    t!("No sound files found"),
                    t!("Try adjusting your search terms or clearing category filters."),
                );
            }
            return;
        }

        let selected_id = self.music.selected_entry.as_ref().map(|e| e.id);

        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(56.0)) // Category tag
            .column(Column::remainder()) // Name
            .column(Column::exact(68.0)) // Size
            .column(Column::exact(92.0)); // Status

        table
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.label(semibold(t!("Kind"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Asset Name"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Size"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Status"), 12.0));
                });
            })
            .body(|body| {
                let row_height = 30.0;
                let count = self.music.filtered_entries.len();

                body.rows(row_height, count, |mut row| {
                    let idx = row.index();
                    let entry = self.music.filtered_entries[idx].clone();
                    let is_selected = selected_id == Some(entry.id);

                    // Category tag code
                    let cat_tag = if entry.name.starts_with("sound/b/") {
                        "BGM"
                    } else if entry.name.starts_with("sound/c/") {
                        "CUT"
                    } else if entry.name.starts_with("sound/j/") {
                        "JNG"
                    } else if entry.name.starts_with("sound/l/") {
                        "LIVE"
                    } else if entry.name.starts_with("sound/s/") {
                        "SYS"
                    } else if entry.name.starts_with("sound/v/") {
                        "VOX"
                    } else {
                        "SND"
                    };

                    let file_stem = Path::new(&entry.name)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&entry.name);

                    let size_kb = entry.length as f64 / 1024.0;
                    let size_str = if size_kb >= 1024.0 {
                        format!("{:.1} MB", size_kb / 1024.0)
                    } else {
                        format!("{:.0} KB", size_kb)
                    };

                    let is_dl = entry.state == 1 || self.gameaudio.is_entry_on_disk(entry.id);

                    // Row column 1: Category Badge
                    row.col(|ui| {
                        let fill = if is_selected {
                            p.accent.gamma_multiply(0.2)
                        } else {
                            p.input
                        };
                        egui::Frame::new()
                            .fill(fill)
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(4, 2))
                            .show(ui, |ui| {
                                ui.label(RichText::new(cat_tag).size(10.5).color(if is_selected {
                                    p.accent
                                } else {
                                    p.weak
                                }));
                            });
                    });

                    // Row column 2: Asset Name
                    row.col(|ui| {
                        let text = RichText::new(file_stem).size(13.0).color(if is_selected {
                            p.accent
                        } else {
                            p.text
                        });
                        if ui
                            .selectable_label(is_selected, text)
                            .on_hover_text(&entry.name)
                            .clicked()
                        {
                            let companion = self.gameaudio.find_companion_awb(&entry.name).cloned();
                            self.music.select_entry(
                                entry.clone(),
                                companion,
                                &self.gameaudio.game_path,
                            );
                        }
                    });

                    // Row column 3: Size
                    row.col(|ui| {
                        ui.label(RichText::new(size_str).size(11.5).color(p.weak));
                    });

                    // Row column 4: Download Status Badge & Quick play
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            if is_dl {
                                ui.label(RichText::new(t!("✓ Ready")).size(11.5).color(p.good));
                                if ui
                                    .small_button("▶")
                                    .on_hover_text(t!("Play cue #0"))
                                    .clicked()
                                {
                                    self.gameaudio.play_manifest_cue(&entry, 0);
                                }
                            } else {
                                ui.label(RichText::new(t!("⏳ Missing")).size(11.5).color(p.weak))
                                    .on_hover_text(t!(
                                        "Not downloaded in game. Download game data to play."
                                    ));
                            }
                        });
                    });
                });
            });
    }

    /// Right column: File information, Cues list, Play cue, Export WAV.
    fn music_files_detail(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some(entry) = self.music.selected_entry.clone() else {
            return;
        };

        let is_dl = entry.state == 1 || self.gameaudio.is_entry_on_disk(entry.id);

        // Header info
        ui.horizontal(|ui| {
            ui.label(semibold(t!("Track Details"), 16.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(RichText::new("✕").size(14.0).color(p.weak))
                    .clicked()
                {
                    self.music.selected_entry = None;
                    self.music.cues = None;
                }
            });
        });

        ui.add_space(8.0);

        // Metadata Card
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(semibold(t!("Asset:"), 12.5));
                ui.label(RichText::new(&entry.name).size(12.5).color(p.text));
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(semibold(t!("Length:"), 12.0));
                let kb = entry.length as f64 / 1024.0;
                ui.label(
                    RichText::new(format!("{kb:.1} KB ({})", entry.length))
                        .size(12.0)
                        .color(p.weak),
                );
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(semibold(t!("Storage hash:"), 11.5));
                ui.label(RichText::new(&entry.hash_name).size(11.5).color(p.weak));
            });
        });

        ui.add_space(10.0);

        if !is_dl {
            // Not downloaded notice
            egui::Frame::new()
                .fill(p.surface.gamma_multiply(0.7))
                .stroke(Stroke::new(1.0, p.accent.gamma_multiply(0.4)))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.label(RichText::new(t!("⚠️ File not downloaded in game")).size(14.0).color(p.accent));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(t!(
                            "This audio file is registered in the game's manifest database but its data file is not present on your PC. Please download additional assets within the game to enable playback."
                        ))
                        .size(12.5)
                        .color(p.weak),
                    );
                });
            return;
        }

        // Action Toolbar
        ui.horizontal(|ui| {
            let play_btn = primary_button(ui, t!("▶ Play cue #0"), 13.0);
            if ui.add(play_btn).clicked() {
                self.gameaudio.play_manifest_cue(&entry, 0);
            }

            if ui
                .button(RichText::new(t!("💾 Export all WAV")).size(13.0))
                .on_hover_text(t!("Export all waves in this file to exports/audio/files/"))
                .clicked()
            {
                if let Some(cues) = &self.music.cues {
                    let out_dir = self
                        .gameaudio
                        .export_category_dir(crate::gameaudio::ExportCategory::Files);
                    for cue in cues {
                        let base_name = Path::new(&entry.name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("track");
                        let out_file = out_dir.join(format!("{}_{}.wav", base_name, cue.name));
                        self.gameaudio
                            .export_manifest_cue_wav(&entry, cue.index, out_file);
                    }
                    self.toast(
                        tf!("Queued all tracks for WAV export to {}.", out_dir.display()),
                        crate::ui_util::ToastKind::Success,
                    );
                }
            }
        });

        ui.add_space(12.0);
        ui.label(semibold(t!("Wave Cues"), 14.0));
        ui.add_space(4.0);

        // Cues List
        let cues_opt = self.music.cues.clone();
        if self.music.is_loading_cues {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.add_space(6.0);
                ui.label(t!("Reading audio cues..."));
            });
        } else if let Some(err) = &self.music.cues_error {
            ui.label(
                RichText::new(format!("{}: {err}", t!("Cannot read cues")))
                    .size(12.5)
                    .color(p.bad),
            );
        } else if let Some(cues) = cues_opt {
            if cues.is_empty() {
                ui.label(t!("No audio waves inside this container."));
            } else {
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for cue in cues {
                            let cue_idx = cue.index;
                            let cue_name = cue.name.clone();

                            card(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("#{cue_idx}"))
                                            .size(13.0)
                                            .color(p.accent),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(RichText::new(&cue_name).size(13.0).color(p.text));

                                    let kb = cue.size_bytes as f64 / 1024.0;
                                    ui.label(
                                        RichText::new(format!("{kb:.1} KB"))
                                            .size(11.5)
                                            .color(p.weak),
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        // Export button
                                        if ui
                                            .button(RichText::new("💾").size(13.0))
                                            .on_hover_text(t!(
                                                "Export this track as 16-bit PCM WAV"
                                            ))
                                            .clicked()
                                        {
                                            let base_name = Path::new(&entry.name)
                                                .file_stem()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or("track");
                                            let out_dir = self.gameaudio.export_category_dir(
                                                crate::gameaudio::ExportCategory::Files,
                                            );
                                            let out_file = out_dir
                                                .join(format!("{}_{}.wav", base_name, cue_name));
                                            self.gameaudio.export_manifest_cue_wav(
                                                &entry,
                                                cue_idx,
                                                out_file.clone(),
                                            );
                                            self.toast(
                                                tf!("Exporting to {}", out_file.display()),
                                                crate::ui_util::ToastKind::Info,
                                            );
                                        }

                                        // Play button
                                        if ui
                                            .button(RichText::new("▶").size(13.0))
                                            .on_hover_text(t!("Play this cue"))
                                            .clicked()
                                        {
                                            self.gameaudio.play_manifest_cue(&entry, cue_idx);
                                        }
                                    });
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });
            }
        }
    }

    /// Sub-tab Jukebox: left sidebar filters, middle song list, right detail & export panel.
    fn music_jukebox_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // 1. Sidebar filters
            egui::Frame::new()
                .inner_margin(Margin::same(10))
                .fill(pal(ui).input.gamma_multiply(0.4))
                .corner_radius(CornerRadius::same(10))
                .stroke(Stroke::new(1.0, pal(ui).border))
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        vec2(250.0, ui.available_height()),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.music_jukebox_sidebar(ui);
                        },
                    );
                });

            ui.add_space(8.0);

            // 2. Middle song list & Right detail panel
            let total_w = ui.available_width();
            let selected_song = self
                .music
                .juke_selected_id
                .and_then(|id| {
                    self.gameaudio
                        .master
                        .jukebox_songs
                        .iter()
                        .find(|s| s.music_id == id)
                })
                .cloned();

            let has_selection = selected_song.is_some();
            let list_w = if has_selection {
                (total_w * 0.54).max(380.0)
            } else {
                total_w
            };

            ui.allocate_ui_with_layout(
                vec2(list_w, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| {
                    self.music_jukebox_list(ui);
                },
            );

            if let Some(song) = selected_song {
                ui.add_space(8.0);
                egui::Frame::new()
                    .inner_margin(Margin::same(12))
                    .fill(pal(ui).surface)
                    .corner_radius(CornerRadius::same(10))
                    .stroke(Stroke::new(1.0, pal(ui).border))
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), ui.available_height()),
                            Layout::top_down(Align::Min),
                            |ui| {
                                self.music_jukebox_detail(ui, &song);
                            },
                        );
                    });
            }
        });
    }

    /// Sidebar controls for Jukebox search, version preference, and download filters.
    fn music_jukebox_sidebar(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let focus = self.focus_search.take();

        ui.label(semibold(t!("Filters"), 14.0));
        ui.add_space(6.0);

        // Search box with Ctrl+F focus
        kit::search_box(
            ui,
            &mut self.music.juke_search,
            t!("🔍 Search title or cue (Ctrl+F)"),
            focus,
        );

        ui.add_space(12.0);
        crate::ui_util::section_label(
            ui,
            "DEFAULT VERSION",
            "Preferred playback version for quick-play",
        );
        ui.add_space(4.0);

        let versions = [
            (
                JukeboxVersion::Short,
                t!("⚡ Short"),
                t!("Shortened preview version (~20-35 seconds)"),
            ),
            (
                JukeboxVersion::GameSize,
                t!("🎼 Game-size"),
                t!("Full in-game length track version"),
            ),
        ];

        for (ver, label, hover) in versions {
            let selected = self.music.juke_version == ver;
            let text =
                RichText::new(label)
                    .size(13.0)
                    .color(if selected { p.text } else { p.weak });

            let btn = egui::Button::new(text)
                .fill(if selected {
                    p.raised
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(if selected {
                    Stroke::new(1.0, p.accent.gamma_multiply(0.7))
                } else {
                    Stroke::NONE
                })
                .corner_radius(CornerRadius::same(6))
                .min_size(vec2(ui.available_width(), 26.0));

            if ui.add(btn).on_hover_text(hover).clicked() {
                self.music.juke_version = ver;
            }
        }

        ui.add_space(14.0);
        crate::ui_util::section_label(ui, "AVAILABILITY", "Filter by storage and visibility");
        ui.add_space(4.0);

        ui.checkbox(&mut self.music.juke_only_downloaded, t!("Downloaded only"))
            .on_hover_text(t!(
                "Only show songs that have at least one audio file downloaded on disk"
            ));

        ui.checkbox(&mut self.music.juke_hide_hidden, t!("Hide hidden tracks"))
            .on_hover_text(t!(
                "Hide unreleased or secret jukebox tracks flagged as hidden"
            ));

        ui.add_space(14.0);
        crate::ui_util::section_label(ui, "STATISTICS", "Loaded jukebox songs overview");
        ui.add_space(4.0);

        let total = self.gameaudio.master.jukebox_songs.len();

        let ready_count = self
            .gameaudio
            .master
            .jukebox_songs
            .iter()
            .filter(|s| {
                let (s_ok, g_ok) = self.gameaudio.is_jukebox_ready(s.music_id);
                s_ok || g_ok
            })
            .count();

        ui.label(
            RichText::new(tf!("Total songs: {}", total))
                .size(12.5)
                .color(p.text),
        );
        ui.label(
            RichText::new(tf!("Ready on disk: {}", ready_count))
                .size(12.5)
                .color(p.accent),
        );

        ui.add_space(12.0);
        if ui
            .button(RichText::new(t!("↺ Reset filters")).size(12.0))
            .on_hover_text(t!("Reset all search queries and filters to default"))
            .clicked()
        {
            self.music.juke_search.clear();
            self.music.juke_only_downloaded = false;
            self.music.juke_hide_hidden = true;
            self.music.juke_version = JukeboxVersion::Short;
        }

        ui.add_space(14.0);
        card(ui, |ui| {
            ui.label(
                RichText::new(t!("💡 Jukebox Tip"))
                    .size(12.0)
                    .color(p.accent),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new(t!(
                    "If a Short version is not downloaded in the game yet, UmaYu will automatically fallback to the Game-size version, and vice-versa."
                ))
                .size(11.5)
                .color(p.weak),
            );
        });
    }

    /// List of Jukebox tracks formatted in a clean, interactive table.
    fn music_jukebox_list(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let search_lower = self.music.juke_search.trim().to_lowercase();
        let only_dl = self.music.juke_only_downloaded;
        let hide_hidden = self.music.juke_hide_hidden;

        let filtered_songs: Vec<crate::gameaudio::master::JukeboxMusicData> = self
            .gameaudio
            .master
            .jukebox_songs
            .iter()
            .filter(|s| {
                if hide_hidden && s.is_hidden != 0 {
                    return false;
                }
                if !search_lower.is_empty() {
                    let title_match = s.title.to_lowercase().contains(&search_lower);
                    let id_match = s.music_id.to_string().contains(&search_lower);
                    let cs_short_match = s
                        .bgm_cuesheet_name_short
                        .as_deref()
                        .map(|cs| cs.to_lowercase().contains(&search_lower))
                        .unwrap_or(false);
                    let cs_game_match = s
                        .bgm_cuesheet_name_gamesize
                        .as_deref()
                        .map(|cs| cs.to_lowercase().contains(&search_lower))
                        .unwrap_or(false);
                    if !title_match && !id_match && !cs_short_match && !cs_game_match {
                        return false;
                    }
                }
                if only_dl {
                    let (s_ok, g_ok) = self.gameaudio.is_jukebox_ready(s.music_id);
                    if !s_ok && !g_ok {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if filtered_songs.is_empty() {
            empty_state(
                ui,
                "📻",
                t!("No jukebox songs found"),
                t!("Try adjusting your search terms or unchecking 'Downloaded only' filter."),
            );
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tf!("Showing {} songs", filtered_songs.len()))
                    .size(12.5)
                    .color(p.weak),
            );
        });
        ui.add_space(4.0);

        let mut action_select = None;
        let mut action_play = None;

        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(34.0)) // Cover
            .column(Column::exact(45.0)) // Sort
            .column(Column::initial(180.0).range(120.0..=360.0)) // Title
            .column(Column::exact(60.0)) // Short Length
            .column(Column::exact(85.0)) // Short status
            .column(Column::exact(85.0)) // Game status
            .column(Column::remainder()); // Quick play buttons

        table
            .header(26.0, |mut header| {
                header.col(|ui| {
                    ui.label(semibold(t!("Cover"), 11.5));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("#"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Song Title"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Length"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Short"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Game-size"), 12.0));
                });
                header.col(|ui| {
                    ui.label(semibold(t!("Play"), 12.0));
                });
            })
            .body(|body| {
                let count = filtered_songs.len();
                body.rows(32.0, count, |mut row| {
                    let song = &filtered_songs[row.index()];
                    let is_selected = self.music.juke_selected_id == Some(song.music_id);
                    let (short_ready, game_ready) = self.gameaudio.is_jukebox_ready(song.music_id);

                    let tooltip_text = format!(
                        "ID: {}\nSort: {}\nShort Cue: {}\nGame Cue: {}\nHidden: {}",
                        song.music_id,
                        song.sort,
                        song.bgm_cuesheet_name_short.as_deref().unwrap_or("-"),
                        song.bgm_cuesheet_name_gamesize.as_deref().unwrap_or("-"),
                        if song.is_hidden != 0 { "Yes" } else { "No" },
                    );

                    row.set_selected(is_selected);

                    // 0. Cover
                    row.col(|ui| {
                        let (icon_rect, resp) =
                            ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
                        if resp.on_hover_text(&tooltip_text).clicked() {
                            action_select = Some(song.music_id);
                        }
                        let jacket = self.music.textures.get_or_request_jacket(
                            &self.gameaudio.game_path,
                            song.music_id,
                            &self.gameaudio.manifest_entries,
                        );
                        if let Some(tex) = jacket {
                            egui::Image::from_texture(tex)
                                .corner_radius(CornerRadius::same(4))
                                .paint_at(ui, icon_rect);
                        } else {
                            ui.painter().rect_filled(
                                icon_rect,
                                CornerRadius::same(4),
                                p.input.gamma_multiply(0.4),
                            );
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "♫",
                                egui::FontId::proportional(11.0),
                                p.weak,
                            );
                        }
                    });

                    // 1. Sort
                    row.col(|ui| {
                        let resp = ui.label(
                            RichText::new(format!("{}", song.sort))
                                .size(12.0)
                                .color(p.weak),
                        );
                        if resp.on_hover_text(&tooltip_text).clicked() {
                            action_select = Some(song.music_id);
                        }
                    });

                    // 2. Title
                    row.col(|ui| {
                        let text = RichText::new(&song.title).size(13.0).color(if is_selected {
                            p.accent
                        } else {
                            p.text
                        });
                        let resp = ui.add(egui::Label::new(text).truncate());
                        if resp.on_hover_text(&tooltip_text).clicked() {
                            action_select = Some(song.music_id);
                        }
                    });

                    // 3. Short Length
                    row.col(|ui| {
                        let dur_str = if song.short_length > 0 {
                            format!("{:.0}s", song.short_length as f32 / 1000.0)
                        } else {
                            "-".to_string()
                        };
                        let resp = ui.label(RichText::new(dur_str).size(12.0).color(p.weak));
                        if resp.on_hover_text(&tooltip_text).clicked() {
                            action_select = Some(song.music_id);
                        }
                    });

                    // 4. Short status
                    row.col(|ui| {
                        let (badge, color) = if short_ready {
                            (t!("✓ Ready"), p.accent)
                        } else {
                            (t!("⏳ Missing"), p.weak)
                        };
                        let resp = ui.label(RichText::new(badge).size(11.5).color(color));
                        if resp.on_hover_text(&tooltip_text).clicked() {
                            action_select = Some(song.music_id);
                        }
                    });

                    // 5. Game status
                    row.col(|ui| {
                        let (badge, color) = if game_ready {
                            (t!("✓ Ready"), p.accent)
                        } else {
                            (t!("⏳ Missing"), p.weak)
                        };
                        let resp = ui.label(RichText::new(badge).size(11.5).color(color));
                        if resp.on_hover_text(&tooltip_text).clicked() {
                            action_select = Some(song.music_id);
                        }
                    });

                    // 6. Action buttons
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            let btn_s = egui::Button::new(RichText::new("▶ S").size(11.0))
                                .fill(if short_ready {
                                    p.raised
                                } else {
                                    p.input.gamma_multiply(0.3)
                                })
                                .corner_radius(CornerRadius::same(4))
                                .min_size(vec2(28.0, 20.0));
                            if ui
                                .add(btn_s)
                                .on_hover_text(t!(
                                    "Play Short version (auto-fallback to Game-size)"
                                ))
                                .clicked()
                            {
                                action_select = Some(song.music_id);
                                action_play = Some((song.clone(), JukeboxVersion::Short));
                            }

                            let btn_g = egui::Button::new(RichText::new("▶ G").size(11.0))
                                .fill(if game_ready {
                                    p.raised
                                } else {
                                    p.input.gamma_multiply(0.3)
                                })
                                .corner_radius(CornerRadius::same(4))
                                .min_size(vec2(28.0, 20.0));
                            if ui
                                .add(btn_g)
                                .on_hover_text(t!(
                                    "Play Game-size version (auto-fallback to Short)"
                                ))
                                .clicked()
                            {
                                action_select = Some(song.music_id);
                                action_play = Some((song.clone(), JukeboxVersion::GameSize));
                            }
                        });
                    });
                });
            });

        if let Some(id) = action_select {
            self.music.juke_selected_id = Some(id);
        }
        if let Some((song, ver)) = action_play {
            self.gameaudio.play_jukebox_song(&song, ver);
        }
    }

    /// Detail & export inspector panel for the selected Jukebox track.
    fn music_jukebox_detail(
        &mut self,
        ui: &mut egui::Ui,
        song: &crate::gameaudio::master::JukeboxMusicData,
    ) {
        let p = pal(ui);

        let short_entry = song
            .bgm_cuesheet_name_short
            .as_deref()
            .and_then(|cs| self.gameaudio.find_cuesheet_entry(cs))
            .cloned();

        let game_entry = song
            .bgm_cuesheet_name_gamesize
            .as_deref()
            .and_then(|cs| self.gameaudio.find_cuesheet_entry(cs))
            .cloned();

        let (short_ready, game_ready) = self.gameaudio.is_jukebox_ready(song.music_id);

        let mut deselect = false;
        let mut play_action = None;
        let mut export_action = None;

        let jacket_tex = self.music.textures.get_or_request_jacket(
            &self.gameaudio.game_path,
            song.music_id,
            &self.gameaudio.manifest_entries,
        );

        ui.horizontal(|ui| {
            let art_size = vec2(88.0, 88.0);
            let (art_rect, _) = ui.allocate_exact_size(art_size, egui::Sense::hover());
            if let Some(tex) = jacket_tex {
                egui::Image::from_texture(tex)
                    .corner_radius(CornerRadius::same(8))
                    .paint_at(ui, art_rect);
                ui.painter().rect_stroke(
                    art_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0, p.border),
                    egui::StrokeKind::Inside,
                );
            } else {
                ui.painter().rect_filled(
                    art_rect,
                    CornerRadius::same(8),
                    p.input.gamma_multiply(0.4),
                );
                ui.painter().rect_stroke(
                    art_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0, p.border),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    art_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "♫",
                    egui::FontId::proportional(30.0),
                    p.weak,
                );
            }

            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(semibold(&song.title, 18.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(RichText::new("✕").size(13.0).color(p.weak))
                            .on_hover_text(t!("Deselect song"))
                            .clicked()
                        {
                            deselect = true;
                        }
                    });
                });

                ui.add_space(2.0);
                let hidden_str = if song.is_hidden != 0 {
                    t!("Yes")
                } else {
                    t!("No")
                };
                ui.label(
                    RichText::new(tf!(
                        "Music ID: #{} • Sort Order: {} • Hidden: {}",
                        song.music_id,
                        song.sort,
                        hidden_str
                    ))
                    .size(12.0)
                    .color(p.weak),
                );
            });
        });

        ui.add_space(14.0);

        // Card 1: Short Version
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(semibold(t!("⚡ Short Version"), 14.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if short_ready {
                        ui.label(
                            RichText::new(t!("✓ Ready on disk"))
                                .color(p.accent)
                                .size(12.0),
                        );
                    } else {
                        ui.label(
                            RichText::new(t!("⏳ Not downloaded in game"))
                                .color(p.weak)
                                .size(12.0),
                        );
                    }
                });
            });

            ui.add_space(4.0);
            let cs_short = song.bgm_cuesheet_name_short.as_deref().unwrap_or("-");
            ui.label(
                RichText::new(tf!("Cue sheet: {}", cs_short))
                    .size(12.0)
                    .color(p.text),
            );

            if let Some(entry) = &short_entry {
                ui.label(
                    RichText::new(tf!("Asset: {}", entry.name))
                        .size(11.5)
                        .color(p.weak),
                );
            }

            if song.short_length > 0 {
                let sec_str = format!("{:.1}s", song.short_length as f32 / 1000.0);
                ui.label(
                    RichText::new(tf!("Length: {}", sec_str))
                        .size(11.5)
                        .color(p.weak),
                );
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let can_play_short = short_ready || game_ready;
                let play_btn = egui::Button::new(RichText::new(t!("▶ Play Short")).size(12.0))
                    .fill(if short_ready {
                        p.accent.gamma_multiply(0.2)
                    } else {
                        p.input.gamma_multiply(0.3)
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if short_ready { p.accent } else { p.border },
                    ))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(vec2(100.0, 26.0));

                if ui
                    .add_enabled(can_play_short, play_btn)
                    .on_hover_text(if short_ready {
                        t!("Play the Short version (wave 0)")
                    } else {
                        t!("Short version not downloaded — will auto-fallback to Game-size version")
                    })
                    .clicked()
                {
                    play_action = Some(JukeboxVersion::Short);
                }

                let export_btn = egui::Button::new(RichText::new(t!("💾 Export WAV")).size(12.0))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(vec2(90.0, 26.0));

                if ui
                    .add_enabled(short_ready && !self.gameaudio.is_exporting, export_btn)
                    .on_hover_text(t!("Export Short version to WAV (exports/audio/jukebox/)"))
                    .clicked()
                {
                    export_action = Some(JukeboxVersion::Short);
                }
            });
        });

        ui.add_space(12.0);

        // Card 2: Game-size Version
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(semibold(t!("🎼 Game-size Version"), 14.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if game_ready {
                        ui.label(
                            RichText::new(t!("✓ Ready on disk"))
                                .color(p.accent)
                                .size(12.0),
                        );
                    } else {
                        ui.label(
                            RichText::new(t!("⏳ Not downloaded in game"))
                                .color(p.weak)
                                .size(12.0),
                        );
                    }
                });
            });

            ui.add_space(4.0);
            let cs_game = song.bgm_cuesheet_name_gamesize.as_deref().unwrap_or("-");
            ui.label(
                RichText::new(tf!("Cue sheet: {}", cs_game))
                    .size(12.0)
                    .color(p.text),
            );

            if let Some(entry) = &game_entry {
                ui.label(
                    RichText::new(tf!("Asset: {}", entry.name))
                        .size(11.5)
                        .color(p.weak),
                );
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let can_play_game = game_ready || short_ready;
                let play_btn = egui::Button::new(RichText::new(t!("▶ Play Game-size")).size(12.0))
                    .fill(if game_ready {
                        p.accent.gamma_multiply(0.2)
                    } else {
                        p.input.gamma_multiply(0.3)
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if game_ready { p.accent } else { p.border },
                    ))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(vec2(120.0, 26.0));

                if ui
                    .add_enabled(can_play_game, play_btn)
                    .on_hover_text(if game_ready {
                        t!("Play the Game-size version (wave 0)")
                    } else {
                        t!("Game-size version not downloaded — will auto-fallback to Short version")
                    })
                    .clicked()
                {
                    play_action = Some(JukeboxVersion::GameSize);
                }

                let export_btn = egui::Button::new(RichText::new(t!("💾 Export WAV")).size(12.0))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(vec2(90.0, 26.0));

                if ui
                    .add_enabled(game_ready && !self.gameaudio.is_exporting, export_btn)
                    .on_hover_text(t!(
                        "Export Game-size version to WAV (exports/audio/jukebox/)"
                    ))
                    .clicked()
                {
                    export_action = Some(JukeboxVersion::GameSize);
                }
            });
        });

        ui.add_space(14.0);

        if deselect {
            self.music.juke_selected_id = None;
        }
        if let Some(ver) = play_action {
            self.gameaudio.play_jukebox_song(song, ver);
        }
        if let Some(ver) = export_action {
            self.gameaudio.export_jukebox_song_wav(song, ver);
        }
    }

    /// Sidebar filter content for Live sub-tab.
    fn music_live_sidebar_content(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);

        // Search bar & collapse button
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("SEARCH LIVE")).size(11.0).color(p.weak));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button("◀")
                    .on_hover_text(t!("Fold the filters away, for more room"))
                    .clicked()
                {
                    self.music.live_sidebar_open = false;
                }
            });
        });
        ui.add_space(2.0);
        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut self.music.live_search)
                .hint_text(t!("Search live song or ID (Ctrl+F)"))
                .desired_width(f32::INFINITY),
        );
        if self.focus_search.get() {
            search_resp.request_focus();
            self.focus_search.set(false);
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);

        // Availability filter
        ui.label(RichText::new(t!("AVAILABILITY")).size(11.0).color(p.weak));
        ui.add_space(4.0);
        ui.checkbox(&mut self.music.live_only_downloaded, t!("Downloaded only"));

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);

        // Statistics
        ui.label(RichText::new(t!("STATISTICS")).size(11.0).color(p.weak));
        ui.add_space(4.0);
        let total_count = self.gameaudio.master.live_songs.len();
        ui.label(
            RichText::new(tf!("Total performances: {}", total_count))
                .size(12.0)
                .color(p.text),
        );

        let ready_count = self
            .gameaudio
            .master
            .live_songs
            .iter()
            .filter(|s| self.gameaudio.is_live_ready(s.music_id))
            .count();
        ui.label(
            RichText::new(tf!("Ready on disk: {}", ready_count))
                .size(12.0)
                .color(p.accent),
        );

        ui.add_space(10.0);
        if ui
            .button(t!("↺ Reset filters"))
            .on_hover_text(t!("Reset all search queries and filters to default"))
            .clicked()
        {
            self.music.live_search.clear();
            self.music.live_only_downloaded = false;
        }
    }

    /// Compact inline filter bar when the sidebar is folded away or on narrow screens.
    fn music_live_compact_filter_bar(&mut self, ui: &mut egui::Ui, has_selection: bool) {
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new("🔍").size(12.0))
                .on_hover_text(t!("Show the filters"))
                .clicked()
            {
                self.music.live_sidebar_open = true;
            }
            let search_w = if has_selection { 140.0 } else { 200.0 };
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.music.live_search)
                    .hint_text(t!("Search live song or ID (Ctrl+F)"))
                    .desired_width(search_w),
            );
            if self.focus_search.get() {
                search_resp.request_focus();
                self.focus_search.set(false);
            }
            ui.checkbox(&mut self.music.live_only_downloaded, t!("Downloaded only"));
            if !self.music.live_search.is_empty() || self.music.live_only_downloaded {
                if ui
                    .small_button("↺")
                    .on_hover_text(t!("Reset all search queries and filters to default"))
                    .clicked()
                {
                    self.music.live_search.clear();
                    self.music.live_only_downloaded = false;
                }
            }
        });
    }

    /// Song list table for Live performances.
    fn music_live_song_table(
        &mut self,
        ui: &mut egui::Ui,
        filtered_songs: &[crate::gameaudio::master::LiveData],
    ) {
        let p = pal(ui);
        ui.label(
            RichText::new(tf!("Showing {} performances", filtered_songs.len()))
                .size(12.0)
                .color(p.weak),
        );
        ui.add_space(4.0);

        if filtered_songs.is_empty() {
            empty_state(
                ui,
                "🎤",
                t!("No live performances found"),
                t!("Try adjusting your search terms or unchecking 'Downloaded only' filter."),
            );
            return;
        }

        let mut action_select = None;

        let table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(38.0)) // ID
            .column(Column::remainder().clip(true)) // Title
            .column(Column::exact(48.0)) // Performers count
            .column(Column::exact(72.0)) // Status
            .column(Column::exact(34.0)); // Action

        table
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.label(RichText::new("#").size(12.0).color(p.weak));
                });
                header.col(|ui| {
                    ui.label(RichText::new(t!("Song Title")).size(12.0).color(p.weak));
                });
                header.col(|ui| {
                    ui.label(RichText::new("👥").size(12.0).color(p.weak))
                        .on_hover_text(t!("Performers"));
                });
                header.col(|ui| {
                    ui.label(RichText::new(t!("Status")).size(12.0).color(p.weak));
                });
                header.col(|ui| {
                    ui.label(RichText::new("⚙").size(12.0).color(p.weak))
                        .on_hover_text(t!("Lineup & Setup"));
                });
            })
            .body(|body| {
                let count = filtered_songs.len();
                body.rows(28.0, count, |mut row| {
                    let song = &filtered_songs[row.index()];
                    let is_selected = self.music.live_selected_id == Some(song.music_id);
                    let is_ready = self.gameaudio.is_live_ready(song.music_id);

                    row.set_selected(is_selected);

                    // Col 0: ID
                    row.col(|ui| {
                        let resp = ui.label(
                            RichText::new(song.music_id.to_string())
                                .size(11.5)
                                .color(p.weak),
                        );
                        if resp.clicked() {
                            action_select = Some((song.music_id, song.live_member_number as usize));
                        }
                    });

                    // Col 1: Title (clickable row selection)
                    row.col(|ui| {
                        let text_color = if is_selected { p.accent } else { p.text };
                        let label = ui.selectable_label(
                            is_selected,
                            RichText::new(&song.title).color(text_color).size(13.0),
                        );
                        if label.clicked() {
                            action_select = Some((song.music_id, song.live_member_number as usize));
                        }
                    });

                    // Col 2: Performers count
                    row.col(|ui| {
                        let text = format!("{}", song.live_member_number);
                        let resp = ui.label(RichText::new(text).size(12.0).color(p.weak));
                        if resp.clicked() {
                            action_select = Some((song.music_id, song.live_member_number as usize));
                        }
                    });

                    // Col 3: Status
                    row.col(|ui| {
                        let badge = if is_ready {
                            ui.label(RichText::new(t!("✓ Ready")).size(11.5).color(p.accent))
                        } else {
                            ui.label(RichText::new(t!("⏳ Missing")).size(11.5).color(p.weak))
                        };
                        if badge.clicked() {
                            action_select = Some((song.music_id, song.live_member_number as usize));
                        }
                    });

                    // Col 4: Select / Setup (no auto-playback)
                    row.col(|ui| {
                        let btn = ui.small_button("⚙");
                        if btn
                            .on_hover_text(t!("Select and configure lineup"))
                            .clicked()
                        {
                            action_select = Some((song.music_id, song.live_member_number as usize));
                        }
                    });
                });
            });

        if let Some((mid, members)) = action_select {
            self.select_live_song(mid, members);
        }
    }

    /// Sub-tab Live: Adaptive filter sidebar, middle live performance list, right stage lineup & player.
    fn music_live_tab(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width();
        let avail_h = ui.available_height();
        let has_selection = self.music.live_selected_id.is_some();

        let search_term = self.music.live_search.trim().to_lowercase();
        let only_dl = self.music.live_only_downloaded;

        let filtered_songs: Vec<_> = self
            .gameaudio
            .master
            .live_songs
            .iter()
            .filter(|s| {
                if only_dl && !self.gameaudio.is_live_ready(s.music_id) {
                    return false;
                }
                if !search_term.is_empty() {
                    let id_str = s.music_id.to_string();
                    let title_lower = s.title.to_lowercase();
                    if !id_str.contains(&search_term) && !title_lower.contains(&search_term) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Very narrow layout (< 760px) with selection: stack table on top, detail panel below
        if avail_w < 760.0 && has_selection {
            let table_h = (avail_h * 0.40).clamp(160.0, 240.0);
            ui.allocate_ui_with_layout(
                vec2(avail_w, table_h),
                Layout::top_down(Align::Min),
                |ui| {
                    self.music_live_compact_filter_bar(ui, true);
                    ui.add_space(4.0);
                    self.music_live_song_table(ui, &filtered_songs);
                },
            );
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                vec2(avail_w, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| {
                    self.music_live_detail_panel(ui);
                },
            );
            return;
        }

        // Side-by-side layout:
        ui.horizontal(|ui| {
            // 1. Sidebar filter (if open)
            if self.music.live_sidebar_open {
                egui::Frame::new()
                    .inner_margin(Margin::same(10))
                    .fill(pal(ui).input.gamma_multiply(0.4))
                    .corner_radius(CornerRadius::same(10))
                    .stroke(Stroke::new(1.0, pal(ui).border))
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            vec2(210.0, ui.available_height()),
                            Layout::top_down(Align::Min),
                            |ui| {
                                self.music_live_sidebar_content(ui);
                            },
                        );
                    });
                ui.add_space(8.0);
            }

            // 2. Live songs list table (dynamic width)
            let rem_w = ui.available_width();
            let table_width = if has_selection {
                (rem_w * 0.38).clamp(280.0, 400.0)
            } else {
                rem_w
            };

            ui.allocate_ui_with_layout(
                vec2(table_width, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| {
                    if !self.music.live_sidebar_open {
                        self.music_live_compact_filter_bar(ui, has_selection);
                        ui.add_space(4.0);
                    }
                    self.music_live_song_table(ui, &filtered_songs);
                },
            );

            // 3. Right detail & stage lineup panel
            if has_selection {
                ui.add_space(8.0);
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), ui.available_height()),
                    Layout::top_down(Align::Min),
                    |ui| {
                        self.music_live_detail_panel(ui);
                    },
                );
            }
        });
    }

    /// Right detail and stage lineup control panel for selected Live performance.
    fn music_live_detail_panel(&mut self, ui: &mut egui::Ui) {
        let selected_id = match self.music.live_selected_id {
            Some(id) => id,
            None => return,
        };

        let song = match self
            .gameaudio
            .master
            .live_songs
            .iter()
            .find(|s| s.music_id == selected_id)
            .cloned()
        {
            Some(s) => s,
            None => return,
        };

        let p = pal(ui);
        let state = self.gameaudio.playback_state();
        let is_current_song = self.gameaudio.current_title().as_deref() == Some(&song.title);
        let is_current_playing = state == PlaybackState::Playing && is_current_song;
        let is_current_paused = state == PlaybackState::Paused && is_current_song;
        let is_current_loading = self.gameaudio.loading_live_id == Some(song.music_id);

        let mut deselect = false;
        let mut play_action = false;
        let mut export_action = false;
        let mut randomize_action = false;
        let mut reset_action = false;

        card(ui, |ui| {
            // Header
            ui.horizontal_wrapped(|ui| {
                ui.label(semibold(&song.title, 17.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new("✕").size(14.0).color(p.weak))
                        .on_hover_text(t!("Deselect live performance"))
                        .clicked()
                    {
                        deselect = true;
                    }
                    let badge = tf!("#{0} • {1} members", song.music_id, song.live_member_number);
                    ui.label(RichText::new(badge).size(12.0).color(p.weak));
                });
            });

            if let Some(credits) = &song.credits {
                ui.label(RichText::new(credits).size(11.5).color(p.weak));
            }

            ui.add_space(6.0);

            // Action toolbar: Play/Pause/Stop, Export WAV, Randomize Lineup, Reset Lineup
            // Wrapped so buttons never overflow horizontally in narrow widths
            ui.horizontal_wrapped(|ui| {
                let (play_icon, play_text, hover_hint) = if is_current_loading {
                    ("⏳", t!("Loading..."), t!("Decoding live tracks..."))
                } else if is_current_playing {
                    ("⏸", t!("Pause"), t!("Pause live playback"))
                } else if is_current_paused {
                    ("▶", t!("Resume Live"), t!("Resume paused live playback"))
                } else {
                    (
                        "▶",
                        t!("Play Live"),
                        t!("Start multi-track live performance"),
                    )
                };

                let play_btn = egui::Button::new(
                    RichText::new(format!("{play_icon} {play_text}"))
                        .size(13.0)
                        .color(if is_current_loading { p.weak } else { p.accent }),
                );
                if ui
                    .add_enabled(!is_current_loading, play_btn)
                    .on_hover_text(hover_hint)
                    .clicked()
                {
                    if is_current_playing {
                        self.gameaudio.pause();
                    } else if is_current_paused {
                        self.gameaudio.play();
                    } else {
                        play_action = true;
                    }
                }

                if ui
                    .button(RichText::new(format!("⏹ {}", t!("Stop"))).size(13.0))
                    .on_hover_text(t!("Stop"))
                    .clicked()
                {
                    self.gameaudio.stop();
                }

                ui.add_space(4.0);
                if ui
                    .button(RichText::new(t!("💾 Export Live WAV")).size(13.0))
                    .on_hover_text(t!(
                        "Export full multi-track live mix to exports/audio/live/"
                    ))
                    .clicked()
                {
                    export_action = true;
                }

                ui.add_space(4.0);
                if ui
                    .button(t!("🎲 Randomize Lineup"))
                    .on_hover_text(t!(
                        "Randomly shuffle permitted characters for this performance"
                    ))
                    .clicked()
                {
                    randomize_action = true;
                }

                if ui
                    .button(t!("🔄 Reset Lineup"))
                    .on_hover_text(t!("Reset lineup to default stage configuration"))
                    .clicked()
                {
                    reset_action = true;
                }
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            let mut lineup_changed = false;
            let mut controls_changed = false;
            let mut set_solo_slot: Option<usize> = None;
            let mut set_all_auto = false;
            let mut set_all_mute = false;
            let mut select_solo_chara: Option<i32> = None;

            // Scrollable lower section for FX, lyrics, and stage slots
            egui::ScrollArea::vertical()
                .id_salt("live_detail_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Audio FX (SFX 1 vs SFX 2) & Mute Controls
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(t!("AUDIO FX")).size(11.0).color(p.weak));
                        let sfx_opts = [
                            (1u8, t!("🔊 SFX 01 (Cheer)").to_string(), t!("Cheering & sound effects track")),
                            (2u8, t!("🎵 SFX 02 (Clean)").to_string(), t!("Clean instrumental track without audience cheer")),
                        ];
                        let prev_sfx = self.music.live_sfx;
                        segmented(ui, &mut self.music.live_sfx, &sfx_opts);
                        if self.music.live_sfx != prev_sfx {
                            if is_current_playing {
                                play_action = true;
                            }
                        }

                        ui.add_space(14.0);

                        // Master mutes
                        let mut mute_oke = self.music.live_mute_oke;
                        if ui.checkbox(&mut mute_oke, t!("🔇 Mute Oke (Vocals only)")).changed() {
                            self.music.live_mute_oke = mute_oke;
                            if is_current_song {
                                self.sync_live_mixer_controls();
                            }
                        }

                        let mut mute_vocals = self.music.live_mute_vocals;
                        if ui.checkbox(&mut mute_vocals, t!("🔇 Mute Vocals (Instrumental only)")).changed() {
                            self.music.live_mute_vocals = mute_vocals;
                            if is_current_song {
                                self.sync_live_mixer_controls();
                            }
                        }

                        ui.add_space(14.0);
                        ui.label(RichText::new(t!("Vocal volume:")).size(11.5).color(p.text));
                        let mut vocal_pct = (self.music.live_vocal_volume * 100.0).round() as u32;
                        let vocal_slider = egui::Slider::new(&mut vocal_pct, 0..=100)
                            .suffix("%")
                            .trailing_fill(true);
                        let resp = ui.add_sized(Vec2::new(90.0, 16.0), vocal_slider);
                        if resp.dragged() || resp.changed() {
                            self.music.live_vocal_volume = (vocal_pct as f32 / 100.0).clamp(0.0, 1.0);
                            if is_current_song {
                                self.sync_live_mixer_controls();
                            }
                        }
                        resp.on_hover_text(t!("Overall volume for all live performers; scales per-slot volumes"));

                        ui.add_space(14.0);
                        ui.checkbox(&mut self.music.live_show_lyrics, t!("Show lyrics"))
                            .on_hover_text(t!("Toggle synchronized karaoke lyrics display"));
                    });

                    ui.add_space(8.0);

                    // Synchronized Karaoke Lyrics: full list tracking the mixer clock
                    if self.music.live_show_lyrics {
                        self.music_live_lyrics_panel(ui);
                        ui.add_space(10.0);
                    } else {
                        self.music.live_lyrics_anchor = None;
                    }

                    // Performer Lineup Stage (Slots)
                    let candidates = self
                        .gameaudio
                        .active_catalog()
                        .live_chara_candidates(song.music_id);
                    let member_count = song.live_member_number as usize;
                    let center_idx = (member_count.saturating_sub(1)) / 2;

                    let total_slots = self.music.live_lineup.len();
                    let num_mute = self
                        .music
                        .live_lineup
                        .iter()
                        .filter(|s| s.mode == SingingMode::Mute)
                        .count();
                    let num_auto = self
                        .music
                        .live_lineup
                        .iter()
                        .filter(|s| s.mode == SingingMode::Auto)
                        .count();
                    let main_slot = self
                        .music
                        .live_lineup
                        .iter()
                        .position(|s| s.mode == SingingMode::ForceMain);
                    let has_single_main = main_slot.is_some()
                        && (total_slots > 0 && num_mute == total_slots.saturating_sub(1));
                    let is_all_auto = total_slots > 0 && num_auto == total_slots;
                    let is_all_mute = total_slots > 0 && num_mute == total_slots;
                    let is_custom = !has_single_main && !is_all_auto && !is_all_mute;

                    // Vocal Mode Toolbar & Guidance Box
                    egui::Frame::new()
                        .fill(p.surface.gamma_multiply(0.85))
                        .corner_radius(CornerRadius::same(8))
                        .stroke(Stroke::new(1.0, p.border))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(t!("Vocal mode:"))
                                            .size(12.0)
                                            .color(p.text)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);

                                    // Solo Vocal button
                                    let solo_btn = if has_single_main {
                                        egui::Button::new(
                                            RichText::new(t!("🎤 Solo Vocal"))
                                                .size(12.0)
                                                .color(p.accent)
                                                .strong(),
                                        )
                                        .fill(p.accent.gamma_multiply(0.18))
                                        .stroke(Stroke::new(1.4, p.accent))
                                    } else {
                                        egui::Button::new(
                                            RichText::new(t!("🎤 Solo Vocal")).size(12.0).color(p.text),
                                        )
                                    };
                                    if ui
                                        .add(solo_btn)
                                        .on_hover_text(t!(
                                            "Choose one Uma as exclusive main singer; mute all other slots"
                                        ))
                                        .clicked()
                                    {
                                        let target = main_slot.unwrap_or(center_idx);
                                        set_solo_slot = Some(target);
                                    }

                                    // Full Cast (Auto Part) button
                                    let auto_btn = if is_all_auto {
                                        egui::Button::new(
                                            RichText::new(t!("👥 Full Cast (Auto Part)"))
                                                .size(12.0)
                                                .color(p.accent)
                                                .strong(),
                                        )
                                        .fill(p.accent.gamma_multiply(0.18))
                                        .stroke(Stroke::new(1.4, p.accent))
                                    } else {
                                        egui::Button::new(
                                            RichText::new(t!("👥 Full Cast (Auto Part)"))
                                                .size(12.0)
                                                .color(p.text),
                                        )
                                    };
                                    if ui
                                        .add(auto_btn)
                                        .on_hover_text(t!(
                                            "Perform original stage score triggers across all slots"
                                        ))
                                        .clicked()
                                    {
                                        set_all_auto = true;
                                    }

                                    // Instrumental button
                                    let inst_btn = if is_all_mute {
                                        egui::Button::new(
                                            RichText::new(t!("🎼 Instrumental"))
                                                .size(12.0)
                                                .color(p.accent)
                                                .strong(),
                                        )
                                        .fill(p.accent.gamma_multiply(0.18))
                                        .stroke(Stroke::new(1.4, p.accent))
                                    } else {
                                        egui::Button::new(
                                            RichText::new(t!("🎼 Instrumental"))
                                                .size(12.0)
                                                .color(p.text),
                                        )
                                    };
                                    if ui
                                        .add(inst_btn)
                                        .on_hover_text(t!(
                                            "Mute all singers and play instrumental accompaniment only"
                                        ))
                                        .clicked()
                                    {
                                        set_all_mute = true;
                                    }

                                    if is_custom {
                                        ui.label(
                                            RichText::new(t!("⚙ Custom"))
                                                .size(11.0)
                                                .color(p.weak),
                                        );
                                    }

                                    // Quick single-singer selector when solo mode is active
                                    if has_single_main {
                                        ui.add_space(8.0);
                                        let curr_target = main_slot.unwrap();
                                        let curr_chara_id = self
                                            .music
                                            .live_lineup
                                            .get(curr_target)
                                            .map(|s| s.chara_id)
                                            .unwrap_or(0);
                                        let curr_name = self
                                            .music
                                            .live_lineup
                                            .get(curr_target)
                                            .map(|s| s.name.clone())
                                            .unwrap_or_default();

                                        ui.label(
                                            RichText::new(t!("Solo singer:"))
                                                .size(11.5)
                                                .color(p.accent)
                                                .strong(),
                                        );
                                        egui::ComboBox::from_id_salt("quick_solo_singer_selector")
                                            .selected_text(RichText::new(&curr_name).size(12.0))
                                            .width(170.0)
                                            .show_ui(ui, |ui| {
                                                for cand in &candidates {
                                                    let cname = self
                                                        .gameaudio
                                                        .master
                                                        .chara_name(cand.chara_id)
                                                        .map(String::from)
                                                        .unwrap_or_else(|| tf!("Chara #{}", cand.chara_id));
                                                    let is_sel = cand.chara_id == curr_chara_id;
                                                    ui.horizontal(|ui| {
                                                        if ui.selectable_label(is_sel, &cname).clicked() {
                                                            select_solo_chara = Some(cand.chara_id);
                                                        }
                                                        if cand.voice_ready {
                                                            ui.label(
                                                                RichText::new(t!("✓ Ready"))
                                                                    .size(10.0)
                                                                    .color(p.accent),
                                                            );
                                                        } else {
                                                            ui.label(
                                                                RichText::new(t!("⏳ Missing"))
                                                                    .size(10.0)
                                                                    .color(p.weak),
                                                            );
                                                        }
                                                    });
                                                }
                                            });
                                        ui.label(
                                            RichText::new(tf!("(Slot {})", curr_target + 1))
                                                .size(11.0)
                                                .color(p.weak),
                                        );
                                    }
                                });

                                ui.add_space(3.0);

                                // Guidance banner
                                if is_all_mute {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(t!(
                                                "🎼 Instrumental mode (No vocal selected). Playing live will play the accompaniment only. Select [🎤 Solo Vocal] or [👥 Full Cast] to enable singing."
                                            ))
                                            .size(11.5)
                                            .color(p.weak),
                                        );
                                    });
                                } else if has_single_main {
                                    let curr_target = main_slot.unwrap();
                                    let solo_slot = &self.music.live_lineup[curr_target];
                                    let is_ready = self
                                        .gameaudio
                                        .is_live_chara_ready(song.music_id, solo_slot.chara_id);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(tf!(
                                                "🎤 Solo Vocal: {} (Slot {}). All other stage slots are muted for clean single-singer playback.",
                                                &solo_slot.name,
                                                curr_target + 1
                                            ))
                                            .size(11.5)
                                            .color(p.accent),
                                        );
                                        if !is_ready && solo_slot.chara_id > 0 {
                                            ui.label(
                                                RichText::new(t!("⚠ Voice file is not downloaded in game yet."))
                                                    .size(11.0)
                                                    .color(p.warn),
                                            );
                                        }
                                    });
                                } else if is_all_auto {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(t!(
                                                "👥 Full Cast (Auto Part): All performers sing according to original stage part triggers."
                                            ))
                                            .size(11.5)
                                            .color(p.weak),
                                        );
                                    });
                                } else if candidates.is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(t!(
                                                "ℹ This live song has no vocal data in game files (Instrumental only)."
                                            ))
                                            .size(11.5)
                                            .color(p.weak),
                                        );
                                    });
                                }
                            });
                        });
                    ui.add_space(8.0);

                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(t!("STAGE PERFORMER LINEUP")).size(11.0).color(p.weak));
                        ui.add_space(10.0);

                        match main_slot {
                            Some(i) => {
                                let slot = &self.music.live_lineup[i];
                                let voice_ready =
                                    self.gameaudio.is_live_chara_ready(song.music_id, slot.chara_id);
                                ui.label(RichText::new(t!("🎤 Main vocal:")).size(11.5).color(p.accent).strong())
                                    .on_hover_text(t!("This performer is forced to sing the main vocal"));

                                let avatar_dim = 22.0;
                                let (avatar_rect, avatar_resp) = ui.allocate_exact_size(vec2(avatar_dim, avatar_dim), egui::Sense::hover());
                                let avatar_tex = if slot.chara_id > 0 {
                                    self.music.textures.get_or_request_avatar(
                                        &self.gameaudio.game_path,
                                        slot.chara_id,
                                        &self.gameaudio.manifest_entries,
                                        self.root.as_deref(),
                                    )
                                } else {
                                    None
                                };
                                if let Some(tex) = avatar_tex {
                                    egui::Image::from_texture(tex)
                                        .corner_radius(CornerRadius::same(5))
                                        .paint_at(ui, avatar_rect);
                                    ui.painter().rect_stroke(
                                        avatar_rect,
                                        CornerRadius::same(5),
                                        Stroke::new(1.2, p.accent),
                                        egui::StrokeKind::Inside,
                                    );
                                } else {
                                    ui.painter().rect_filled(avatar_rect, CornerRadius::same(5), p.input.gamma_multiply(0.4));
                                    ui.painter().text(
                                        avatar_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "👤",
                                        egui::FontId::proportional(11.0),
                                        p.weak,
                                    );
                                    ui.painter().rect_stroke(
                                        avatar_rect,
                                        CornerRadius::same(5),
                                        Stroke::new(1.0, p.border.gamma_multiply(0.8)),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                avatar_resp.on_hover_text(tf!(
                                    "{0} - {1} (Slot {2})",
                                    t!("Main vocal avatar"),
                                    &slot.name,
                                    i + 1
                                ));

                                let name = ui.label(RichText::new(&slot.name).size(11.5).color(if voice_ready {
                                    p.text
                                } else {
                                    p.warn
                                }));
                                if !voice_ready {
                                    name.on_hover_text(t!("Main vocal voice file is not downloaded in game yet."));
                                }
                                ui.label(RichText::new(tf!("Slot {}", i + 1)).size(11.0).color(p.weak));
                            }
                            None => {
                                ui.label(RichText::new(t!("🎤 Main vocal:")).size(11.0).color(p.weak))
                                    .on_hover_text(t!("Follows stage part triggers dynamically"));
                                ui.label(RichText::new(t!("Auto (Part)")).size(11.0).color(p.weak));
                            }
                        }
                    });
                    ui.add_space(4.0);

                    for slot_idx in 0..self.music.live_lineup.len() {
                        let is_center = slot_idx == center_idx;
                        let slot = &mut self.music.live_lineup[slot_idx];
                        let is_forced_main = slot.mode == SingingMode::ForceMain;
                        let is_ready = self.gameaudio.is_live_chara_ready(song.music_id, slot.chara_id);

                        egui::Frame::new()
                            .fill(if is_forced_main {
                                p.accent.gamma_multiply(0.12)
                            } else if is_center {
                                p.accent.gamma_multiply(0.08)
                            } else {
                                p.input.gamma_multiply(0.2)
                            })
                            .stroke(Stroke::new(
                                if is_forced_main {
                                    1.8
                                } else if is_center {
                                    1.5
                                } else {
                                    1.0
                                },
                                if is_forced_main || is_center { p.accent } else { p.border },
                            ))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    // Slot position indicator
                                    let slot_label = if is_center {
                                        tf!("★ Slot {} (Center)", slot_idx + 1)
                                    } else {
                                        tf!("Slot {}", slot_idx + 1)
                                    };
                                    ui.label(RichText::new(slot_label).size(12.5).color(if is_center { p.accent } else { p.weak }).strong());

                                    if is_forced_main {
                                        ui.label(
                                            RichText::new(t!("🎤 [Main Vocal]"))
                                                .size(11.0)
                                                .color(p.accent)
                                                .strong(),
                                        )
                                        .on_hover_text(t!("This performer is forced to sing the main vocal"));
                                    }

                                    if !slot.main_vocal_enabled {
                                        ui.label(
                                            RichText::new(t!("🔇 [Main Off]"))
                                                .size(11.0)
                                                .color(p.weak),
                                        )
                                        .on_hover_text(t!("Main vocal wave is muted for this performer"));
                                    }

                                    // Performer avatar thumbnail
                                    let avatar_dim = 26.0;
                                    let (avatar_rect, avatar_resp) = ui.allocate_exact_size(vec2(avatar_dim, avatar_dim), egui::Sense::hover());
                                    let avatar_tex = if slot.chara_id > 0 {
                                        self.music.textures.get_or_request_avatar(
                                            &self.gameaudio.game_path,
                                            slot.chara_id,
                                            &self.gameaudio.manifest_entries,
                                            self.root.as_deref(),
                                        )
                                    } else {
                                        None
                                    };
                                    if let Some(tex) = avatar_tex {
                                        egui::Image::from_texture(tex)
                                            .corner_radius(CornerRadius::same(6))
                                            .paint_at(ui, avatar_rect);
                                        ui.painter().rect_stroke(
                                            avatar_rect,
                                            CornerRadius::same(6),
                                            Stroke::new(if is_forced_main { 1.8 } else { 1.0 }, if is_forced_main { p.accent } else { p.border }),
                                            egui::StrokeKind::Inside,
                                        );
                                    } else {
                                        ui.painter().rect_filled(avatar_rect, CornerRadius::same(6), p.input.gamma_multiply(0.4));
                                        ui.painter().text(
                                            avatar_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "👤",
                                            egui::FontId::proportional(12.0),
                                            p.weak,
                                        );
                                        ui.painter().rect_stroke(
                                            avatar_rect,
                                            CornerRadius::same(6),
                                            Stroke::new(if is_forced_main { 1.5 } else { 1.0 }, if is_forced_main { p.accent } else { p.border.gamma_multiply(0.6) }),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    let role_str = if is_forced_main {
                                        t!("🎤 Main vocal")
                                    } else if is_center {
                                        t!("★ Center")
                                    } else {
                                        t!("Auto (Part)")
                                    };
                                    avatar_resp.on_hover_text(tf!("{0} - {1} (Slot {2}) · {3}", t!("Performer avatar"), &slot.name, slot_idx + 1, role_str));

                                    // Character selector combobox
                                    let curr_chara_id = slot.chara_id;
                                    let curr_name = slot.name.clone();

                                    egui::ComboBox::from_id_salt(format!("slot_chara_{slot_idx}"))
                                        .selected_text(RichText::new(&curr_name).size(12.5))
                                        .width(200.0)
                                        .show_ui(ui, |ui| {
                                            for cand in &candidates {
                                                let cname = self
                                                    .gameaudio
                                                    .master
                                                    .chara_name(cand.chara_id)
                                                    .map(String::from)
                                                    .unwrap_or_else(|| tf!("Chara #{}", cand.chara_id));
                                                let is_sel = cand.chara_id == curr_chara_id;
                                                ui.horizontal(|ui| {
                                                    let (mini_rect, mini_resp) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
                                                    let mini_tex = if cand.chara_id > 0 {
                                                        self.music.textures.get_or_request_avatar(
                                                            &self.gameaudio.game_path,
                                                            cand.chara_id,
                                                            &self.gameaudio.manifest_entries,
                                                            self.root.as_deref(),
                                                        )
                                                    } else {
                                                        None
                                                    };
                                                    if let Some(tex) = mini_tex {
                                                        egui::Image::from_texture(tex)
                                                            .corner_radius(CornerRadius::same(3))
                                                            .paint_at(ui, mini_rect);
                                                    } else {
                                                        ui.painter().rect_filled(mini_rect, CornerRadius::same(3), p.input.gamma_multiply(0.4));
                                                        ui.painter().text(mini_rect.center(), egui::Align2::CENTER_CENTER, "👤", egui::FontId::proportional(9.0), p.weak);
                                                    }
                                                    mini_resp.on_hover_text(format!("{} (#{})", &cname, cand.chara_id));
                                                    if ui.selectable_label(is_sel, &cname).clicked() {
                                                        slot.chara_id = cand.chara_id;
                                                        slot.name = cname;
                                                        lineup_changed = true;
                                                    }
                                                    if cand.voice_ready {
                                                        ui.label(RichText::new(t!("✓ Ready")).size(10.0).color(p.accent));
                                                    } else {
                                                        ui.label(RichText::new(t!("⏳ Missing")).size(10.0).color(p.weak));
                                                    }
                                                });
                                            }
                                        });

                                    // Status badge
                                    if slot.chara_id == 0 {
                                        ui.label(RichText::new(t!("No vocal data")).size(11.0).color(p.weak));
                                    } else if is_ready {
                                        ui.label(RichText::new(t!("✓ Ready")).size(11.0).color(p.accent));
                                    } else {
                                        ui.label(RichText::new(t!("⏳ Missing")).size(11.0).color(p.weak))
                                            .on_hover_text(t!("Missing vocal file in game"));
                                    }

                                    ui.add_space(6.0);

                                    // Quick action button: set as the exclusive single main vocal
                                    let is_exclusive_solo = has_single_main && main_slot == Some(slot_idx);
                                    let solo_btn = if is_exclusive_solo {
                                        egui::Button::new(
                                            RichText::new(t!("🎤 Solo [Active]")).size(11.5).color(p.accent).strong(),
                                        )
                                        .fill(p.accent.gamma_multiply(0.2))
                                        .stroke(Stroke::new(1.3, p.accent))
                                    } else {
                                        egui::Button::new(
                                            RichText::new(t!("🎤 Set as Solo")).size(11.5).color(p.weak),
                                        )
                                    };

                                    if ui
                                        .add(solo_btn)
                                        .on_hover_text(if is_exclusive_solo {
                                            t!("Currently the exclusive main singer. Click to mute and switch to Instrumental.")
                                        } else {
                                            t!("Set this performer as the exclusive main singer; mutes all other stage slots.")
                                        })
                                        .clicked()
                                    {
                                        set_solo_slot = Some(slot_idx);
                                    }

                                    // Singing mode combobox
                                    let mode_text = match slot.mode {
                                        SingingMode::Auto => t!("Auto (Part)"),
                                        SingingMode::ForceMain => t!("Main"),
                                        SingingMode::ForceExtra => t!("Extra"),
                                        SingingMode::ForceMerge => t!("Merge"),
                                        SingingMode::Mute => t!("Mute"),
                                    };

                                    egui::ComboBox::from_id_salt(format!("slot_mode_{slot_idx}"))
                                        .selected_text(RichText::new(mode_text).size(11.5))
                                        .width(95.0)
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_value(&mut slot.mode, SingingMode::Auto, t!("Auto (Part)")).clicked() {
                                                controls_changed = true;
                                            }
                                            if ui.selectable_value(&mut slot.mode, SingingMode::ForceMain, t!("Main")).clicked() {
                                                controls_changed = true;
                                            }
                                            if ui.selectable_value(&mut slot.mode, SingingMode::ForceExtra, t!("Extra")).clicked() {
                                                controls_changed = true;
                                            }
                                            if ui.selectable_value(&mut slot.mode, SingingMode::ForceMerge, t!("Merge")).clicked() {
                                                controls_changed = true;
                                            }
                                            if ui.selectable_value(&mut slot.mode, SingingMode::Mute, t!("Mute")).clicked() {
                                                controls_changed = true;
                                            }
                                        });

                                    // Main vocal wave toggle checkbox
                                    let is_main_vocal = slot.main_vocal_enabled;
                                    let main_cb = ui.checkbox(
                                        &mut slot.main_vocal_enabled,
                                        RichText::new(t!("Main vocal")).size(11.5).color(if is_main_vocal { p.text } else { p.weak }),
                                    )
                                    .on_hover_text(t!("Toggle the main vocal wave (Wave 0) for this performer. When off, only extra/harmony voice plays if active."));
                                    if main_cb.changed() {
                                        controls_changed = true;
                                    }

                                    // Solo button
                                    let solo_color = if slot.solo { p.accent } else { p.weak };
                                    if ui
                                        .button(RichText::new("⭐ Solo").size(11.5).color(solo_color))
                                        .on_hover_text(t!("Solo this performer (mutes other non-solo singers)"))
                                        .clicked()
                                    {
                                        slot.solo = !slot.solo;
                                        controls_changed = true;
                                    }

                                    // Volume slider
                                    let vol_slider = egui::Slider::new(&mut slot.volume, 0.0..=1.0)
                                        .show_value(false)
                                        .trailing_fill(true);
                                    let resp = ui.add_sized(Vec2::new(60.0, 16.0), vol_slider);
                                    if resp.dragged() || resp.clicked() {
                                        controls_changed = true;
                                    }
                                    let pct = (slot.volume * 100.0).round() as u32;
                                    ui.label(RichText::new(format!("{pct}%")).size(11.0).color(p.weak));
                                });
                            });
                        ui.add_space(4.0);
                    }
                });

            let curr_main_slot = self
                .music
                .live_lineup
                .iter()
                .position(|s| s.mode == SingingMode::ForceMain);
            let curr_has_single_main = curr_main_slot.is_some()
                && (!self.music.live_lineup.is_empty()
                    && self
                        .music
                        .live_lineup
                        .iter()
                        .filter(|s| s.mode == SingingMode::Mute)
                        .count()
                        == self.music.live_lineup.len().saturating_sub(1));
            let center_slot_idx = ((song.live_member_number as usize).saturating_sub(1)) / 2;

            if let Some(target_idx) = set_solo_slot {
                let is_already_exclusive_solo =
                    curr_has_single_main && curr_main_slot == Some(target_idx);
                if is_already_exclusive_solo {
                    if let Some(s) = self.music.live_lineup.get_mut(target_idx) {
                        s.mode = SingingMode::Mute;
                    }
                } else {
                    for (i, s) in self.music.live_lineup.iter_mut().enumerate() {
                        if i == target_idx {
                            s.mode = SingingMode::ForceMain;
                            s.solo = false;
                        } else {
                            s.mode = SingingMode::Mute;
                            s.solo = false;
                        }
                    }
                }
                controls_changed = true;
            }

            if let Some(new_cid) = select_solo_chara {
                let target_idx = curr_main_slot.unwrap_or(center_slot_idx);
                if let Some(s) = self.music.live_lineup.get_mut(target_idx) {
                    if s.chara_id != new_cid {
                        s.chara_id = new_cid;
                        s.name = self
                            .gameaudio
                            .master
                            .chara_name(new_cid)
                            .map(String::from)
                            .unwrap_or_else(|| tf!("Chara #{new_cid}"));
                        s.mode = SingingMode::ForceMain;
                        s.solo = false;
                        lineup_changed = true;
                    }
                }
                for (i, s) in self.music.live_lineup.iter_mut().enumerate() {
                    if i != target_idx {
                        s.mode = SingingMode::Mute;
                        s.solo = false;
                    }
                }
                controls_changed = true;
            }

            if set_all_auto {
                for s in self.music.live_lineup.iter_mut() {
                    s.mode = SingingMode::Auto;
                    s.solo = false;
                }
                controls_changed = true;
            }

            if set_all_mute {
                for s in self.music.live_lineup.iter_mut() {
                    s.mode = SingingMode::Mute;
                    s.solo = false;
                }
                controls_changed = true;
            }

            if controls_changed && is_current_song {
                self.sync_live_mixer_controls();
            }
            if lineup_changed && is_current_playing {
                play_action = true;
            }
        });

        if deselect {
            self.music.live_selected_id = None;
        }
        if randomize_action {
            self.randomize_live_lineup(song.music_id, song.live_member_number as usize);
            if is_current_playing {
                play_action = true;
            }
        }
        if reset_action {
            self.init_live_lineup(song.music_id, song.live_member_number as usize);
            if is_current_playing {
                play_action = true;
            }
        }
        if play_action {
            self.start_live_playback(song.music_id, song.title.clone());
        }
        if export_action {
            let lineup = self.live_lineup_effective();
            self.gameaudio.export_live_song_wav(
                song.music_id,
                song.title.clone(),
                self.music.live_sfx,
                lineup,
                self.music.live_mute_oke,
                self.music.live_mute_vocals,
            );
        }
    }

    /// Scrollable synchronized karaoke lyrics list for the current live performance.
    ///
    /// Highlights the line active at the mixer's playback position, auto-scrolls
    /// when that line changes (and on song changes), and explains why lyrics are
    /// unavailable instead of showing an empty panel.
    fn music_live_lyrics_panel(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let gameaudio = &self.gameaudio;
        let music = &mut self.music;

        egui::Frame::new()
            .fill(p.surface.gamma_multiply(0.8))
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, p.accent.gamma_multiply(0.5)))
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!("🎤 Synchronized Karaoke Lyrics"))
                                .size(11.0)
                                .color(p.accent),
                        );
                        if let Some(score) = &gameaudio.current_live_score {
                            let pos_ms = (gameaudio.position_secs() * 1000.0) as u32;
                            if let Some(trig) = score.part_at(pos_ms) {
                                let active_indices: Vec<_> = trig
                                    .members
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, m)| m.track > 0)
                                    .map(|(idx, _)| idx)
                                    .collect();
                                if !active_indices.is_empty() {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(tf!(
                                                "Active voices: {}",
                                                active_indices.len()
                                            ))
                                            .size(11.0)
                                            .color(p.weak),
                                        );
                                    });
                                }
                            }
                        }
                    });

                    ui.add_space(4.0);

                    let Some(score) = gameaudio.current_live_score.as_ref() else {
                        ui.label(
                            RichText::new(t!(
                                "Play a Live performance to see synchronized lyrics."
                            ))
                            .size(13.0)
                            .color(p.weak),
                        );
                        return;
                    };

                    if Some(score.music_id) != music.live_selected_id {
                        ui.label(
                            RichText::new(t!(
                                "Play this Live performance to see synchronized lyrics."
                            ))
                            .size(13.0)
                            .color(p.weak),
                        );
                        return;
                    }

                    match &score.lyrics_status {
                        LyricsLoadState::Failed(reason) => {
                            ui.label(
                                RichText::new(tf!(
                                    "⚠ {}: {}",
                                    t!("Could not read lyrics data"),
                                    reason
                                ))
                                .size(13.0)
                                .color(p.weak),
                            );
                        }
                        LyricsLoadState::NoScoreAsset => {
                            ui.label(
                                RichText::new(t!(
                                    "This song has no lyrics data in the game files."
                                ))
                                .size(13.0)
                                .color(p.weak),
                            );
                        }
                        LyricsLoadState::NotDownloaded => {
                            ui.label(
                                RichText::new(t!("Lyrics data is not downloaded in game yet."))
                                    .size(13.0)
                                    .color(p.weak),
                            );
                        }
                        LyricsLoadState::NotLoaded | LyricsLoadState::Loaded { lyric_lines: 0 } => {
                            ui.label(
                                RichText::new(t!("This song has no lyrics (instrumental)."))
                                    .size(13.0)
                                    .color(p.weak),
                            );
                        }
                        LyricsLoadState::Loaded { .. } => {
                            let pos_ms = (gameaudio.position_secs() * 1000.0).max(0.0) as u32;
                            let active_idx = score.lyrics_index_at(pos_ms);

                            if let Some(idx) = active_idx {
                                ui.label(
                                    RichText::new(tf!("Line {}/{}", idx + 1, score.lyrics.len()))
                                        .size(11.0)
                                        .color(p.weak),
                                );
                                ui.add_space(4.0);
                            }

                            // Auto-scroll only when the anchor changes, so manual
                            // scrolling is never fought: a new score jumps to the
                            // top, a changed active line centers itself.
                            let anchor_now = (score.music_id, active_idx);
                            let scroll_now = music.live_lyrics_anchor != Some(anchor_now);
                            let scroll_line = match music.live_lyrics_anchor {
                                None => active_idx,
                                Some((mid, _)) if mid != score.music_id => None,
                                Some(_) => active_idx,
                            };
                            music.live_lyrics_anchor = Some(anchor_now);

                            egui::ScrollArea::vertical()
                                .id_salt("live_lyrics_list")
                                .max_height(210.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for (idx, trig) in score.lyrics.iter().enumerate() {
                                        let is_active = Some(idx) == active_idx;
                                        let near_active =
                                            active_idx.is_some_and(|a| idx.abs_diff(a) <= 2);
                                        let text = if is_active {
                                            RichText::new(&trig.text)
                                                .size(15.0)
                                                .color(p.accent)
                                                .strong()
                                        } else if near_active {
                                            RichText::new(&trig.text).size(12.5).color(p.text)
                                        } else {
                                            RichText::new(&trig.text).size(12.5).color(p.weak)
                                        };
                                        let resp = ui.label(text);
                                        if scroll_now {
                                            if scroll_line == Some(idx) {
                                                resp.scroll_to_me(Some(Align::Center));
                                            } else if scroll_line.is_none() && idx == 0 {
                                                resp.scroll_to_me(Some(Align::Min));
                                            }
                                        }
                                    }
                                });
                        }
                    }
                });
            });
    }

    /// Selects a live performance track and initializes its performer lineup.
    pub(crate) fn select_live_song(&mut self, music_id: i32, member_count: usize) {
        if self.music.live_selected_id != Some(music_id) {
            self.music.live_selected_id = Some(music_id);
            self.init_live_lineup(music_id, member_count);
        }
    }

    /// Initializes performer stage slots from real lineup candidates
    /// (permission table + manifest voice assets). Slots stay without vocal
    /// data when the catalog has none for this song; no ids are fabricated.
    pub(crate) fn init_live_lineup(&mut self, music_id: i32, member_count: usize) {
        let ids: Vec<i32> = self
            .gameaudio
            .active_catalog()
            .live_chara_candidates(music_id)
            .into_iter()
            .map(|c| c.chara_id)
            .collect();
        let mut lineup = Vec::with_capacity(member_count);

        for i in 0..member_count {
            let (cid, name) = if ids.is_empty() {
                (0, t!("No vocal data").to_string())
            } else {
                let cid = ids[i % ids.len()];
                let name = self
                    .gameaudio
                    .master
                    .chara_name(cid)
                    .map(String::from)
                    .unwrap_or_else(|| tf!("Chara #{cid}"));
                (cid, name)
            };

            lineup.push(crate::gameaudio::PerformerSlotConfig {
                chara_id: cid,
                name,
                mode: SingingMode::Mute,
                solo: false,
                volume: 1.0,
                main_vocal_enabled: true,
            });
        }

        self.music.live_lineup = lineup;
    }

    /// Randomly assigns permitted characters to live stage slots.
    pub(crate) fn randomize_live_lineup(&mut self, music_id: i32, member_count: usize) {
        let ids: Vec<i32> = self
            .gameaudio
            .active_catalog()
            .live_chara_candidates(music_id)
            .into_iter()
            .map(|c| c.chara_id)
            .collect();
        if ids.is_empty() {
            return;
        }

        let mut candidates = ids;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as usize)
            .unwrap_or(42);

        let mut chosen = Vec::new();
        for step in 0..member_count {
            let idx = (seed + step * 31) % candidates.len();
            let cid = candidates[idx];
            if candidates.len() > 1 {
                candidates.remove(idx);
            }
            chosen.push(cid);
        }

        for (i, &cid) in chosen.iter().enumerate() {
            if i < self.music.live_lineup.len() {
                let name = self
                    .gameaudio
                    .master
                    .chara_name(cid)
                    .map(String::from)
                    .unwrap_or_else(|| tf!("Chara #{cid}"));
                self.music.live_lineup[i].chara_id = cid;
                self.music.live_lineup[i].name = name;
            }
        }
    }

    /// Resolves live stage lineup with effective performer volumes
    /// scaled by the overall vocal volume gain.
    pub(crate) fn live_lineup_effective(&self) -> Vec<PerformerSlotConfig> {
        let gain = if self.music.live_vocal_volume.is_finite() {
            self.music.live_vocal_volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let mut lineup = self.music.live_lineup.clone();
        for slot in &mut lineup {
            let slot_vol = if slot.volume.is_finite() {
                slot.volume
            } else {
                1.0
            };
            slot.volume = (slot_vol * gain).clamp(0.0, 1.0);
        }
        lineup
    }

    /// Synchronizes live mixer controls (modes, solo, volumes, mutes) in real-time.
    ///
    /// Controls are keyed by slot index so two slots holding the same Uma
    /// stay independently controllable.
    pub(crate) fn sync_live_mixer_controls(&self) {
        let gain = if self.music.live_vocal_volume.is_finite() {
            self.music.live_vocal_volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let controls: Vec<_> = self
            .music
            .live_lineup
            .iter()
            .enumerate()
            .map(|(slot, s)| {
                let slot_vol = if s.volume.is_finite() { s.volume } else { 1.0 };
                PerformerControlUpdate {
                    slot,
                    mode: s.mode,
                    solo: s.solo,
                    volume: (slot_vol * gain).clamp(0.0, 1.0),
                    main_vocal_enabled: s.main_vocal_enabled,
                }
            })
            .collect();

        self.gameaudio.update_live_mixer(
            self.music.live_mute_oke,
            self.music.live_mute_vocals,
            controls,
        );
    }

    /// Starts multi-track playback of selected live song.
    pub(crate) fn start_live_playback(&mut self, music_id: i32, title: String) {
        let lineup = self.live_lineup_effective();
        self.gameaudio.play_live_song(
            music_id,
            title,
            self.music.live_sfx,
            lineup,
            self.music.live_mute_oke,
            self.music.live_mute_vocals,
            true,
        );
    }

    /// Mini-player fixed at the bottom of the Music tab.
    fn music_mini_player(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let state = self.gameaudio.playback_state();
        let pos = self.gameaudio.position_secs();
        let dur = self.gameaudio.duration_secs();
        let title = self
            .gameaudio
            .current_title()
            .unwrap_or_else(|| t!("Track").to_string());
        let mut vol = self.gameaudio.volume();
        let mut is_lp = self.gameaudio.is_loop();

        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(14, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(38.0);

                    // Track title & Status
                    let status_icon = match state {
                        PlaybackState::Playing => "▶",
                        PlaybackState::Paused => "⏸",
                        PlaybackState::Stopped => "⏹",
                    };
                    ui.label(RichText::new(status_icon).size(16.0).color(p.accent));
                    ui.add_space(4.0);

                    let title_text = RichText::new(&title).size(13.5).color(p.text);
                    ui.label(title_text).on_hover_text(&title);

                    ui.add_space(12.0);

                    // Playback Controls
                    let play_pause_icon = if state == PlaybackState::Playing {
                        "⏸"
                    } else {
                        "▶"
                    };
                    if ui
                        .button(RichText::new(play_pause_icon).size(15.0))
                        .on_hover_text(if state == PlaybackState::Playing {
                            t!("Pause")
                        } else {
                            t!("Play")
                        })
                        .clicked()
                    {
                        if state == PlaybackState::Playing {
                            self.gameaudio.pause();
                        } else {
                            self.gameaudio.play();
                        }
                    }

                    if ui
                        .button(RichText::new("⏹").size(15.0))
                        .on_hover_text(t!("Stop"))
                        .clicked()
                    {
                        self.gameaudio.stop();
                    }

                    ui.add_space(8.0);

                    // Progress bar & Time
                    let time_str = format!(
                        "{:02}:{:02} / {:02}:{:02}",
                        (pos / 60.0) as u32,
                        (pos % 60.0) as u32,
                        (dur / 60.0) as u32,
                        (dur % 60.0) as u32
                    );
                    ui.label(RichText::new(time_str).size(12.0).color(p.weak));

                    let mut seek_pos = pos;
                    let slider = egui::Slider::new(&mut seek_pos, 0.0..=dur.max(0.1))
                        .show_value(false)
                        .trailing_fill(true);
                    let resp = ui.add_sized(Vec2::new(160.0, 20.0), slider);
                    if resp.dragged() || resp.clicked() {
                        self.gameaudio.seek(seek_pos);
                    }

                    ui.add_space(8.0);

                    // Loop toggle
                    if ui
                        .checkbox(&mut is_lp, t!("Loop"))
                        .on_hover_text(t!("Seamless track repeat"))
                        .changed()
                    {
                        self.gameaudio.set_loop(is_lp);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Dismiss button
                        if ui
                            .button(RichText::new("✕").size(13.0).color(p.weak))
                            .on_hover_text(t!("Close player"))
                            .clicked()
                        {
                            self.gameaudio.stop();
                        }

                        // Volume slider
                        ui.add_space(4.0);
                        let vol_slider = egui::Slider::new(&mut vol, 0.0..=3.0)
                            .show_value(false)
                            .trailing_fill(true);
                        let vol_resp = ui.add_sized(Vec2::new(70.0, 18.0), vol_slider);
                        if vol_resp.dragged() || vol_resp.clicked() {
                            self.request_set_volume(vol);
                        }

                        let vol_pct = (vol * 100.0).round() as u32;
                        ui.label(
                            RichText::new(format!("🔊 {vol_pct}%"))
                                .size(12.0)
                                .color(p.weak),
                        );
                    });
                });
            });
    }
}
