//! WAV export path resolution and directory management for Game Audio.
//!
//! Handles default (`exports/audio`) and customized export directories,
//! subcategories (`files`, `jukebox`, `live`), and opening folders in Windows Explorer.

use std::path::{Path, PathBuf};

/// Subcategories for audio exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportCategory {
    /// Generic cues or individual audio file cues (`exports/audio/files/`).
    Files,
    /// Jukebox tracks (`exports/audio/jukebox/`).
    Jukebox,
    /// Multi-track live stage mixes (`exports/audio/live/`).
    Live,
}

impl ExportCategory {
    /// Returns the subdirectory name for this export category.
    pub fn subdir_name(self) -> &'static str {
        match self {
            ExportCategory::Files => "files",
            ExportCategory::Jukebox => "jukebox",
            ExportCategory::Live => "live",
        }
    }
}

/// Resolves the default base export directory (`<working_dir>/exports/audio`).
pub fn default_export_base_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join("exports")
        .join("audio")
}

/// Resolves the base export directory, using the custom path if specified or falling back to default.
pub fn resolve_export_base_dir(custom: Option<&str>) -> PathBuf {
    match custom.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            let p = PathBuf::from(s);
            if p.is_relative() {
                std::env::current_dir().unwrap_or_default().join(p)
            } else {
                p
            }
        }
        None => default_export_base_dir(),
    }
}

/// Resolves the directory for a specific export category under the base directory.
pub fn resolve_category_dir(base: &Path, category: ExportCategory) -> PathBuf {
    base.join(category.subdir_name())
}

/// Opens the specified folder in Windows Explorer using a dedicated process argument.
pub fn open_folder_in_explorer(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create folder {}: {e}", dir.display()))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(dir.as_os_str())
            .spawn()
            .map_err(|e| format!("Failed to launch explorer.exe: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
        Ok(())
    }
}
