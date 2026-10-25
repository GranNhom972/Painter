//! File access for the UI: read, atomic write, and the file passed on the
//! command line ("Open with Paint").

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::ipc::{InvokeBody, Request, Response};

#[tauri::command]
pub async fn read_file(path: String) -> Result<Response, String> {
    fs::read(&path).map(Response::new).map_err(|e| format!("{path}: {e}"))
}

/// Body: the file bytes. Header `x-path`: the percent-encoded destination.
#[tauri::command]
pub async fn write_file(request: Request<'_>) -> Result<(), String> {
    let encoded = request.headers().get("x-path").and_then(|v| v.to_str().ok()).ok_or("missing x-path header")?;
    let path = percent_decode(encoded)?;
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected file bytes".into());
    };
    write_atomic(Path::new(&path), bytes)
}

#[tauri::command]
pub fn initial_file() -> Option<String> {
    file_from_args(std::env::args().skip(1))
}

/// First argument naming an existing file (plain path or `file://` URI).
pub fn file_from_args(args: impl IntoIterator<Item = String>) -> Option<String> {
    args.into_iter()
        .filter(|a| !a.starts_with('-'))
        .map(|a| match a.strip_prefix("file://") {
            Some(rest) => percent_decode(rest).unwrap_or(a),
            None => a,
        })
        .find(|a| Path::new(a).is_file())
}

/// Writes through a temporary file then renames it, so a failed save never
/// leaves a truncated picture behind. Every call uses its own temporary file
/// (overlapping saves are safe), a symbolic link is followed to its target, and
/// an existing file keeps its permissions.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let path: PathBuf = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    path.file_name().ok_or("invalid file name")?;
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".paint-{}-{n}.tmp", std::process::id()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        if let Ok(meta) = fs::metadata(&path) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(fail)
}

/// Decodes `%XX` escapes (UTF-8).
pub fn percent_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3).ok_or("truncated escape")?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| format!("bad escape %{hex}"))?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}
