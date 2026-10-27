//! Unity asset bundle and music score parsing.
//!
//! Extracts TextAsset CSV files (`m{id}_lyrics` and `m{id}_part`) from UnityFS containers
//! using LZ4 decompression, and parses lyrics and multi-member part triggers with center-out stage mapping.

#![allow(dead_code)]

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::gameaudio::meta::ManifestEntry;

/// A timed lyrics line trigger for synchronized karaoke display.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LyricsTrigger {
    pub time_ms: u32,
    pub text: String,
}

/// Vocal track assignment for an individual stage performer in a part trigger.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemberPart {
    /// Track selection: 0 = silent/off, 1 = main vocal (wave 0), 2 = extra vocal / harmony (wave 1).
    pub track: u8,
    /// Volume override in range [0.0, 1.0], or 999.0 if unset.
    pub volume: f32,
    /// Stereo panning (-1.0 to 1.0), or 999.0 if unset.
    pub pan: f32,
}

impl Default for MemberPart {
    fn default() -> Self {
        Self {
            track: 1,
            volume: 999.0,
            pan: 999.0,
        }
    }
}

/// A timed vocal part trigger defining which performers sing at a specific timestamp.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PartTrigger {
    pub time_ms: u32,
    pub members: Vec<MemberPart>,
    pub volume_rate: f32,
}

/// Outcome of resolving lyric data for a live performance, so the UI can tell
/// missing assets, not-yet-downloaded files, and read/parse failures apart
/// from songs that simply have no lyrics.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LyricsLoadState {
    /// No live score has been loaded yet.
    #[default]
    NotLoaded,
    /// Lyrics parsed successfully; `lyric_lines` is the number of timed lines.
    Loaded { lyric_lines: usize },
    /// The region catalog has no lyrics asset entry for this song.
    NoScoreAsset,
    /// The lyrics asset is listed in the manifest but its file is not on disk.
    NotDownloaded,
    /// The lyrics asset could not be read, decompressed, or parsed.
    Failed(String),
}

/// Consolidated music score for a live performance.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LiveScore {
    pub music_id: i32,
    pub member_count: usize,
    pub lyrics: Vec<LyricsTrigger>,
    pub parts: Vec<PartTrigger>,
    pub lyrics_status: LyricsLoadState,
}

impl LiveScore {
    /// Returns the index of the active lyrics line at the given playback
    /// position in milliseconds, or `None` before the first line has triggered.
    pub fn lyrics_index_at(&self, time_ms: u32) -> Option<usize> {
        if self.lyrics.is_empty() {
            return None;
        }
        match self.lyrics.binary_search_by_key(&time_ms, |t| t.time_ms) {
            Ok(i) => Some(i),
            Err(0) => None,
            Err(i) => Some(i - 1),
        }
    }

    /// Returns the active part trigger at the given playback position in milliseconds.
    pub fn part_at(&self, time_ms: u32) -> Option<&PartTrigger> {
        if self.parts.is_empty() {
            return None;
        }
        let idx = match self.parts.binary_search_by_key(&time_ms, |t| t.time_ms) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
        self.parts.get(idx)
    }
}

/// Maps a center-out CSV member column index (0 = center, 1 = right, 2 = left, etc.)
/// to an absolute stage position (0 to member_count - 1).
pub fn center_out_to_stage_pos(csv_idx: usize, member_count: usize) -> usize {
    if member_count <= 1 || csv_idx == 0 {
        return (member_count.saturating_sub(1)) / 2;
    }
    let center = (member_count - 1) / 2;
    let offset = ((csv_idx + 1) / 2) as isize;
    let pos = if csv_idx % 2 == 1 {
        center as isize + offset
    } else {
        center as isize - offset
    };
    pos.clamp(0, (member_count - 1) as isize) as usize
}

/// Maps an absolute stage position (0 to member_count - 1) to a center-out CSV member column index.
pub fn stage_pos_to_center_out(stage_idx: usize, member_count: usize) -> usize {
    if member_count <= 1 {
        return 0;
    }
    let center = (member_count - 1) / 2;
    let diff = stage_idx as isize - center as isize;
    if diff == 0 {
        0
    } else if diff > 0 {
        (diff as usize) * 2 - 1
    } else {
        ((-diff) as usize) * 2
    }
}

/// Reads a null-terminated ASCII/UTF-8 string from raw bytes at cursor.
fn read_null_terminated_string(bytes: &[u8], cursor: &mut usize) -> Result<String, String> {
    let start = *cursor;
    while *cursor < bytes.len() && bytes[*cursor] != 0 {
        *cursor += 1;
    }
    if *cursor >= bytes.len() {
        return Err("Unterminated string in UnityFS header".to_string());
    }
    let s = String::from_utf8_lossy(&bytes[start..*cursor]).into_owned();
    *cursor += 1; // skip null byte
    Ok(s)
}

/// Extracts the raw TextAsset string from a UnityFS asset bundle archive.
pub fn extract_text_asset_from_bundle(bundle_bytes: &[u8]) -> Result<String, String> {
    if bundle_bytes.len() < 16 || &bundle_bytes[0..7] != b"UnityFS" {
        return Err("Not a valid UnityFS bundle header".to_string());
    }

    let mut cursor = 8usize;
    if cursor + 4 > bundle_bytes.len() {
        return Err("Unexpected EOF reading version".to_string());
    }
    let _version = u32::from_be_bytes(bundle_bytes[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;

    let _unity_ver = read_null_terminated_string(bundle_bytes, &mut cursor)?;
    let _unity_rev = read_null_terminated_string(bundle_bytes, &mut cursor)?;

    if cursor + 20 > bundle_bytes.len() {
        return Err("Unexpected EOF reading header sizes".to_string());
    }
    let file_size = i64::from_be_bytes(bundle_bytes[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;

    let compressed_metadata_size =
        u32::from_be_bytes(bundle_bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4;
    let uncompressed_metadata_size =
        u32::from_be_bytes(bundle_bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4;
    let flags = u32::from_be_bytes(bundle_bytes[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;

    if (flags & 0x100) != 0 {
        cursor = (cursor + 15) & !15;
    }

    let meta_offset = if (flags & 0x80) != 0 {
        (file_size as usize).saturating_sub(compressed_metadata_size)
    } else {
        cursor
    };

    let data_offset_start = if (flags & 0x80) != 0 {
        cursor
    } else {
        meta_offset + compressed_metadata_size
    };

    if meta_offset + compressed_metadata_size > bundle_bytes.len() {
        return Err("Metadata block exceeds file length".to_string());
    }

    let comp_type = flags & 0x3F;
    let meta_bytes = match comp_type {
        0 => bundle_bytes[meta_offset..meta_offset + compressed_metadata_size].to_vec(),
        2 | 3 => lz4_flex::decompress(
            &bundle_bytes[meta_offset..meta_offset + compressed_metadata_size],
            uncompressed_metadata_size,
        )
        .map_err(|e| format!("LZ4 metadata decompression error: {e}"))?,
        _ => {
            return Err(format!(
                "Unsupported metadata compression type: {comp_type}"
            ));
        }
    };

    let mut m_cursor = 16usize; // Skip 16-byte GUID
    if m_cursor + 4 > meta_bytes.len() {
        return Err("Invalid metadata block length".to_string());
    }
    let num_blocks =
        i32::from_be_bytes(meta_bytes[m_cursor..m_cursor + 4].try_into().unwrap()) as usize;
    m_cursor += 4;

    let mut blocks = Vec::with_capacity(num_blocks);
    for _ in 0..num_blocks {
        if m_cursor + 10 > meta_bytes.len() {
            return Err("Invalid block entry".to_string());
        }
        let uncompressed_sz =
            u32::from_be_bytes(meta_bytes[m_cursor..m_cursor + 4].try_into().unwrap()) as usize;
        m_cursor += 4;
        let compressed_sz =
            u32::from_be_bytes(meta_bytes[m_cursor..m_cursor + 4].try_into().unwrap()) as usize;
        m_cursor += 4;
        let b_flags = u16::from_be_bytes(meta_bytes[m_cursor..m_cursor + 2].try_into().unwrap());
        m_cursor += 2;
        blocks.push((uncompressed_sz, compressed_sz, b_flags));
    }

    if m_cursor + 4 > meta_bytes.len() {
        return Err("Invalid directory count in metadata".to_string());
    }
    let num_nodes =
        i32::from_be_bytes(meta_bytes[m_cursor..m_cursor + 4].try_into().unwrap()) as usize;
    m_cursor += 4;

    let mut nodes = Vec::with_capacity(num_nodes);
    for _ in 0..num_nodes {
        if m_cursor + 20 > meta_bytes.len() {
            return Err("Invalid directory entry in metadata".to_string());
        }
        let node_offset =
            i64::from_be_bytes(meta_bytes[m_cursor..m_cursor + 8].try_into().unwrap()) as usize;
        m_cursor += 8;
        let node_size =
            i64::from_be_bytes(meta_bytes[m_cursor..m_cursor + 8].try_into().unwrap()) as usize;
        m_cursor += 8;
        let node_flags = u32::from_be_bytes(meta_bytes[m_cursor..m_cursor + 4].try_into().unwrap());
        m_cursor += 4;
        let node_path = read_null_terminated_string(&meta_bytes, &mut m_cursor)?;
        nodes.push((node_offset, node_size, node_flags, node_path));
    }

    let mut block_stream_cursor = data_offset_start;
    let mut uncompressed_data = Vec::new();
    for (uncomp_sz, comp_sz, b_flags) in blocks {
        if block_stream_cursor + comp_sz > bundle_bytes.len() {
            return Err("Data block out of bounds in bundle".to_string());
        }
        let block_slice = &bundle_bytes[block_stream_cursor..block_stream_cursor + comp_sz];
        let block_comp = b_flags & 0x3F;
        match block_comp {
            0 => {
                uncompressed_data.extend_from_slice(block_slice);
            }
            2 | 3 => {
                let decomp = lz4_flex::decompress(block_slice, uncomp_sz)
                    .map_err(|e| format!("LZ4 data block decompression error: {e}"))?;
                uncompressed_data.extend_from_slice(&decomp);
            }
            _ => {
                return Err(format!(
                    "Unsupported block compression format: {block_comp}"
                ));
            }
        }
        block_stream_cursor += comp_sz;
    }

    for (offset, size, _, _path) in nodes {
        if offset + size <= uncompressed_data.len() {
            let file_bytes = &uncompressed_data[offset..offset + size];
            if let Ok(text) = extract_text_asset_from_serialized_file(file_bytes) {
                return Ok(text);
            }
        }
    }

    extract_text_asset_heuristic(&uncompressed_data)
}

/// Parses a Unity SerializedFile and extracts the TextAsset object content.
fn extract_text_asset_from_serialized_file(file_bytes: &[u8]) -> Result<String, String> {
    if file_bytes.len() < 24 {
        return Err("Serialized file too short".to_string());
    }

    let _metadata_size = u32::from_be_bytes(file_bytes[0..4].try_into().unwrap());
    let _file_size = u32::from_be_bytes(file_bytes[4..8].try_into().unwrap());
    let version = u32::from_be_bytes(file_bytes[8..12].try_into().unwrap());
    let data_offset = u32::from_be_bytes(file_bytes[12..16].try_into().unwrap()) as usize;

    if data_offset > 0 && data_offset < file_bytes.len() {
        let payload = &file_bytes[data_offset..];
        if payload.len() >= 8 {
            let name_len = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
            if name_len < 256 && 4 + name_len <= payload.len() {
                let aligned_name = (name_len + 3) & !3;
                let script_offset = 4 + aligned_name;
                if script_offset + 4 <= payload.len() {
                    let script_len = u32::from_le_bytes(
                        payload[script_offset..script_offset + 4]
                            .try_into()
                            .unwrap(),
                    ) as usize;
                    let script_start = script_offset + 4;
                    if script_len > 0 && script_start + script_len <= payload.len() {
                        let script_bytes = &payload[script_start..script_start + script_len];
                        if let Ok(s) = std::str::from_utf8(script_bytes) {
                            if s.contains(',') || s.contains('\n') {
                                return Ok(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    Err(format!(
        "Could not extract TextAsset from SerializedFile version {version}"
    ))
}

/// Scans decompressed data bytes for UTF-8 CSV content as a resilient fallback.
fn extract_text_asset_heuristic(data: &[u8]) -> Result<String, String> {
    let mut i = 0;
    while i + 8 < data.len() {
        let candidate_len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        if candidate_len >= 16 && candidate_len <= 1_000_000 && i + 4 + candidate_len <= data.len()
        {
            let slice = &data[i + 4..i + 4 + candidate_len];
            if let Ok(text) = std::str::from_utf8(slice) {
                if (text.starts_with("time,") || text.starts_with("0,") || text.contains(",\""))
                    && text.contains('\n')
                {
                    return Ok(text.to_string());
                }
            }
        }
        i += 4;
    }
    Err("TextAsset CSV signature not found in decompressed bundle".to_string())
}

/// Parses lyrics CSV text into time-sorted `LyricsTrigger` list.
///
/// Tolerates a UTF-8 BOM, a `time` header row, CRLF line endings, and quoted
/// cells; see [`parse_lyrics_csv_counted`] for row-level details.
pub fn parse_lyrics_csv(csv_text: &str) -> Vec<LyricsTrigger> {
    parse_lyrics_csv_counted(csv_text).0
}

/// Parses lyrics CSV text and also reports how many non-header data rows were
/// attempted, so callers can distinguish "no lyric rows at all" from "rows
/// existed but none parsed".
pub fn parse_lyrics_csv_counted(csv_text: &str) -> (Vec<LyricsTrigger>, usize) {
    let body = csv_text.strip_prefix('\u{feff}').unwrap_or(csv_text);
    let mut triggers = Vec::new();
    let mut data_rows = 0usize;

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, ',');
        let time_str = match parts.next() {
            Some(t) => t.trim().trim_matches('"'),
            None => continue,
        };
        if time_str.eq_ignore_ascii_case("time") {
            continue;
        }

        let time_ms: u32 = match time_str.parse() {
            Ok(t) => t,
            Err(_) => {
                data_rows += 1;
                continue;
            }
        };

        let raw_text = match parts.next() {
            Some(t) => t.trim(),
            None => {
                data_rows += 1;
                continue;
            }
        };

        let unquoted =
            if raw_text.starts_with('"') && raw_text.ends_with('"') && raw_text.len() >= 2 {
                &raw_text[1..raw_text.len() - 1]
            } else {
                raw_text
            };

        let clean_text = unquoted
            .replace("\"\"", "\"")
            .replace("[COMMA]", ",")
            .replace("&&", "&")
            .replace("\\n", "\n")
            .replace('\r', "");

        data_rows += 1;
        if !clean_text.is_empty() {
            triggers.push(LyricsTrigger {
                time_ms,
                text: clean_text,
            });
        }
    }

    triggers.sort_by_key(|t| t.time_ms);
    (triggers, data_rows)
}

/// Parses vocal part trigger CSV text into sorted `PartTrigger` list.
pub fn parse_part_csv(csv_text: &str, expected_members: usize) -> Vec<PartTrigger> {
    let mut triggers = Vec::new();
    let lines: Vec<&str> = csv_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return triggers;
    }

    // Determine column schema from first line / data lines
    let mut data_start_idx = 0;
    if lines[0].starts_with("time") || lines[0].starts_with("Time") {
        data_start_idx = 1;
    }

    for &line in &lines[data_start_idx..] {
        let tokens: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if tokens.is_empty() {
            continue;
        }

        let time_ms: u32 = match tokens[0].parse() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let remaining = &tokens[1..];
        let mut members = Vec::with_capacity(expected_members);
        let mut volume_rate = 1.0f32;

        if remaining.is_empty() {
            continue;
        }

        // Check if last token is volume_rate
        let (member_tokens, has_rate) = if remaining.len() > expected_members
            && (remaining.len() == expected_members + 1
                || remaining.len() == expected_members * 3 + 1)
        {
            (&remaining[..remaining.len() - 1], true)
        } else {
            (remaining, false)
        };

        if has_rate {
            if let Some(rate_str) = remaining.last() {
                if let Ok(r) = rate_str.parse::<f32>() {
                    volume_rate = r;
                }
            }
        }

        // Schema 1: 1 column per member (track: 0, 1, 2)
        // Schema 2: 3 columns per member (track, volume, pan)
        let is_three_col = member_tokens.len() >= expected_members * 3;

        for m_idx in 0..expected_members {
            if is_three_col {
                let base = m_idx * 3;
                let track = member_tokens
                    .get(base)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                let vol = member_tokens
                    .get(base + 1)
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(999.0);
                let pan = member_tokens
                    .get(base + 2)
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(999.0);
                members.push(MemberPart {
                    track,
                    volume: vol,
                    pan,
                });
            } else {
                let track = member_tokens
                    .get(m_idx)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                members.push(MemberPart {
                    track,
                    volume: 999.0,
                    pan: 999.0,
                });
            }
        }

        triggers.push(PartTrigger {
            time_ms,
            members,
            volume_rate,
        });
    }

    triggers.sort_by_key(|t| t.time_ms);
    triggers
}

/// Finds the first manifest entry matching any of the candidate asset names.
fn find_manifest_entry<'a>(
    manifest: &'a [ManifestEntry],
    names: &[String],
) -> Option<&'a ManifestEntry> {
    names
        .iter()
        .find_map(|n| manifest.iter().find(|e| e.name.eq_ignore_ascii_case(n)))
}

/// Reads and extracts the TextAsset CSV from a manifest entry's bundle file,
/// classifying a missing file as not-downloaded and other failures as errors.
fn read_text_asset(game_dir: &Path, entry: &ManifestEntry) -> Result<String, LyricsLoadState> {
    let path = entry.file_path(game_dir);
    if !path.is_file() {
        return Err(LyricsLoadState::NotDownloaded);
    }
    let mut bytes = Vec::new();
    File::open(&path)
        .and_then(|mut f| f.read_to_end(&mut bytes))
        .map_err(|e| LyricsLoadState::Failed(format!("read error: {e}")))?;
    extract_text_asset_from_bundle(&bytes).map_err(LyricsLoadState::Failed)
}

/// Loads lyrics and part triggers for a live music track from game asset storage.
///
/// Lyric availability is classified into [`LiveScore::lyrics_status`] so the UI
/// can explain missing lyrics instead of silently showing an empty score.
/// Part triggers stay best-effort: a missing or unreadable part bundle only
/// means the Auto singing mode has no triggers to follow.
pub fn load_live_score(
    game_dir: &Path,
    music_id: i32,
    member_count: usize,
    manifest: &[ManifestEntry],
) -> LiveScore {
    let lyrics_names = [
        format!("live/musicscores/m{music_id}/m{music_id}_lyrics"),
        format!("live/musicscores/m{:04}/m{:04}_lyrics", music_id, music_id),
    ];
    let part_names = [
        format!("live/musicscores/m{music_id}/m{music_id}_part"),
        format!("live/musicscores/m{:04}/m{:04}_part", music_id, music_id),
    ];

    let (lyrics, lyrics_status) = match find_manifest_entry(manifest, &lyrics_names) {
        None => (Vec::new(), LyricsLoadState::NoScoreAsset),
        Some(entry) => match read_text_asset(game_dir, entry) {
            Err(state) => (Vec::new(), state),
            Ok(csv_text) => {
                let (triggers, data_rows) = parse_lyrics_csv_counted(&csv_text);
                let state = if triggers.is_empty() && data_rows > 0 {
                    LyricsLoadState::Failed(
                        "score CSV has rows but none parsed as lyrics".to_string(),
                    )
                } else {
                    LyricsLoadState::Loaded {
                        lyric_lines: triggers.len(),
                    }
                };
                (triggers, state)
            }
        },
    };

    let mut parts = Vec::new();
    if let Some(entry) = find_manifest_entry(manifest, &part_names) {
        if let Ok(csv_text) = read_text_asset(game_dir, entry) {
            parts = parse_part_csv(&csv_text, member_count);
        }
    }

    LiveScore {
        music_id,
        member_count,
        lyrics,
        parts,
        lyrics_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_entry(id: i64, name: &str, hash_name: &str) -> ManifestEntry {
        ManifestEntry {
            id,
            name: name.to_string(),
            dependencies: None,
            group: 0,
            length: 0,
            checksum: 0,
            hash_name: hash_name.to_string(),
            kind: 0,
            state: 1,
            priority: 0,
        }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("umayu_unity_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_hashed_asset(game_dir: &Path, hash_name: &str, bytes: &[u8]) {
        let path = game_dir.join("dat").join(&hash_name[0..2]).join(hash_name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn parse_lyrics_csv_handles_bom_header_crlf_and_quotes() {
        let csv = "\u{feff}time,text\r\n1000,\"Hello[COMMA] world\"\r\n2000,\"Say \"\"hey\"\"\"\r\n3000,plain row\r\n";
        let triggers = parse_lyrics_csv(csv);
        assert_eq!(triggers.len(), 3);
        assert_eq!(triggers[0].time_ms, 1000);
        assert_eq!(triggers[0].text, "Hello, world");
        assert_eq!(triggers[1].time_ms, 2000);
        assert_eq!(triggers[1].text, "Say \"hey\"");
        assert_eq!(triggers[2].time_ms, 3000);
        assert_eq!(triggers[2].text, "plain row");
    }

    #[test]
    fn parse_lyrics_csv_sorts_and_keeps_duplicate_times() {
        let csv = "5000,B\n1000,A\n5000,C\n";
        let triggers = parse_lyrics_csv(csv);
        let times: Vec<u32> = triggers.iter().map(|t| t.time_ms).collect();
        assert_eq!(times, vec![1000, 5000, 5000]);
        assert_eq!(triggers[1].text, "B");
        assert_eq!(triggers[2].text, "C");
    }

    #[test]
    fn parse_lyrics_csv_counted_distinguishes_unparseable_rows() {
        // Header-only CSV: zero data rows means the score simply has no lyrics.
        let (triggers, rows) = parse_lyrics_csv_counted("time,text\n");
        assert!(triggers.is_empty());
        assert_eq!(rows, 0);

        // Data rows exist but none carry a parseable timestamp.
        let (triggers, rows) = parse_lyrics_csv_counted("time,text\nxx,bad row\nyy,worse row\n");
        assert!(triggers.is_empty());
        assert_eq!(rows, 2);
    }

    #[test]
    fn lyrics_index_at_tracks_playback_cursor() {
        let score = LiveScore {
            music_id: 1,
            member_count: 1,
            lyrics: vec![
                LyricsTrigger {
                    time_ms: 1000,
                    text: "a".into(),
                },
                LyricsTrigger {
                    time_ms: 3000,
                    text: "b".into(),
                },
                LyricsTrigger {
                    time_ms: 7000,
                    text: "c".into(),
                },
            ],
            parts: Vec::new(),
            lyrics_status: LyricsLoadState::Loaded { lyric_lines: 3 },
        };

        assert_eq!(score.lyrics_index_at(0), None);
        assert_eq!(score.lyrics_index_at(999), None);
        assert_eq!(score.lyrics_index_at(1000), Some(0));
        assert_eq!(score.lyrics_index_at(2999), Some(0));
        assert_eq!(score.lyrics_index_at(3000), Some(1));
        assert_eq!(score.lyrics_index_at(6999), Some(1));
        assert_eq!(score.lyrics_index_at(999_999), Some(2));

        let empty = LiveScore::default();
        assert_eq!(empty.lyrics_index_at(5000), None);
    }

    #[test]
    fn load_live_score_classifies_missing_entry_and_missing_file() {
        let dir = temp_dir("classify");
        let manifest = vec![manifest_entry(7, "live/musicscores/m99/m99_lyrics", "ab12")];

        let no_entry = load_live_score(&dir, 55, 1, &manifest);
        assert_eq!(no_entry.lyrics_status, LyricsLoadState::NoScoreAsset);
        assert!(no_entry.lyrics.is_empty());

        let not_downloaded = load_live_score(&dir, 99, 1, &manifest);
        assert_eq!(not_downloaded.lyrics_status, LyricsLoadState::NotDownloaded);
        assert!(not_downloaded.lyrics.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_live_score_reports_unreadable_bundle() {
        let dir = temp_dir("unreadable");
        let manifest = vec![manifest_entry(7, "live/musicscores/m99/m99_lyrics", "cd34")];
        write_hashed_asset(&dir, "cd34", b"definitely not a UnityFS bundle");

        let score = load_live_score(&dir, 99, 1, &manifest);
        assert!(matches!(score.lyrics_status, LyricsLoadState::Failed(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_live_score_uses_only_the_given_catalog() {
        // Same music ID resolved against two independent catalogs (regions):
        // each classification comes solely from its own manifest and disk.
        let dir_a = temp_dir("catalog_a");
        let dir_b = temp_dir("catalog_b");
        let manifest_a = vec![manifest_entry(1, "live/musicscores/m42/m42_lyrics", "ef56")];
        let manifest_b: Vec<ManifestEntry> = Vec::new();
        write_hashed_asset(&dir_a, "ef56", b"garbage");

        let score_a = load_live_score(&dir_a, 42, 1, &manifest_a);
        let score_b = load_live_score(&dir_b, 42, 1, &manifest_b);
        assert!(matches!(score_a.lyrics_status, LyricsLoadState::Failed(_)));
        assert_eq!(score_b.lyrics_status, LyricsLoadState::NoScoreAsset);

        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }
}
