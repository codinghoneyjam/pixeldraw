// <META - FILE SUMMARY - Tauri shell: run() plus the native file bridge the editor's UI calls>
//
// The UI is plain ESM loaded straight from index.html (no bundler, no build step --
// see docs/contract.md §5), so this crate owns only what a browser cannot give
// the editor: native save/open dialogs and unrestricted filesystem writes.
//
// Every command here is reached from src/io/file_io.js, which feature-detects
// `window.__TAURI__`. When the same code runs in a browser (`python run.py`),
// that global is absent and the File System Access API / download paths are used
// instead. Nothing in src/ imports Node or Tauri types, so the module graph
// stays identical for the browser, the tests and the desktop shell.

use std::fs;

use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

/// Extension filter for a native dialog, serialized from the editor's picker.
#[derive(Debug, Deserialize)]
pub struct FileFilter {
    name: String,
    extensions: Vec<String>,
}

// <META - ROLE : Turn a native FilePath into a UTF-8 string | L20-22>
fn path_string(path: FilePath) -> Option<String> {
    path.into_path().ok().map(|p| p.to_string_lossy().to_string())
}

// <META - ROLE : Show a native save dialog; null when the user cancels | L24-35>
#[tauri::command]
fn pick_save_path(
    app: AppHandle,
    suggested: Option<String>,
    filter: Option<FileFilter>,
) -> Option<String> {
    let mut builder = app.dialog().file();
    if let Some(name) = suggested {
        builder = builder.set_file_name(name);
    }
    if let Some(filter) = filter {
        let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
        builder = builder.add_filter(filter.name, &extensions);
    }
    path_string(builder.blocking_save_file()?)
}

// <META - ROLE : Show a native open dialog; null when the user cancels | L37-47>
#[tauri::command]
fn pick_open_path(app: AppHandle, filter: Option<FileFilter>) -> Option<String> {
    let mut builder = app.dialog().file();
    if let Some(filter) = filter {
        let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
        builder = builder.add_filter(filter.name, &extensions);
    }
    path_string(builder.blocking_pick_file()?)
}

// <META - ROLE : Write bytes to a path the user chose in the save dialog | L49-53>
#[tauri::command]
fn write_file_bytes(path: String, data: String) -> Result<(), String> {
    fs::write(path, base64_decode(&data)?).map_err(|e| e.to_string())
}

// <META - ROLE : Read bytes from a path the user chose in the open dialog | L55-59>
#[tauri::command]
fn read_file_bytes(path: String) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(base64_encode(&bytes))
}

// The bridge uses base64 rather than raw bytes so the payload only depends on
// JSON and never on a particular Tauri IPC version. Both helpers are the plain
// RFC 4648 alphabet the browser produces with btoa/atob.
const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

// <META - ROLE : RFC 4648 decode, tolerant of = padding | L66-85>
fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u8 = 0;
    for &ch in text.as_bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return Err(format!("invalid base64 byte 0x{ch:02x}")),
        } as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1u32 << bits) - 1;
        }
    }
    Ok(out)
}

// <META - ROLE : RFC 4648 encode with = padding | L88-103>
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | (((*chunk.get(1).unwrap_or(&0)) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0)) as u32;
        out.push(B64_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(B64_ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64_ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64_ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_save_path,
            pick_open_path,
            write_file_bytes,
            read_file_bytes,
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : base64 helpers agree with the browser's btoa/atob strings | L151-176>
    #[test]
    fn base64_round_trip() {
        let cases: [&[u8]; 5] = [b"", b"f", b"fo", b"foo", b"fooba"];
        let expected = ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYmE="];
        for (bytes, want) in cases.iter().zip(expected.iter()) {
            assert_eq!(&base64_encode(bytes), want);
        }
    }

    #[test]
    fn base64_decodes_browser_padding() {
        for bytes in [b"".as_slice(), b"f", b"fo", b"foo", b"fooba"] {
            let encoded = base64_encode(bytes);
            assert_eq!(base64_decode(&encoded).unwrap(), bytes);
        }
    }

    #[test]
    fn base64_rejects_foreign_bytes() {
        assert!(base64_decode("a b").is_err());
    }
}

