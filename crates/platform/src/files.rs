//! Byte IO plus the file-naming rules copied from `src/io/file_io.js`.
//!
//! The naming half is pure so the blocking half (`dialogs`) stays testable:
//! `sanitize_file_name`, `trailing_extension`, `fix_extension`, `filter_for`
//! and `extensions_from_accept` are 1:1 ports of the JS helpers.

use std::fs;
use std::path::{Path, PathBuf};

use crate::PlatformError;

/// `readJsonFile`'s 64 MiB guard (`file_io.js` L21).
pub const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;

/// Which kind of file a dialog is for (replaces the JS ad-hoc `accept`/`mime`
/// arguments of `saveTextFile`/`pickFile`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Png,
    Json,
    Any,
}

impl FileKind {
    /// MIME the JS call site would have passed: `file_actions.js` uses
    /// `"image/png"` for PNG export and the default `"application/json"`.
    pub fn mime(self) -> &'static str {
        match self {
            FileKind::Png => "image/png",
            // "" is fine: filter_for() only consults the MIME when the name has
            // no extension, and the JS JSON exports always carry one.
            FileKind::Json | FileKind::Any => "",
        }
    }

    /// Extension appended when a caller passes a bare name. `Json` is the
    /// app's document extension (`docs/roadmap/phase2.yaml` 짠W9); `Any` never
    /// rewrites a name.
    pub fn default_extension(self) -> Option<&'static str> {
        match self {
            FileKind::Png => Some("png"),
            FileKind::Json => Some("pixeldraw.json"),
            FileKind::Any => None,
        }
    }

    /// Filter for the open dialog ??port of `pickFile(accept)`'s extension
    /// parsing (`file_io.js` L50-60): all call sites pass `.json` only.
    pub fn open_filter(self) -> FileFilter {
        match self {
            FileKind::Png => FileFilter::new("PNG", &["png"]),
            FileKind::Json => FileFilter::new("JSON", &["json"]),
            FileKind::Any => FileFilter::no_filter(),
        }
    }
}

/// A dialog filter, 1:1 with the JS `filterFor` result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileFilter {
    fn new(name: &str, extensions: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            extensions: extensions.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// The JS `{name, extensions: []}` case: no filter at all.
    pub fn no_filter() -> Self {
        Self::new("Any", &[])
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

/// JS `/\.([a-z0-9]+)$/i` ??the trailing dot-segment, ASCII alphanumeric only.
pub fn trailing_extension(name: &str) -> Option<String> {
    let (_, ext) = name.rsplit_once('.')?;
    if ext.is_empty() || !ext.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    Some(ext.to_string())
}

/// TRAP: JS `slice(0, 80)` counts UTF-16 units; this counts `char`s. Only
/// differs for names outside the BMP, which the sanitizer cannot produce.
fn truncate_80(s: &str) -> String {
    s.chars().take(80).collect()
}

/// Port of `sanitizeFileName` (`file_io.js` L69-77): banned characters (and
/// runs of them) collapse to one `_`, then leading/trailing whitespace+dot are
/// trimmed, then 80 chars, then trimmed again, then `untitled` if empty.
pub fn sanitize_file_name(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    let mut in_banned_run = false;
    for ch in name.chars() {
        let banned = matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            || (ch as u32) <= 0x1f;
        if banned {
            in_banned_run = true;
        } else {
            if in_banned_run {
                s.push('_');
                in_banned_run = false;
            }
            s.push(ch);
        }
    }
    if in_banned_run {
        s.push('_');
    }
    let trimmed = s.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let cut = truncate_80(trimmed);
    let result = cut.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if result.is_empty() {
        "untitled".to_string()
    } else {
        result.to_string()
    }
}

/// Appends `kind`'s default extension only when the name carries none.
///
/// TRAP: JS never rewrites a name the caller already built ??`file_actions.js`
/// passes `untitled.draw.json`/`untitled.png` and only sanitizes it. So a name
/// that already has an extension is returned untouched.
pub fn fix_extension(name: &str, kind: FileKind) -> String {
    match (kind.default_extension(), trailing_extension(name)) {
        (Some(ext), None) => format!("{name}.{ext}"),
        _ => name.to_string(),
    }
}

/// Port of `filterFor` (`file_io.js` L31-36).
pub fn filter_for(suggested_name: &str, mime: &str) -> FileFilter {
    if let Some(ext) = trailing_extension(suggested_name) {
        return FileFilter::new(&ext.to_uppercase(), &[&ext.to_lowercase()]);
    }
    if mime.contains("png") {
        return FileFilter::new("PNG", &["png"]);
    }
    FileFilter::new("JSON", &["json"])
}

/// Filter the save dialog shows: JS derives it from the **raw** suggested name
/// plus the MIME (`saveNative`), so mirror that on the sanitized+fixed name.
pub fn save_filter(name: &str, kind: FileKind) -> FileFilter {
    let fixed = fix_extension(&sanitize_file_name(name), kind);
    filter_for(&fixed, kind.mime())
}

/// Port of `pickFileNative`'s accept-string parsing (`file_io.js` L51-57):
/// split on `,`, strip a leading `.`, keep the last MIME segment, drop
/// anything that is not `[a-z0-9]+`, dedupe preserving first-seen order.
pub fn extensions_from_accept(accept: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in accept.split(',') {
        let token = raw.trim();
        let token = token.strip_prefix('.').unwrap_or(token);
        let token = token.rsplit('/').next().unwrap_or(token);
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_alphanumeric()) {
            continue;
        }
        let lowered = token.to_lowercase();
        if !out.contains(&lowered) {
            out.push(lowered);
        }
    }
    out
}

/// Port of `readJsonFile`'s size guard + text read (`file_io.js` L159-173):
/// JSON parsing itself belongs to the engine's serializer.
pub fn read_json_text(path: &Path) -> Result<String, PlatformError> {
    let len = fs::metadata(path)?.len();
    if len > MAX_JSON_BYTES {
        return Err(PlatformError::FileTooLarge {
            size: len,
            limit: MAX_JSON_BYTES,
        });
    }
    Ok(fs::read_to_string(path)?)
}

/// Port of the Tauri `read_file_bytes` command minus base64 (`lib.rs` L55-59).
pub fn read_bytes(path: &Path) -> Result<Vec<u8>, PlatformError> {
    Ok(fs::read(path)?)
}

/// Port of the Tauri `write_file_bytes` command minus base64 (`lib.rs` L49-53).
pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), PlatformError> {
    Ok(fs::write(path, bytes)?)
}

/// Atomic write: `<dir>/<name>.tmp` then rename over the destination.
///
/// TRAP (Windows): `fs::rename` mirrors `MoveFileEx` and historically fails
/// when the destination exists, so the second attempt removes the destination
/// first. Either way the temp file is removed on failure ??no `.tmp` litter.
pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), PlatformError> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    // Never create a missing directory: the JS bridge let the OS report it.
    if !dir.is_dir() {
        return Err(PlatformError::NoDir);
    }
    let tmp = dir.join(temp_name(path));
    let written = fs::write(&tmp, bytes);
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(PlatformError::Io(e));
    }
    let renamed = fs::rename(&tmp, path);
    let renamed = match renamed {
        Ok(()) => Ok(()),
        Err(first) => match fs::remove_file(path) {
            Ok(()) => fs::rename(&tmp, path).map_err(PlatformError::Io),
            Err(_) => Err(PlatformError::Io(first)),
        },
    };
    match renamed {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn temp_name(path: &Path) -> PathBuf {
    let base = path
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("pixeldraw"));
    let mut name = base.into_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests;
