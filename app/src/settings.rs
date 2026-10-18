//! Per-user settings in %APPDATA%\umaLord\settings.json (theme, saved API key).

use std::path::PathBuf;

use serde_json::Value;

pub fn path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("umaLord").join("settings.json"))
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
    p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&p, all.to_string()).is_ok()
}
