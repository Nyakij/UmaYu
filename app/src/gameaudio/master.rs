//! Reads game catalog tables from `master/master.mdb`.
//!
//! Extracts live performance info, chara stage permissions, jukebox tracks, and localized text.

use std::collections::HashMap;
use std::path::Path;

/// Primary music title text category in `text_data` (used for both Live performances and Jukebox songs).
pub const TEXT_CATEGORY_MUSIC_NAME: i32 = 16;
/// Music credits (lyrics/composer) text category in `text_data`.
pub const TEXT_CATEGORY_MUSIC_CREDITS: i32 = 17;
/// Character name category in `text_data`.
pub const TEXT_CATEGORY_CHARA_NAME: i32 = 6;
/// English character name category in `text_data` (Global).
pub const TEXT_CATEGORY_CHARA_NAME_EN: i32 = 372;
/// Secondary Jukebox song title category in `text_data` (fallback for Jukebox entries).
pub const TEXT_CATEGORY_JUKEBOX_NAME: i32 = 210;

/// A live performance track definition from `live_data`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiveData {
    pub music_id: i32,
    pub sort: i32,
    pub music_type: i32,
    pub live_member_number: i32,
    pub condition_type: i32,
    pub has_live: i32,
    pub title: String,
    pub credits: Option<String>,
}

/// A character permitted to stand on stage for a live track from `live_permission_data`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LivePermissionData {
    pub music_id: i32,
    pub chara_id: i32,
}

/// A jukebox music track definition from `jukebox_music_data`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JukeboxMusicData {
    pub music_id: i32,
    pub sort: i32,
    pub bgm_cue_name_short: Option<String>,
    pub bgm_cuesheet_name_short: Option<String>,
    pub bgm_cue_name_gamesize: Option<String>,
    pub bgm_cuesheet_name_gamesize: Option<String>,
    pub short_length: i32,
    pub is_hidden: i32,
    pub title: String,
}

/// Aggregate master data loaded from `master/master.mdb`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MasterData {
    pub live_songs: Vec<LiveData>,
    pub live_permissions: Vec<LivePermissionData>,
    pub jukebox_songs: Vec<JukeboxMusicData>,
    pub chara_names: HashMap<i32, String>,
    pub live_perm_index: HashMap<i32, Vec<i32>>,
    pub has_live_data: bool,
    pub has_live_permission_data: bool,
    pub has_jukebox_data: bool,
    pub has_text_data: bool,
}

impl MasterData {
    /// Returns characters allowed to perform in the given live music track.
    #[allow(dead_code)]
    pub fn allowed_charas_for_live(&self, music_id: i32) -> &[i32] {
        self.live_perm_index
            .get(&music_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Looks up a character's name by `chara_id`.
    #[allow(dead_code)]
    pub fn chara_name(&self, chara_id: i32) -> Option<&str> {
        self.chara_names.get(&chara_id).map(|s| s.as_str())
    }
}

/// Reads all relevant tables from `master.mdb`.
pub fn load_master_data(master_path: &Path) -> Result<MasterData, String> {
    if !master_path.is_file() {
        return Err(format!(
            "Master database not found: {}",
            master_path.display()
        ));
    }

    let conn = rusqlite::Connection::open_with_flags(
        master_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("Failed to open master database: {e}"))?;

    let mut master = MasterData::default();

    // 1. Read text_data for song titles and chara names
    let mut titles: HashMap<i32, String> = HashMap::new();
    let mut juke_fallback_titles: HashMap<i32, String> = HashMap::new();
    let mut credits: HashMap<i32, String> = HashMap::new();
    let mut chara_names: HashMap<i32, String> = HashMap::new();

    let text_query =
        "SELECT category, `index`, text FROM text_data WHERE category IN (6, 16, 17, 210, 372);";
    if let Ok(mut stmt) = conn.prepare(text_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let cat: i32 = row.get(0)?;
            let idx: i32 = row.get(1)?;
            let txt: String = row.get(2)?;
            Ok((cat, idx, txt))
        }) {
            for r in rows.flatten() {
                match r.0 {
                    TEXT_CATEGORY_MUSIC_NAME => {
                        titles.insert(r.1, r.2);
                    }
                    TEXT_CATEGORY_JUKEBOX_NAME => {
                        juke_fallback_titles.insert(r.1, r.2);
                    }
                    TEXT_CATEGORY_MUSIC_CREDITS => {
                        credits.insert(r.1, r.2);
                    }
                    TEXT_CATEGORY_CHARA_NAME_EN => {
                        // Prioritize English character names
                        chara_names.insert(r.1, r.2);
                    }
                    TEXT_CATEGORY_CHARA_NAME => {
                        // Fallback if not already populated by En
                        chara_names.entry(r.1).or_insert(r.2);
                    }
                    _ => {}
                }
            }
            master.has_text_data = true;
        }
    }
    master.chara_names = chara_names;

    // 2. Read live_data
    let live_query = "SELECT music_id, sort, music_type, live_member_number, condition_type, has_live FROM live_data ORDER BY sort ASC, music_id ASC;";
    if let Ok(mut stmt) = conn.prepare(live_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let music_id: i32 = row.get(0)?;
            let sort: i32 = row.get(1)?;
            let music_type: i32 = row.get(2)?;
            let live_member_number: i32 = row.get(3)?;
            let condition_type: i32 = row.get(4)?;
            let has_live: i32 = row.get(5)?;
            Ok((
                music_id,
                sort,
                music_type,
                live_member_number,
                condition_type,
                has_live,
            ))
        }) {
            for r in rows.flatten() {
                let (music_id, sort, music_type, live_member_number, condition_type, has_live) = r;
                let title = titles
                    .get(&music_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Live {music_id}"));
                let song_credits = credits.get(&music_id).cloned();

                master.live_songs.push(LiveData {
                    music_id,
                    sort,
                    music_type,
                    live_member_number,
                    condition_type,
                    has_live,
                    title,
                    credits: song_credits,
                });
            }
            master.has_live_data = true;
        }
    }

    // 3. Read live_permission_data
    let perm_query = "SELECT music_id, chara_id FROM live_permission_data;";
    if let Ok(mut stmt) = conn.prepare(perm_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok(LivePermissionData {
                music_id: row.get(0)?,
                chara_id: row.get(1)?,
            })
        }) {
            for r in rows.flatten() {
                master
                    .live_perm_index
                    .entry(r.music_id)
                    .or_default()
                    .push(r.chara_id);
                master.live_permissions.push(r);
            }
            master.has_live_permission_data = true;
        }
    }

    // 4. Read jukebox_music_data
    let juke_query = "SELECT music_id, sort, bgm_cue_name_short, bgm_cuesheet_name_short, bgm_cue_name_gamesize, bgm_cuesheet_name_gamesize, short_length, is_hidden FROM jukebox_music_data ORDER BY sort ASC, music_id ASC;";
    if let Ok(mut stmt) = conn.prepare(juke_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let music_id: i32 = row.get(0)?;
            let sort: i32 = row.get(1)?;
            let bgm_cue_name_short: Option<String> = row.get(2)?;
            let bgm_cuesheet_name_short: Option<String> = row.get(3)?;
            let bgm_cue_name_gamesize: Option<String> = row.get(4)?;
            let bgm_cuesheet_name_gamesize: Option<String> = row.get(5)?;
            let short_length: i32 = row.get(6)?;
            let is_hidden: i32 = row.get(7)?;
            Ok((
                music_id,
                sort,
                bgm_cue_name_short,
                bgm_cuesheet_name_short,
                bgm_cue_name_gamesize,
                bgm_cuesheet_name_gamesize,
                short_length,
                is_hidden,
            ))
        }) {
            for r in rows.flatten() {
                let (
                    music_id,
                    sort,
                    bgm_cue_name_short,
                    bgm_cuesheet_name_short,
                    bgm_cue_name_gamesize,
                    bgm_cuesheet_name_gamesize,
                    short_length,
                    is_hidden,
                ) = r;
                let title = titles
                    .get(&music_id)
                    .or_else(|| juke_fallback_titles.get(&music_id))
                    .cloned()
                    .unwrap_or_else(|| format!("Music {music_id}"));

                master.jukebox_songs.push(JukeboxMusicData {
                    music_id,
                    sort,
                    bgm_cue_name_short,
                    bgm_cuesheet_name_short,
                    bgm_cue_name_gamesize,
                    bgm_cuesheet_name_gamesize,
                    short_length,
                    is_hidden,
                    title,
                });
            }
            master.has_jukebox_data = true;
        }
    }

    Ok(master)
}
