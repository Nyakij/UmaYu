//! Per-user settings in %APPDATA%\UmaYu\settings.json (theme, saved API key).

use std::path::PathBuf;
use std::sync::Once;

use serde_json::Value;

static MIGRATE: Once = Once::new();

/// If the new `%APPDATA%\UmaYu` directory doesn't exist yet but the old `%APPDATA%\umaLord`
/// exists, rename it so settings, cache, and profiles are preserved.
pub fn migrate_appdata() {
    MIGRATE.call_once(|| {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let base = PathBuf::from(appdata);
            let old_dir = base.join("umaLord");
            let new_dir = base.join("UmaYu");
            if old_dir.exists() && !new_dir.exists() {
                let _ = std::fs::rename(old_dir, new_dir);
            }
        }
    });
}

pub fn path() -> Option<PathBuf> {
    migrate_appdata();
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("UmaYu").join("settings.json"))
}

fn read_all() -> Value {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Default::default()))
}

pub fn get(key: &str) -> Option<Value> {
    read_all().get(key).cloned().filter(|v| !v.is_null())
}

/// Set one key, keeping the others. `Value::Null` removes it.
pub fn set(key: &str, value: Value) -> bool {
    let Some(p) = path() else { return false };
    let mut all = read_all();
    if let Some(map) = all.as_object_mut() {
        if value.is_null() {
            map.remove(key);
        } else {
            map.insert(key.to_string(), value);
        }
    }
    p.parent()
        .is_some_and(|d| std::fs::create_dir_all(d).is_ok())
        && std::fs::write(&p, all.to_string()).is_ok()
}
