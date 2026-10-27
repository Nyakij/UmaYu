//! Reads and parses the game asset manifest database (`meta`).
//!
//! Handles both unencrypted plain SQLite manifests and encrypted manifests (ChaCha20 via SQLite3MultipleCiphers).

use std::path::{Path, PathBuf};

/// Known encryption keys used by different versions/regions of Uma Musume.
pub const KNOWN_KEYS: &[&str] = &[
    // Global / English key
    "c753a5e8f5f78294f7fef57df4a14ffbf9a896cea1d4e09947e0d904e7fde8eaf0",
    // JP DMM key
    "9c2bab97bcf8c0c4f1a9ea7881a213f6c9ebf9d8d4c6a8e43ce5a259bde7e9fd",
    // Fallback key
    "8908b6dca7b6db99dda0",
];

/// A single file entry in the game's asset manifest table `a`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    /// Asset ID (`i`).
    pub id: i64,
    /// Logical asset name / path in game virtual filesystem (`n`).
    pub name: String,
    /// Dependencies list (`d`).
    pub dependencies: Option<String>,
    /// Group flags (`g`).
    pub group: i32,
    /// Asset length in bytes (`l`).
    pub length: i64,
    /// Checksum (`c`).
    pub checksum: i64,
    /// Hash filename on disk (`h`), located at `dat/{h[0..2]}/{h}`.
    pub hash_name: String,
    /// Asset kind (`k`).
    pub kind: i32,
    /// Download state (`s`): 1 = downloaded, 0 = not downloaded.
    pub state: i32,
    /// Priority (`p`).
    pub priority: i32,
}

impl ManifestEntry {
    /// Resolves the absolute path to this asset in the game's persistent directory.
    #[allow(dead_code)]
    pub fn file_path(&self, game_dir: &Path) -> PathBuf {
        resolve_path(game_dir, &self.hash_name)
    }

    /// Checks if the physical file for this asset exists on disk.
    #[allow(dead_code)]
    pub fn exists_on_disk(&self, game_dir: &Path) -> bool {
        self.file_path(game_dir).is_file()
    }
}

/// Resolves a hashed asset name into its storage path under `dat/{h[0..2]}/{h}`.
#[allow(dead_code)]
pub fn resolve_path(game_dir: &Path, hash_name: &str) -> PathBuf {
    if hash_name.len() >= 2 {
        game_dir.join("dat").join(&hash_name[0..2]).join(hash_name)
    } else {
        game_dir.join("dat").join(hash_name)
    }
}

/// Checks whether a `meta` file begins with the plain SQLite header (`SQLi` / `SQLite format 3`).
pub fn is_meta_plain(meta_path: &Path) -> bool {
    let mut header = [0u8; 16];
    if let Ok(mut file) = std::fs::File::open(meta_path) {
        use std::io::Read;
        if file.read_exact(&mut header).is_ok() {
            return &header[0..4] == b"SQLi";
        }
    }
    false
}

/// Decodes a hex string into raw bytes.
fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

#[cfg(windows)]
mod mc_ffi {
    use std::ffi::{c_char, c_int, c_longlong, c_void};
    use std::path::{Path, PathBuf};

    type Hmodule = *mut c_void;
    type Farproc = *mut c_void;

    unsafe extern "system" {
        fn LoadLibraryW(lp_file_name: *const u16) -> Hmodule;
        fn FreeLibrary(h_module: Hmodule) -> c_int;
        fn GetProcAddress(h_module: Hmodule, lp_proc_name: *const c_char) -> Farproc;
    }

    type Sqlite3OpenV2 = unsafe extern "C" fn(
        filename: *const c_char,
        pp_db: *mut *mut c_void,
        flags: c_int,
        z_vfs: *const c_char,
    ) -> c_int;
    type Sqlite3Exec = unsafe extern "C" fn(
        db: *mut c_void,
        sql: *const c_char,
        callback: *mut c_void,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    type Sqlite3Key =
        unsafe extern "C" fn(db: *mut c_void, p_key: *const c_void, n_key: c_int) -> c_int;
    type Sqlite3PrepareV2 = unsafe extern "C" fn(
        db: *mut c_void,
        z_sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut c_void,
        pz_tail: *mut *const c_char,
    ) -> c_int;
    type Sqlite3Step = unsafe extern "C" fn(p_stmt: *mut c_void) -> c_int;
    type Sqlite3ColumnInt = unsafe extern "C" fn(p_stmt: *mut c_void, i_col: c_int) -> c_int;
    type Sqlite3ColumnInt64 = unsafe extern "C" fn(p_stmt: *mut c_void, i_col: c_int) -> c_longlong;
    type Sqlite3ColumnText =
        unsafe extern "C" fn(p_stmt: *mut c_void, i_col: c_int) -> *const c_char;
    type Sqlite3Finalize = unsafe extern "C" fn(p_stmt: *mut c_void) -> c_int;
    type Sqlite3Close = unsafe extern "C" fn(db: *mut c_void) -> c_int;
    type Sqlite3Errmsg = unsafe extern "C" fn(db: *mut c_void) -> *const c_char;

    pub struct Sqlite3McLib {
        handle: Hmodule,
        pub open_v2: Sqlite3OpenV2,
        pub exec: Sqlite3Exec,
        pub key: Sqlite3Key,
        pub prepare_v2: Sqlite3PrepareV2,
        pub step: Sqlite3Step,
        pub col_int: Sqlite3ColumnInt,
        pub col_int64: Sqlite3ColumnInt64,
        pub col_text: Sqlite3ColumnText,
        pub finalize: Sqlite3Finalize,
        pub close: Sqlite3Close,
        pub _errmsg: Sqlite3Errmsg,
    }

    impl Drop for Sqlite3McLib {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { FreeLibrary(self.handle) };
            }
        }
    }

    /// Finds `sqlite3mc.dll` in known locations.
    pub fn find_sqlite3mc_dll() -> Option<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Next to current executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("sqlite3mc.dll"));
            }
        }

        // 2. Current working directory
        candidates.push(PathBuf::from("sqlite3mc.dll"));
        candidates.push(PathBuf::from("app").join("sqlite3mc.dll"));

        // 3. User profile nuget cache
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            let nuget_base = PathBuf::from(user_profile)
                .join(".nuget")
                .join("packages")
                .join("sqlite3mc.pclraw.lib");
            if nuget_base.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&nuget_base) {
                    for entry in entries.flatten() {
                        let dll = entry
                            .path()
                            .join("runtimes")
                            .join("win-x64")
                            .join("native")
                            .join("sqlite3mc.dll");
                        if dll.is_file() {
                            candidates.push(dll);
                        }
                    }
                }
            }
        }

        // 4. AppData UmaYu folder
        if let Some(appdata) = std::env::var_os("APPDATA") {
            candidates.push(PathBuf::from(appdata).join("UmaYu").join("sqlite3mc.dll"));
        }

        candidates.into_iter().find(|p| p.is_file())
    }

    impl Sqlite3McLib {
        pub fn load(path: &Path) -> Result<Self, String> {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return Err(format!("Failed to load {}", path.display()));
            }

            unsafe fn get_sym<T>(handle: Hmodule, name: &[u8]) -> Result<T, String> {
                let proc = unsafe { GetProcAddress(handle, name.as_ptr() as *const c_char) };
                if proc.is_null() {
                    Err(format!("Missing symbol {}", String::from_utf8_lossy(name)))
                } else {
                    Ok(unsafe { std::mem::transmute_copy(&proc) })
                }
            }

            unsafe {
                let open_v2 = get_sym(handle, b"sqlite3_open_v2\0")?;
                let exec = get_sym(handle, b"sqlite3_exec\0")?;
                let key = get_sym(handle, b"sqlite3_key\0")?;
                let prepare_v2 = get_sym(handle, b"sqlite3_prepare_v2\0")?;
                let step = get_sym(handle, b"sqlite3_step\0")?;
                let col_int = get_sym(handle, b"sqlite3_column_int\0")?;
                let col_int64 = get_sym(handle, b"sqlite3_column_int64\0")?;
                let col_text = get_sym(handle, b"sqlite3_column_text\0")?;
                let finalize = get_sym(handle, b"sqlite3_finalize\0")?;
                let close = get_sym(handle, b"sqlite3_close\0")?;
                let errmsg = get_sym(handle, b"sqlite3_errmsg\0")?;

                Ok(Self {
                    handle,
                    open_v2,
                    exec,
                    key,
                    prepare_v2,
                    step,
                    col_int,
                    col_int64,
                    col_text,
                    finalize,
                    close,
                    _errmsg: errmsg,
                })
            }
        }
    }

    use std::os::windows::ffi::OsStrExt;
}

/// Opens an unencrypted `meta` database and extracts manifest entries.
fn load_plain_manifest(
    meta_path: &Path,
    filter_sql: Option<&str>,
) -> Result<Vec<ManifestEntry>, String> {
    let conn = rusqlite::Connection::open_with_flags(
        meta_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("Failed to open plain meta: {e}"))?;

    let sql = match filter_sql {
        Some(f) => format!("SELECT i, n, d, g, l, c, h, k, s, p FROM a WHERE {f}"),
        None => "SELECT i, n, d, g, l, c, h, k, s, p FROM a".to_string(),
    };

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare query on plain meta: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ManifestEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                dependencies: row.get(2)?,
                group: row.get(3)?,
                length: row.get(4)?,
                checksum: row.get(5)?,
                hash_name: row.get(6)?,
                kind: row.get(7)?,
                state: row.get(8)?,
                priority: row.get(9)?,
            })
        })
        .map_err(|e| format!("Query error on plain meta: {e}"))?;

    let mut result = Vec::new();
    for r in rows {
        if let Ok(entry) = r {
            result.push(entry);
        }
    }
    Ok(result)
}

/// Opens an encrypted `meta` database using SQLite3MultipleCiphers and extracts entries.
#[cfg(windows)]
fn load_encrypted_manifest(
    meta_path: &Path,
    custom_key: Option<&str>,
    filter_sql: Option<&str>,
) -> Result<Vec<ManifestEntry>, String> {
    use std::ffi::{CStr, CString};

    let dll_path = mc_ffi::find_sqlite3mc_dll().ok_or_else(|| {
        "sqlite3mc.dll not found. Please place sqlite3mc.dll next to umayu.exe or in %APPDATA%\\UmaYu\\."
            .to_string()
    })?;

    let mc = mc_ffi::Sqlite3McLib::load(&dll_path)?;

    // Build candidate keys list: custom key first if provided, then known keys
    let mut key_candidates: Vec<Vec<u8>> = Vec::new();
    if let Some(custom) = custom_key {
        if let Some(bytes) = decode_hex(custom) {
            key_candidates.push(bytes);
        }
    }
    for k in KNOWN_KEYS {
        if let Some(bytes) = decode_hex(k) {
            if !key_candidates.contains(&bytes) {
                key_candidates.push(bytes);
            }
        }
    }

    let meta_c = CString::new(meta_path.to_string_lossy().as_bytes())
        .map_err(|e| format!("Invalid path: {e}"))?;
    let pragma_cipher = CString::new("PRAGMA cipher = 'chacha20';").unwrap();

    let query_sql = match filter_sql {
        Some(f) => format!("SELECT i, n, d, g, l, c, h, k, s, p FROM a WHERE {f};"),
        None => "SELECT i, n, d, g, l, c, h, k, s, p FROM a;".to_string(),
    };
    let query_c = CString::new(query_sql).map_err(|e| format!("Invalid query: {e}"))?;

    for key_bytes in &key_candidates {
        let mut db: *mut std::ffi::c_void = std::ptr::null_mut();
        // SQLITE_OPEN_READONLY = 0x00000001
        let rc = unsafe { (mc.open_v2)(meta_c.as_ptr(), &mut db, 1, std::ptr::null()) };
        if rc != 0 || db.is_null() {
            continue;
        }

        // Apply chacha20 cipher and key
        unsafe {
            (mc.exec)(
                db,
                pragma_cipher.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (mc.key)(
                db,
                key_bytes.as_ptr() as *const std::ffi::c_void,
                key_bytes.len() as std::ffi::c_int,
            );
        }

        // Test if decryption succeeds by preparing the query
        let mut stmt: *mut std::ffi::c_void = std::ptr::null_mut();
        let rc =
            unsafe { (mc.prepare_v2)(db, query_c.as_ptr(), -1, &mut stmt, std::ptr::null_mut()) };

        if rc == 0 && !stmt.is_null() {
            let mut entries = Vec::new();
            // SQLITE_ROW = 100
            while unsafe { (mc.step)(stmt) } == 100 {
                let id = unsafe { (mc.col_int64)(stmt, 0) };
                let name = unsafe {
                    let ptr = (mc.col_text)(stmt, 1);
                    if ptr.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(ptr).to_string_lossy().into_owned()
                    }
                };
                let dependencies = unsafe {
                    let ptr = (mc.col_text)(stmt, 2);
                    if ptr.is_null() {
                        None
                    } else {
                        Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
                    }
                };
                let group = unsafe { (mc.col_int)(stmt, 3) };
                let length = unsafe { (mc.col_int64)(stmt, 4) };
                let checksum = unsafe { (mc.col_int64)(stmt, 5) };
                let hash_name = unsafe {
                    let ptr = (mc.col_text)(stmt, 6);
                    if ptr.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(ptr).to_string_lossy().into_owned()
                    }
                };
                let kind = unsafe { (mc.col_int)(stmt, 7) };
                let state = unsafe { (mc.col_int)(stmt, 8) };
                let priority = unsafe { (mc.col_int)(stmt, 9) };

                entries.push(ManifestEntry {
                    id,
                    name,
                    dependencies,
                    group,
                    length,
                    checksum,
                    hash_name,
                    kind,
                    state,
                    priority,
                });
            }

            unsafe {
                (mc.finalize)(stmt);
                (mc.close)(db);
            }
            return Ok(entries);
        }

        unsafe {
            if !stmt.is_null() {
                (mc.finalize)(stmt);
            }
            (mc.close)(db);
        }
    }

    Err("Unable to decrypt meta database with any known or custom key.".to_string())
}

#[cfg(not(windows))]
fn load_encrypted_manifest(
    _meta_path: &Path,
    _custom_key: Option<&str>,
    _filter_sql: Option<&str>,
) -> Result<Vec<ManifestEntry>, String> {
    Err("Encrypted meta database is only supported on Windows.".to_string())
}

/// Loads all or filtered entries from the manifest database `meta`.
///
/// Automatically determines if the file is plain or encrypted.
pub fn load_manifest(
    meta_path: &Path,
    custom_key: Option<&str>,
    filter_sql: Option<&str>,
) -> Result<Vec<ManifestEntry>, String> {
    if !meta_path.is_file() {
        return Err(format!("Manifest file not found: {}", meta_path.display()));
    }

    if is_meta_plain(meta_path) {
        load_plain_manifest(meta_path, filter_sql)
    } else {
        load_encrypted_manifest(meta_path, custom_key, filter_sql)
    }
}
