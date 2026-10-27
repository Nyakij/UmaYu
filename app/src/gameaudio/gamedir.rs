//! Auto-detects and validates local Uma Musume game installations and persistent directories.

use std::path::{Path, PathBuf};

/// Operating game data region (Global English or Japanese server).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default,
)]
pub enum GameRegion {
    #[default]
    Global,
    Jp,
}

impl GameRegion {
    pub const ALL: [GameRegion; 2] = [GameRegion::Global, GameRegion::Jp];

    /// Settings key string for this region.
    pub fn key(self) -> &'static str {
        match self {
            GameRegion::Global => "global",
            GameRegion::Jp => "jp",
        }
    }

    /// User-facing display label for this region.
    pub fn label(self) -> &'static str {
        match self {
            GameRegion::Global => "Global",
            GameRegion::Jp => "JP",
        }
    }

    /// Parses a region from string key ("global" or "jp").
    pub fn from_key(key: &str) -> Self {
        match key.trim().to_ascii_lowercase().as_str() {
            "jp" => GameRegion::Jp,
            _ => GameRegion::Global,
        }
    }
}

/// Represents an identified Uma Musume game data installation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameInstallation {
    /// Friendly label indicating the installation source (e.g. "Global", "Steam JP", "DMM JP").
    pub label: String,
    /// Absolute path to the persistent game directory.
    pub path: PathBuf,
    /// Operating game region for this installation.
    pub region: GameRegion,
    /// Whether the manifest database `meta` exists.
    pub has_meta: bool,
    /// Whether the master database `master/master.mdb` exists.
    pub has_master: bool,
    /// Whether the hashed asset directory `dat` exists.
    pub has_dat: bool,
}

impl GameInstallation {
    /// Returns true if the installation has all three required elements.
    pub fn is_valid(&self) -> bool {
        self.has_meta && self.has_master && self.has_dat
    }
}

/// Checks an arbitrary directory path for required game files.
pub fn inspect_directory(
    path: &Path,
    label: impl Into<String>,
    region: GameRegion,
) -> GameInstallation {
    let has_meta = path.join("meta").is_file();
    let has_master = path.join("master").join("master.mdb").is_file();
    let has_dat = path.join("dat").is_dir();
    GameInstallation {
        label: label.into(),
        path: path.to_path_buf(),
        region,
        has_meta,
        has_master,
        has_dat,
    }
}

/// Scans standard system locations for installed game directories.
pub fn detect_installations() -> Vec<GameInstallation> {
    let mut results = Vec::new();

    // 1. AppData LocalLow Cygames umamusume (Global / English)
    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
        let locallow = PathBuf::from(local_appdata).join("..").join("LocalLow");
        let global_dir = locallow.join("Cygames").join("umamusume");
        if global_dir.exists() {
            let inst = inspect_directory(&global_dir, "Global (LocalLow)", GameRegion::Global);
            if inst.has_meta || inst.has_master || inst.has_dat {
                results.push(inst);
            }
        }

        // 2. AppData LocalLow Cygames UmamusumePrettyDerby_Jpn (Steam JP)
        let steam_jp_dir = locallow.join("Cygames").join("UmamusumePrettyDerby_Jpn");
        if steam_jp_dir.exists() {
            let inst = inspect_directory(&steam_jp_dir, "Steam JP (LocalLow)", GameRegion::Jp);
            if inst.has_meta || inst.has_master || inst.has_dat {
                results.push(inst);
            }
        }
    }

    // 3. DMM Game Player config (%APPDATA%\dmmgameplayer5\dmmgame.cnf)
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let dmm_conf = PathBuf::from(appdata)
            .join("dmmgameplayer5")
            .join("dmmgame.cnf");
        if dmm_conf.is_file() {
            if let Ok(content) = std::fs::read_to_string(&dmm_conf) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(contents) = json.get("contents").and_then(|c| c.as_array()) {
                        for item in contents {
                            let prod_id = item.get("productId").and_then(|p| p.as_str());
                            if prod_id == Some("umamusume") {
                                if let Some(install_path) = item
                                    .get("detail")
                                    .and_then(|d| d.get("path"))
                                    .and_then(|p| p.as_str())
                                {
                                    let persistent = PathBuf::from(install_path)
                                        .join("umamusume_Data")
                                        .join("Persistent");
                                    if persistent.exists() {
                                        let inst = inspect_directory(
                                            &persistent,
                                            "DMM JP",
                                            GameRegion::Jp,
                                        );
                                        if inst.has_meta || inst.has_master || inst.has_dat {
                                            results.push(inst);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    results
}
