//! WAV file exporter for decoded audio streams.
//!
//! Writes standard 16-bit PCM RIFF WAVE format without external dependencies.

use std::fs::File;
use std::io::{BufWriter, Result as IoResult, Write};
use std::path::Path;

/// Writes interleaved 32-bit float audio samples as a 16-bit PCM WAV file to a writer.
#[allow(dead_code)]
pub fn write_wav_pcm16<W: Write>(
    writer: &mut W,
    channels: u16,
    sample_rate: u32,
    samples: &[f32],
) -> IoResult<()> {
    let num_samples = samples.len();
    let bytes_per_sample = 2u32;
    let data_len = (num_samples as u32) * bytes_per_sample;
    let riff_len = 36u32.saturating_add(data_len);
    let byte_rate = sample_rate * (channels as u32) * bytes_per_sample;
    let block_align = channels * (bytes_per_sample as u16);

    // RIFF chunk descriptor
    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_len.to_le_bytes())?;
    writer.write_all(b"WAVE")?;

    // "fmt " sub-chunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    writer.write_all(&1u16.to_le_bytes())?; // AudioFormat (1 = PCM)
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&16u16.to_le_bytes())?; // BitsPerSample

    // "data" sub-chunk
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;

    // Convert f32 samples to 16-bit signed PCM
    let mut chunk = [0u8; 4096];
    let mut chunk_idx = 0;

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0).round() as i16;
        let bytes = sample_i16.to_le_bytes();

        chunk[chunk_idx] = bytes[0];
        chunk[chunk_idx + 1] = bytes[1];
        chunk_idx += 2;

        if chunk_idx >= chunk.len() {
            writer.write_all(&chunk[..chunk_idx])?;
            chunk_idx = 0;
        }
    }

    if chunk_idx > 0 {
        writer.write_all(&chunk[..chunk_idx])?;
    }

    writer.flush()
}

/// Saves audio samples directly to a WAV file on disk.
#[allow(dead_code)]
pub fn save_wav_file(
    path: &Path,
    channels: u16,
    sample_rate: u32,
    samples: &[f32],
) -> IoResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_wav_pcm16(&mut writer, channels, sample_rate, samples)
}
