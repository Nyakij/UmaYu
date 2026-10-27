//! Unity Texture2D extraction and decoding for game UI assets (album jackets and icons).
//!
//! Extracts serialized `Texture2D` objects from UnityFS LZ4 containers, decodes common
//! PC and mobile texture compression formats (BC1/DXT1, BC3/DXT5, BC7, ASTC, ETC2, RGBA32),
//! flips pixel orientation for egui, and manages an asynchronous background loader with a bounded cache.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use eframe::egui;

use crate::gameaudio::meta::ManifestEntry;

/// A raw decoded RGBA8 image buffer with width and height.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Flips an RGBA image buffer vertically in-place.
pub fn flip_vertical(rgba: &mut [u8], width: usize, height: usize) {
    if width == 0 || height <= 1 {
        return;
    }
    let row_stride = width * 4;
    let half_h = height / 2;
    for y in 0..half_h {
        let top_idx = y * row_stride;
        let bot_idx = (height - 1 - y) * row_stride;
        if top_idx + row_stride <= rgba.len() && bot_idx + row_stride <= rgba.len() {
            for i in 0..row_stride {
                rgba.swap(top_idx + i, bot_idx + i);
            }
        }
    }
}

/// Converts a slice of packed BGRA `u32` integers into an RGBA8 byte vector.
fn u32_bgra_to_rgba_bytes(pixels: &[u32]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for &p in pixels {
        let b = (p & 0xFF) as u8;
        let g = ((p >> 8) & 0xFF) as u8;
        let r = ((p >> 16) & 0xFF) as u8;
        let a = ((p >> 24) & 0xFF) as u8;
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    rgba
}

/// Decodes raw pixel bytes according to Unity `TextureFormat` integer ID.
pub fn decode_texture_data(
    raw_bytes: &[u8],
    width: u32,
    height: u32,
    format: i32,
) -> Result<DecodedImage, String> {
    if width == 0 || height == 0 {
        return Err("Texture dimensions must be non-zero".to_string());
    }

    let w = width as usize;
    let h = height as usize;
    let pixel_count = w * h;

    let mut rgba = match format {
        // Alpha8
        1 => {
            if raw_bytes.len() < pixel_count {
                return Err("Alpha8 buffer too small".to_string());
            }
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &a in &raw_bytes[..pixel_count] {
                out.extend_from_slice(&[255, 255, 255, a]);
            }
            out
        }
        // ARGB4444
        2 => {
            if raw_bytes.len() < pixel_count * 2 {
                return Err("ARGB4444 buffer too small".to_string());
            }
            let mut out = Vec::with_capacity(pixel_count * 4);
            for i in 0..pixel_count {
                let val = u16::from_le_bytes(raw_bytes[i * 2..i * 2 + 2].try_into().unwrap());
                let a = (((val >> 12) & 0xF) * 17) as u8;
                let r = (((val >> 8) & 0xF) * 17) as u8;
                let g = (((val >> 4) & 0xF) * 17) as u8;
                let b = ((val & 0xF) * 17) as u8;
                out.extend_from_slice(&[r, g, b, a]);
            }
            out
        }
        // RGB24
        3 => {
            if raw_bytes.len() < pixel_count * 3 {
                return Err("RGB24 buffer too small".to_string());
            }
            let mut out = Vec::with_capacity(pixel_count * 4);
            for i in 0..pixel_count {
                let r = raw_bytes[i * 3];
                let g = raw_bytes[i * 3 + 1];
                let b = raw_bytes[i * 3 + 2];
                out.extend_from_slice(&[r, g, b, 255]);
            }
            out
        }
        // RGBA32
        4 => {
            if raw_bytes.len() < pixel_count * 4 {
                return Err("RGBA32 buffer too small".to_string());
            }
            raw_bytes[..pixel_count * 4].to_vec()
        }
        // ARGB32
        5 => {
            if raw_bytes.len() < pixel_count * 4 {
                return Err("ARGB32 buffer too small".to_string());
            }
            let mut out = Vec::with_capacity(pixel_count * 4);
            for i in 0..pixel_count {
                let a = raw_bytes[i * 4];
                let r = raw_bytes[i * 4 + 1];
                let g = raw_bytes[i * 4 + 2];
                let b = raw_bytes[i * 4 + 3];
                out.extend_from_slice(&[r, g, b, a]);
            }
            out
        }
        // RGB565
        7 => {
            if raw_bytes.len() < pixel_count * 2 {
                return Err("RGB565 buffer too small".to_string());
            }
            let mut out = Vec::with_capacity(pixel_count * 4);
            for i in 0..pixel_count {
                let val = u16::from_le_bytes(raw_bytes[i * 2..i * 2 + 2].try_into().unwrap());
                let r = ((((val >> 11) & 0x1F) as u32 * 255) / 31) as u8;
                let g = ((((val >> 5) & 0x3F) as u32 * 255) / 63) as u8;
                let b = (((val & 0x1F) as u32 * 255) / 31) as u8;
                out.extend_from_slice(&[r, g, b, 255]);
            }
            out
        }
        // DXT1 / BC1
        10 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_bc1(raw_bytes, w, h, &mut buf)
                .map_err(|e| format!("BC1 decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        // DXT5 / BC3
        12 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_bc3(raw_bytes, w, h, &mut buf)
                .map_err(|e| format!("BC3 decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        // RGBA4444
        13 => {
            if raw_bytes.len() < pixel_count * 2 {
                return Err("RGBA4444 buffer too small".to_string());
            }
            let mut out = Vec::with_capacity(pixel_count * 4);
            for i in 0..pixel_count {
                let val = u16::from_le_bytes(raw_bytes[i * 2..i * 2 + 2].try_into().unwrap());
                let r = (((val >> 12) & 0xF) * 17) as u8;
                let g = (((val >> 8) & 0xF) * 17) as u8;
                let b = (((val >> 4) & 0xF) * 17) as u8;
                let a = ((val & 0xF) * 17) as u8;
                out.extend_from_slice(&[r, g, b, a]);
            }
            out
        }
        // BGRA32
        14 => {
            if raw_bytes.len() < pixel_count * 4 {
                return Err("BGRA32 buffer too small".to_string());
            }
            let mut out = raw_bytes[..pixel_count * 4].to_vec();
            for chunk in out.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            out
        }
        // BC7
        25 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_bc7(raw_bytes, w, h, &mut buf)
                .map_err(|e| format!("BC7 decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        // ETC2_RGB
        45 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_etc2_rgb(raw_bytes, w, h, &mut buf)
                .map_err(|e| format!("ETC2 RGB decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        // ETC2_RGBA8
        47 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_etc2_rgba8(raw_bytes, w, h, &mut buf)
                .map_err(|e| format!("ETC2 RGBA8 decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        // ASTC_RGB_4x4
        48 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_astc(raw_bytes, w, h, 4, 4, &mut buf)
                .map_err(|e| format!("ASTC 4x4 decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        // ASTC_RGBA_4x4
        54 => {
            let mut buf = vec![0u32; pixel_count];
            texture2ddecoder::decode_astc(raw_bytes, w, h, 4, 4, &mut buf)
                .map_err(|e| format!("ASTC RGBA 4x4 decode error: {e}"))?;
            u32_bgra_to_rgba_bytes(&buf)
        }
        _ => return Err(format!("Unsupported texture format: {format}")),
    };

    flip_vertical(&mut rgba, w, h);

    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

/// Reads a null-terminated string from raw bytes starting at the given cursor.
fn read_null_terminated_string(bytes: &[u8], cursor: &mut usize) -> Result<String, String> {
    let start = *cursor;
    while *cursor < bytes.len() && bytes[*cursor] != 0 {
        *cursor += 1;
    }
    if *cursor >= bytes.len() {
        return Err("Unterminated string in UnityFS header".to_string());
    }
    let s = String::from_utf8_lossy(&bytes[start..*cursor]).into_owned();
    *cursor += 1;
    Ok(s)
}

/// Extracts and decodes a `Texture2D` from a UnityFS asset bundle archive.
pub fn extract_texture_from_bundle(bundle_bytes: &[u8]) -> Result<DecodedImage, String> {
    if bundle_bytes.len() < 16 || &bundle_bytes[0..7] != b"UnityFS" {
        return Err("Not a valid UnityFS bundle header".to_string());
    }

    let mut cursor = 8usize;
    if cursor + 4 > bundle_bytes.len() {
        return Err("Unexpected EOF reading version".to_string());
    }
    let _version = u32::from_be_bytes(bundle_bytes[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;

    let unity_ver = read_null_terminated_string(bundle_bytes, &mut cursor)?;
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

    // Identify companion .resS streaming resource node if any
    let ress_slice: Option<&[u8]> = nodes
        .iter()
        .find(|(_, _, _, path)| path.ends_with(".resS"))
        .and_then(|&(offset, size, _, _)| {
            if offset + size <= uncompressed_data.len() {
                Some(&uncompressed_data[offset..offset + size])
            } else {
                None
            }
        });

    let (major, minor) = parse_unity_version(&unity_ver);

    for &(offset, size, _, ref path) in &nodes {
        if path.ends_with(".resS") || offset + size > uncompressed_data.len() {
            continue;
        }
        let file_bytes = &uncompressed_data[offset..offset + size];
        if let Ok(img) = extract_texture_from_serialized_file(file_bytes, major, minor, ress_slice)
        {
            return Ok(img);
        }
    }

    extract_texture_heuristic(&uncompressed_data)
}

/// Parses the major and minor numbers from a Unity version string like "2020.3.24f1".
fn parse_unity_version(ver_str: &str) -> (u32, u32) {
    let mut parts = ver_str.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(2020);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(3);
    (major, minor)
}

/// Parses a Unity SerializedFile node and extracts the embedded or referenced Texture2D.
fn extract_texture_from_serialized_file(
    file_bytes: &[u8],
    major: u32,
    minor: u32,
    ress_slice: Option<&[u8]>,
) -> Result<DecodedImage, String> {
    if file_bytes.len() < 24 {
        return Err("Serialized file too short".to_string());
    }

    let version = u32::from_be_bytes(file_bytes[8..12].try_into().unwrap());
    let data_offset = if version >= 22 {
        if file_bytes.len() < 28 {
            return Err("Serialized file too short for v22 header".to_string());
        }
        u64::from_be_bytes(file_bytes[16..24].try_into().unwrap()) as usize
    } else {
        u32::from_be_bytes(file_bytes[12..16].try_into().unwrap()) as usize
    };

    if data_offset == 0 || data_offset >= file_bytes.len() {
        return Err("Invalid data offset in SerializedFile".to_string());
    }

    let payload = &file_bytes[data_offset..];
    if payload.len() < 32 {
        return Err("Serialized payload too small".to_string());
    }

    // 1. m_Name
    let name_len = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
    if name_len > 256 || 4 + name_len > payload.len() {
        return Err("Invalid texture name length".to_string());
    }
    let aligned_name = (name_len + 3) & !3;
    let mut cur = 4 + aligned_name;

    // 2. Texture base class fields (m_ForcedFallbackFormat, m_DownscaleFallback, m_IsAlphaChannelOptional)
    if major > 2017 || (major == 2017 && minor >= 3) {
        if cur + 5 > payload.len() {
            return Err("Unexpected EOF in Texture base fields".to_string());
        }
        cur += 4; // m_ForcedFallbackFormat (i32)
        cur += 1; // m_DownscaleFallback (bool)
        if major > 2020 || (major == 2020 && minor >= 2) {
            cur += 1; // m_IsAlphaChannelOptional (bool)
        }
        cur = (cur + 3) & !3;
    }

    if cur + 16 > payload.len() {
        return Err("Unexpected EOF reading Texture2D dimensions".to_string());
    }

    let width = u32::from_le_bytes(payload[cur..cur + 4].try_into().unwrap());
    cur += 4;
    let height = u32::from_le_bytes(payload[cur..cur + 4].try_into().unwrap());
    cur += 4;
    let complete_image_size =
        u32::from_le_bytes(payload[cur..cur + 4].try_into().unwrap()) as usize;
    cur += 4;

    if major >= 2020 {
        if cur + 4 > payload.len() {
            return Err("Unexpected EOF reading m_MipsStripped".to_string());
        }
        cur += 4; // m_MipsStripped (i32)
    }

    if cur + 4 > payload.len() {
        return Err("Unexpected EOF reading texture format".to_string());
    }
    let texture_format = i32::from_le_bytes(payload[cur..cur + 4].try_into().unwrap());
    cur += 4;

    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("Unreasonable texture dimensions".to_string());
    }

    // 3. Scan forward for image_data_size matching complete_image_size or non-empty payload
    let mut scan_pos = cur;
    let limit = (cur + 256).min(payload.len().saturating_sub(4));
    let mut found_data: Option<&[u8]> = None;

    while scan_pos <= limit {
        let size_candidate =
            u32::from_le_bytes(payload[scan_pos..scan_pos + 4].try_into().unwrap()) as usize;
        if size_candidate > 0
            && (size_candidate == complete_image_size
                || size_candidate >= (width * height / 2) as usize)
            && scan_pos + 4 + size_candidate <= payload.len()
        {
            found_data = Some(&payload[scan_pos + 4..scan_pos + 4 + size_candidate]);
            break;
        }
        scan_pos += 4;
    }

    // 4. Fallback to companion .resS streaming buffer if image_data was 0 / external
    if found_data.is_none() {
        if let Some(ress) = ress_slice {
            if complete_image_size > 0 && complete_image_size <= ress.len() {
                found_data = Some(&ress[..complete_image_size]);
            } else if !ress.is_empty() {
                found_data = Some(ress);
            }
        }
    }

    let raw_pixel_data =
        found_data.ok_or_else(|| "Could not locate pixel data block".to_string())?;

    decode_texture_data(raw_pixel_data, width, height, texture_format)
}

/// Fallback heuristic scanner for textures when structural parsing encounters non-standard layout.
fn extract_texture_heuristic(data: &[u8]) -> Result<DecodedImage, String> {
    let mut i = 0;
    while i + 32 < data.len() {
        let w = u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
        let h = u32::from_le_bytes(data[i + 4..i + 8].try_into().unwrap());

        if (w == 64 || w == 128 || w == 256 || w == 512 || w == 1024)
            && (h == 64 || h == 128 || h == 256 || h == 512 || h == 1024)
        {
            let comp_sz = u32::from_le_bytes(data[i + 8..i + 12].try_into().unwrap()) as usize;
            for offset in &[12, 16] {
                if i + offset + 4 <= data.len() {
                    let fmt =
                        i32::from_le_bytes(data[i + offset..i + offset + 4].try_into().unwrap());
                    if (1..=60).contains(&fmt) {
                        let mut check = i + offset + 4;
                        while check + 4 <= data.len() && check <= i + 128 {
                            let sz = u32::from_le_bytes(data[check..check + 4].try_into().unwrap())
                                as usize;
                            if sz > 0
                                && (sz == comp_sz
                                    || sz == (w * h) as usize
                                    || sz == (w * h / 2) as usize)
                                && check + 4 + sz <= data.len()
                            {
                                let pixel_slice = &data[check + 4..check + 4 + sz];
                                if let Ok(img) = decode_texture_data(pixel_slice, w, h, fmt) {
                                    return Ok(img);
                                }
                            }
                            check += 4;
                        }
                    }
                }
            }
        }
        i += 4;
    }

    Err("Texture2D pattern not found in decompressed bundle".to_string())
}

/// Finds the most relevant album jacket asset entry in the game manifest for a given music ID.
///
/// Priority order:
/// 1. `jacket_icon_l_{id:04}`
/// 2. `jacket_icon_l_{id}`
/// 3. `jacket_icon_m_{id:04}`
/// 4. `jacket_icon_m_{id}`
/// 5. `jacket_icon_s_{id:04}`
/// 6. `jacket_icon_s_{id}`
/// 7. Generic fallback: `jacket_icon_l_0000`, `jacket_icon_m_0000`, `jacket_icon_0000`
pub fn find_jacket_entry<'a>(
    music_id: i32,
    manifest: &'a [ManifestEntry],
) -> Option<&'a ManifestEntry> {
    let id_4 = format!("{music_id:04}");
    let id_str = music_id.to_string();

    let candidates = [
        format!("jacket_icon_l_{id_4}"),
        format!("jacket_icon_l_{id_str}"),
        format!("jacket_icon_m_{id_4}"),
        format!("jacket_icon_m_{id_str}"),
        format!("jacket_icon_s_{id_4}"),
        format!("jacket_icon_s_{id_str}"),
        "jacket_icon_l_0000".to_string(),
        "jacket_icon_m_0000".to_string(),
        "jacket_icon_0000".to_string(),
    ];

    for candidate in &candidates {
        if let Some(entry) = manifest.iter().find(|e| {
            let n = e.name.to_ascii_lowercase();
            n.ends_with(candidate) || n.contains(candidate.as_str())
        }) {
            return Some(entry);
        }
    }

    None
}

/// Discriminator key for caching textures, keeping album jackets and character avatars isolated.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextureKey {
    Jacket { game_path: String, music_id: i32 },
    Avatar { game_path: String, chara_id: i32 },
}

impl TextureKey {
    /// Produces a unique identifier for the egui texture registry.
    pub fn texture_name(&self) -> String {
        match self {
            Self::Jacket {
                game_path,
                music_id,
            } => format!("tex_jkt_{game_path}_{music_id}"),
            Self::Avatar {
                game_path,
                chara_id,
            } => format!("tex_ava_{game_path}_{chara_id}"),
        }
    }
}

/// Request sent to the background texture decoding worker thread.
struct TextureLoadRequest {
    key: TextureKey,
    file_path: PathBuf,
    generation: u64,
}

/// Response returned from the background texture decoding worker thread.
struct TextureLoadResponse {
    key: TextureKey,
    result: Result<DecodedImage, String>,
    generation: u64,
}

/// Current state of a cached texture entry in the UI.
pub enum TextureSlot {
    Loading,
    Ready(egui::TextureHandle),
    Failed,
}

/// Manages asynchronous loading, decoding, and LRU bounded caching for jacket/icon textures.
pub struct TextureManager {
    cache: HashMap<TextureKey, TextureSlot>,
    lru_order: VecDeque<TextureKey>,
    in_flight: HashSet<TextureKey>,
    tx_req: Sender<TextureLoadRequest>,
    rx_resp: Receiver<TextureLoadResponse>,
    max_capacity: usize,
    generation: u64,
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new(128)
    }
}

impl TextureManager {
    /// Creates a new TextureManager with the specified maximum cached texture capacity.
    pub fn new(max_capacity: usize) -> Self {
        let (tx_req, rx_req) = channel::<TextureLoadRequest>();
        let (tx_resp, rx_resp) = channel::<TextureLoadResponse>();

        thread::spawn(move || {
            while let Ok(req) = rx_req.recv() {
                let result = match File::open(&req.file_path) {
                    Ok(mut f) => {
                        let mut bytes = Vec::new();
                        if f.read_to_end(&mut bytes).is_ok() {
                            let is_webp = req
                                .file_path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .map(|s| s.eq_ignore_ascii_case("webp"))
                                .unwrap_or(false);

                            if is_webp {
                                match image::load_from_memory_with_format(
                                    &bytes,
                                    image::ImageFormat::WebP,
                                ) {
                                    Ok(dyn_img) => {
                                        let rgba = dyn_img.to_rgba8();
                                        let (w, h) = rgba.dimensions();
                                        Ok(DecodedImage {
                                            width: w,
                                            height: h,
                                            rgba: rgba.into_raw(),
                                        })
                                    }
                                    Err(e) => Err(format!("WebP decode failed: {e}")),
                                }
                            } else {
                                extract_texture_from_bundle(&bytes)
                            }
                        } else {
                            Err("Failed to read texture asset file".to_string())
                        }
                    }
                    Err(e) => Err(format!("Could not open texture file: {e}")),
                };

                let _ = tx_resp.send(TextureLoadResponse {
                    key: req.key,
                    result,
                    generation: req.generation,
                });
            }
        });

        Self {
            cache: HashMap::new(),
            lru_order: VecDeque::new(),
            in_flight: HashSet::new(),
            tx_req,
            rx_resp,
            max_capacity,
            generation: 0,
        }
    }

    /// Polls completed background texture decodes and uploads them to GPU texture handles.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut loaded_any = false;
        while let Ok(resp) = self.rx_resp.try_recv() {
            if resp.generation != self.generation {
                continue;
            }
            self.in_flight.remove(&resp.key);

            match resp.result {
                Ok(img) => {
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(
                        [img.width as usize, img.height as usize],
                        &img.rgba,
                    );
                    let handle = ctx.load_texture(
                        resp.key.texture_name(),
                        color_img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.insert_ready(resp.key, handle);
                    loaded_any = true;
                }
                Err(_) => {
                    self.cache.insert(resp.key, TextureSlot::Failed);
                }
            }
        }

        if loaded_any {
            ctx.request_repaint();
        } else if !self.in_flight.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// Inserts a ready texture handle into the bounded LRU cache.
    fn insert_ready(&mut self, key: TextureKey, handle: egui::TextureHandle) {
        if self.cache.len() >= self.max_capacity {
            while let Some(old_key) = self.lru_order.pop_front() {
                if self.cache.remove(&old_key).is_some() {
                    break;
                }
            }
        }
        self.lru_order.push_back(key.clone());
        self.cache.insert(key, TextureSlot::Ready(handle));
    }

    /// Retrieves an already-loaded texture handle, or queues a non-blocking background load.
    pub fn get_or_request_jacket(
        &mut self,
        game_path: &str,
        music_id: i32,
        manifest: &[ManifestEntry],
    ) -> Option<&egui::TextureHandle> {
        let key = TextureKey::Jacket {
            game_path: game_path.to_string(),
            music_id,
        };

        if self.cache.contains_key(&key) {
            return match self.cache.get(&key) {
                Some(TextureSlot::Ready(handle)) => Some(handle),
                _ => None,
            };
        }

        if self.in_flight.contains(&key) {
            return None;
        }

        // Locate jacket file in manifest
        if let Some(entry) = find_jacket_entry(music_id, manifest) {
            let path = entry.file_path(Path::new(game_path));
            if path.is_file() {
                self.in_flight.insert(key.clone());
                self.cache.insert(key.clone(), TextureSlot::Loading);
                let _ = self.tx_req.send(TextureLoadRequest {
                    key,
                    file_path: path,
                    generation: self.generation,
                });
                return None;
            }
        }

        self.cache.insert(key, TextureSlot::Failed);
        None
    }

    /// Retrieves an already-loaded character avatar handle, or queues an asynchronous background load.
    ///
    /// Priority order:
    /// 1. Game installation asset bundle matching `chr_icon_####` that exists on disk
    /// 2. Packaged asset fallback in `app/assets/umas/`
    /// 3. Returns `None` (triggers UI placeholder rendering)
    pub fn get_or_request_avatar(
        &mut self,
        game_path: &str,
        chara_id: i32,
        manifest: &[ManifestEntry],
        packaged_root: Option<&Path>,
    ) -> Option<&egui::TextureHandle> {
        if chara_id <= 0 {
            return None;
        }

        let key = TextureKey::Avatar {
            game_path: game_path.to_string(),
            chara_id,
        };

        if self.cache.contains_key(&key) {
            return match self.cache.get(&key) {
                Some(TextureSlot::Ready(handle)) => Some(handle),
                _ => None,
            };
        }

        if self.in_flight.contains(&key) {
            return None;
        }

        // 1. Check if game asset exists in manifest and physically on disk
        if !game_path.is_empty() {
            let game_dir = Path::new(game_path);
            if let Some(entry) = find_chara_icon_entry_on_disk(chara_id, manifest, game_dir) {
                let path = entry.file_path(game_dir);
                if path.is_file() {
                    self.in_flight.insert(key.clone());
                    self.cache.insert(key.clone(), TextureSlot::Loading);
                    let _ = self.tx_req.send(TextureLoadRequest {
                        key,
                        file_path: path,
                        generation: self.generation,
                    });
                    return None;
                }
            }
        }

        // 2. Fallback to packaged webp assets in app/assets/umas/
        if let Some(fallback_path) = find_packaged_uma_art(chara_id, packaged_root) {
            if fallback_path.is_file() {
                self.in_flight.insert(key.clone());
                self.cache.insert(key.clone(), TextureSlot::Loading);
                let _ = self.tx_req.send(TextureLoadRequest {
                    key,
                    file_path: fallback_path,
                    generation: self.generation,
                });
                return None;
            }
        }

        // 3. Neither game asset nor packaged art found
        self.cache.insert(key, TextureSlot::Failed);
        None
    }

    /// Clears cached textures (useful when switching game directories or active regions).
    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.cache.clear();
        self.lru_order.clear();
        self.in_flight.clear();
    }
}

/// Checks if an entry name matches an icon candidate without picking unwanted variants
/// (such as trained icons, mob icons for normal umas, plushies, push pins, or skills).
fn is_valid_chara_icon_entry(entry_name: &str, candidate: &str, chara_id: i32) -> bool {
    let n = entry_name.to_ascii_lowercase();
    if n.contains("trained_")
        || n.contains("generate_")
        || n.contains("plushie_")
        || n.contains("push_")
        || n.contains("course_")
        || n.contains("skill")
    {
        return false;
    }
    if chara_id < 8000 && (n.contains("mob/") || n.contains("mob_")) {
        return false;
    }
    n.match_indices(candidate).any(|(pos, _)| {
        let end = pos + candidate.len();
        let starts_component = pos == 0 || n.as_bytes()[pos - 1] == b'/';
        let ends_component = end == n.len() || matches!(n.as_bytes()[end], b'/' | b'.');
        starts_component && ends_component
    })
}

/// Returns ordered search candidate patterns for locating a character icon.
pub fn chara_icon_candidate_patterns(chara_id: i32) -> Vec<String> {
    let id_4 = format!("{chara_id:04}");
    let id_str = chara_id.to_string();

    vec![
        format!("chr_icon_{id_4}"),
        format!("chr_icon_{id_4}_{id_4}01_01"),
        format!("chr_icon_{id_4}_{id_4}01"),
        format!("chr_icon_{id_4}_{id_4}02_01"),
        format!("chr_icon_{id_4}_{id_4}02"),
        format!("chr_icon_{id_4}_000001_01"),
        format!("chr_icon_{id_4}_000101_01"),
        format!("chr_icon_{id_4}01"),
        format!("chr_icon_{id_4}_00"),
        format!("chr_icon_{id_4}_01"),
        format!("chr_icon_{id_4}_"),
        format!("chr_icon_{id_str}"),
        format!("chr_icon_{id_str}_{id_str}01_01"),
        format!("chr_icon_{id_str}01"),
        format!("chr_icon_{id_str}_"),
    ]
}

/// Finds the most relevant character icon asset entry in the game manifest for a given chara ID.
pub fn find_chara_icon_entry<'a>(
    chara_id: i32,
    manifest: &'a [ManifestEntry],
) -> Option<&'a ManifestEntry> {
    if chara_id <= 0 {
        return None;
    }

    let candidates = chara_icon_candidate_patterns(chara_id);
    for candidate in &candidates {
        if let Some(entry) = manifest
            .iter()
            .find(|e| is_valid_chara_icon_entry(&e.name, candidate, chara_id))
        {
            return Some(entry);
        }
    }

    None
}

/// Finds the highest-priority character icon asset entry that physically exists on disk.
/// Falls back to manifest-only lookup if none are verified on disk.
pub fn find_chara_icon_entry_on_disk<'a>(
    chara_id: i32,
    manifest: &'a [ManifestEntry],
    game_dir: &Path,
) -> Option<&'a ManifestEntry> {
    if chara_id <= 0 {
        return None;
    }

    let candidates = chara_icon_candidate_patterns(chara_id);
    for candidate in &candidates {
        for entry in manifest
            .iter()
            .filter(|e| is_valid_chara_icon_entry(&e.name, candidate, chara_id))
        {
            if entry.exists_on_disk(game_dir) {
                return Some(entry);
            }
        }
    }

    find_chara_icon_entry(chara_id, manifest)
}

/// Locates a packaged character art image in `app/assets/umas/` for fallback display.
pub fn find_packaged_uma_art(chara_id: i32, root: Option<&Path>) -> Option<PathBuf> {
    if chara_id <= 0 {
        return None;
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let base_dirs = [
        root.map(|r| r.join("app").join("assets").join("umas")),
        root.map(|r| r.join("assets").join("umas")),
        crate::data::find_root(None).map(|r| r.join("app").join("assets").join("umas")),
        crate::data::find_root(None).map(|r| r.join("assets").join("umas")),
        exe_dir
            .as_ref()
            .map(|d| d.join("app").join("assets").join("umas")),
        exe_dir.as_ref().map(|d| d.join("assets").join("umas")),
        Some(PathBuf::from("app/assets/umas")),
        Some(PathBuf::from("assets/umas")),
    ];

    let candidates = [
        format!("{chara_id:04}01.webp"),
        format!("{chara_id}01.webp"),
        format!("{chara_id:04}02.webp"),
        format!("{chara_id}02.webp"),
        format!("{chara_id:04}03.webp"),
        format!("{chara_id}03.webp"),
    ];

    for base_opt in &base_dirs {
        if let Some(base) = base_opt {
            if base.is_dir() {
                for cand in &candidates {
                    let path = base.join(cand);
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_manifest_entry(name: &str, hash: &str) -> ManifestEntry {
        ManifestEntry {
            id: 1,
            name: name.to_string(),
            dependencies: None,
            group: 1,
            length: 1024,
            checksum: 12345,
            hash_name: hash.to_string(),
            kind: 1,
            state: 1,
            priority: 0,
        }
    }

    #[test]
    fn test_chara_icon_filtering_excludes_unwanted() {
        assert!(!is_valid_chara_icon_entry(
            "chara/chr1001/trained_chr_icon_1001",
            "chr_icon_1001",
            1001
        ));
        assert!(!is_valid_chara_icon_entry(
            "chara/chr1001/generate_chr_icon_1001",
            "chr_icon_1001",
            1001
        ));
        assert!(!is_valid_chara_icon_entry(
            "mob/mob_chr_icon_1001",
            "chr_icon_1001",
            1001
        ));
        assert!(!is_valid_chara_icon_entry(
            "outgame/skillicon/utx_ico_skill_1001",
            "chr_icon_1001",
            1001
        ));
        assert!(is_valid_chara_icon_entry(
            "chara/chr1001/chr_icon_1001",
            "chr_icon_1001",
            1001
        ));
        assert!(is_valid_chara_icon_entry(
            "chara/chr1001/chr_icon_1001_100101_01",
            "chr_icon_1001_100101_01",
            1001
        ));
    }

    #[test]
    fn test_chara_icon_candidate_priority() {
        let manifest = vec![
            make_test_manifest_entry("chara/chr1001/chr_icon_1001_100101_01", "HASH_COSTUME"),
            make_test_manifest_entry("chara/chr1001/chr_icon_1001", "HASH_BASE"),
        ];

        let found = find_chara_icon_entry(1001, &manifest);
        assert!(found.is_some());
        // Base icon has higher priority than costume icon
        assert_eq!(found.unwrap().hash_name, "HASH_BASE");
    }

    #[test]
    fn test_texture_key_isolation() {
        let key_global = TextureKey::Avatar {
            game_path: "C:\\Game\\Global".to_string(),
            chara_id: 1001,
        };
        let key_jp = TextureKey::Avatar {
            game_path: "D:\\Game\\JP".to_string(),
            chara_id: 1001,
        };
        let key_other = TextureKey::Avatar {
            game_path: "C:\\Game\\Global".to_string(),
            chara_id: 1002,
        };

        assert_ne!(key_global, key_jp);
        assert_ne!(key_global, key_other);
        assert_ne!(key_global.texture_name(), key_jp.texture_name());
    }

    #[test]
    fn test_packaged_uma_art_invalid_id() {
        assert!(find_packaged_uma_art(0, None).is_none());
        assert!(find_packaged_uma_art(-1, None).is_none());
    }
}
