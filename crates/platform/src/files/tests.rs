//! Unit tests for the pure half of `files.rs` (the JS naming rules) plus the
//! blocking half that is still testable: atomic write against a real temp dir.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// Fresh `%TEMP%\pixeldraw-test-<pid>-<n>`; the caller removes it.
fn temp_dir_unique(tag: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pixeldraw-{tag}-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sanitize_mirrors_js_regexes() {
    // Banned runs collapse to one "_" (`[\\/:*?"<>|\x00-\x1f]+`), then trim.
    assert_eq!(sanitize_file_name(r#"a/b:c*d?"<>|f"#), "a_b_c_d_f");
    assert_eq!(sanitize_file_name(r#"a/b|c\f"#), "a_b_c_f");
    assert_eq!(sanitize_file_name(" ..untitled.. "), "untitled");
    assert_eq!(sanitize_file_name("///"), "_");
    assert_eq!(sanitize_file_name("   "), "untitled");
    assert_eq!(sanitize_file_name(""), "untitled");
    assert_eq!(sanitize_file_name("a\t\nb"), "a_b");
    assert_eq!(sanitize_file_name(&"x".repeat(120)), "x".repeat(80));
}

#[test]
fn trailing_extension_matches_js_regex() {
    assert_eq!(
        trailing_extension("untitled.draw.json").as_deref(),
        Some("json")
    );
    assert_eq!(trailing_extension("a.PNG").as_deref(), Some("PNG"));
    assert_eq!(
        trailing_extension(".gitignore").as_deref(),
        Some("gitignore")
    );
    assert_eq!(trailing_extension("a.tar.gz").as_deref(), Some("gz"));
    assert_eq!(trailing_extension("noext"), None);
    assert_eq!(trailing_extension("trail."), None);
    assert_eq!(trailing_extension("a.-1"), None);
    assert_eq!(trailing_extension("a b"), None);
}

#[test]
fn fix_extension_appends_only_when_bare() {
    assert_eq!(fix_extension("foo", FileKind::Json), "foo.pixeldraw.json");
    assert_eq!(fix_extension("foo", FileKind::Png), "foo.png");
    assert_eq!(fix_extension("foo", FileKind::Any), "foo");
    // JS parity: an already-suffixed name is never rewritten.
    assert_eq!(
        fix_extension("untitled.draw.json", FileKind::Json),
        "untitled.draw.json"
    );
    assert_eq!(fix_extension("a.png", FileKind::Png), "a.png");
    assert_eq!(
        fix_extension("untitled.draw.json", FileKind::Png),
        "untitled.draw.json"
    );
}

#[test]
fn filter_for_mirrors_js_branching() {
    assert_eq!(
        filter_for("untitled.draw.json", ""),
        FileFilter::new("JSON", &["json"])
    );
    assert_eq!(filter_for("x.PNG", ""), FileFilter::new("PNG", &["png"]));
    assert_eq!(
        filter_for("noext", "image/png"),
        FileFilter::new("PNG", &["png"])
    );
    assert_eq!(
        filter_for("noext", "application/json"),
        FileFilter::new("JSON", &["json"])
    );
    assert_eq!(filter_for("noext", ""), FileFilter::new("JSON", &["json"]));
}

#[test]
fn open_filters_match_the_js_accept_string() {
    assert_eq!(
        FileKind::Png.open_filter(),
        FileFilter::new("PNG", &["png"])
    );
    assert_eq!(
        FileKind::Json.open_filter(),
        FileFilter::new("JSON", &["json"])
    );
    assert!(FileKind::Any.open_filter().is_empty());
    assert_eq!(
        save_filter("foo", FileKind::Json),
        FileFilter::new("JSON", &["json"])
    );
    assert_eq!(
        save_filter("foo", FileKind::Png),
        FileFilter::new("PNG", &["png"])
    );
    assert_eq!(
        save_filter("a/b", FileKind::Png),
        FileFilter::new("PNG", &["png"])
    );
}

#[test]
fn extensions_from_accept_dedupes_like_js() {
    assert_eq!(
        extensions_from_accept(".json,application/json"),
        vec!["json"]
    );
    assert_eq!(extensions_from_accept(" json , JSON "), vec!["json"]);
    assert_eq!(extensions_from_accept(".draw.json,.png"), vec!["png"]);
    assert_eq!(extensions_from_accept("*.png"), Vec::<String>::new());
    assert_eq!(extensions_from_accept(""), Vec::<String>::new());
    assert_eq!(extensions_from_accept("a-b,c"), vec!["c"]);
    assert_eq!(extensions_from_accept("image/png,.png"), vec!["png"]);
}

#[test]
fn write_bytes_atomic_round_trips() {
    let dir = temp_dir_unique("round");
    let path = dir.join("doc.pixeldraw.json");
    let payload = b"{\"format\":\"draw_tool.document\"}\n";
    write_bytes_atomic(&path, payload).expect("write");
    assert_eq!(read_bytes(&path).expect("read"), payload);
    // The temp file must never survive a successful write.
    assert_eq!(fs::read_dir(&dir).expect("list").count(), 1);
    assert_eq!(
        read_json_text(&path).expect("text"),
        "{\"format\":\"draw_tool.document\"}\n"
    );
    cleanup(&dir);
}

#[test]
fn write_bytes_atomic_replaces_an_existing_file() {
    let dir = temp_dir_unique("over");
    let path = dir.join("autosave.json");
    write_bytes_atomic(&path, b"first").expect("first");
    write_bytes_atomic(&path, b"second-and-longer").expect("second");
    assert_eq!(read_bytes(&path).expect("read"), b"second-and-longer");
    assert!(
        fs::read_dir(&dir)
            .expect("list")
            .all(|e| e.expect("e").path() != path.with_extension("tmp"))
    );
    cleanup(&dir);
}

#[test]
fn write_bytes_atomic_fails_without_creating_the_parent() {
    let dir = temp_dir_unique("missing");
    let orphan = dir.join("nope").join("nested").join("doc.json");
    let err = write_bytes_atomic(&orphan, b"x").expect_err("must fail");
    assert!(matches!(err, PlatformError::NoDir), "{err:?}");
    assert!(!dir.join("nope").exists(), "parent must not be created");
    cleanup(&dir);
}

#[test]
fn write_bytes_skips_the_temp_file() {
    let dir = temp_dir_unique("bare");
    let path = dir.join("plain.json");
    write_bytes(&path, b"one").expect("write");
    assert_eq!(read_bytes(&path).expect("read"), b"one");
    // Plain write leaves no .tmp behind either.
    assert!(fs::read_dir(&dir).expect("list").all(|e| {
        let p = e.expect("e").path();
        p == path
    }));
    cleanup(&dir);
}

#[test]
fn read_bytes_errors_on_missing_paths() {
    let dir = temp_dir_unique("lost");
    let err = read_bytes(&dir.join("gone.txt")).expect_err("missing");
    assert!(matches!(err, PlatformError::Io(_)), "{err:?}");
    cleanup(&dir);
}

#[test]
fn read_json_text_rejects_oversized_files() {
    // Only exercises the guard, so a sparse file stands in for a 64 MiB payload.
    let dir = temp_dir_unique("big");
    let path = dir.join("huge.json");
    let file = fs::File::create(&path).expect("create");
    file.set_len(MAX_JSON_BYTES + 1).expect("set_len");
    drop(file);
    let err = read_json_text(&path).expect_err("too large");
    assert!(matches!(err, PlatformError::FileTooLarge { .. }), "{err:?}");
    cleanup(&dir);
}
