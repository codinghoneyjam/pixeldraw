use super::*;

#[test]
fn decodes_adam7_interlaced_rgba() {
    let w = 3usize;
    let h = 3usize;
    let src: Vec<u8> = (0..w * h * 4).map(|i| ((i * 17 + 3) % 256) as u8).collect();
    const PASSES: [(usize, usize, usize, usize); 7] = [
        (0, 0, 8, 8),
        (4, 0, 8, 8),
        (0, 4, 4, 8),
        (2, 0, 4, 4),
        (0, 2, 2, 4),
        (1, 0, 2, 2),
        (0, 1, 1, 2),
    ];
    let mut raw = Vec::new();
    for (xs, ys, xstep, ystep) in PASSES {
        let mut coords = Vec::new();
        let mut y = ys;
        while y < h {
            let mut x = xs;
            while x < w {
                coords.push((x, y));
                x += xstep;
            }
            y += ystep;
        }
        if coords.is_empty() {
            continue;
        }
        let pw = coords.iter().filter(|(_, yy)| *yy == coords[0].1).count();
        for row_start in (0..coords.len()).step_by(pw) {
            raw.push(0); // filter 0
            for &(x, y) in &coords[row_start..row_start + pw] {
                raw.extend_from_slice(&src[(y * w + x) * 4..(y * w + x) * 4 + 4]);
            }
        }
    }
    let mut idat = Vec::new();
    let mut enc = flate2::write::ZlibEncoder::new(&mut idat, flate2::Compression::default());
    std::io::Write::write_all(&mut enc, &raw).expect("deflate");
    enc.finish().expect("deflate");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&be32(w as u32));
    ihdr.extend_from_slice(&be32(h as u32));
    ihdr.extend_from_slice(&[8, 6, 0, 0, 1]);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&SIG);
    push_chunk(&mut bytes, b"IHDR", &ihdr);
    push_chunk(&mut bytes, b"IDAT", &idat);
    push_chunk(&mut bytes, b"IEND", &[]);
    let img = decode_png(&bytes).expect("interlaced");
    assert_eq!(img.rgba, src);
}

#[test]
fn ancillary_chunks_round_trip_with_the_image() {
    let rgba = vec![1u8, 2, 3, 4];
    let bytes = encode_png_with_chunks(&rgba, 1, 1, &[("dtAt", b"atlas-json")]).expect("encode");
    let img = decode_png(&bytes).expect("decode");
    assert_eq!(img.rgba, rgba);
    assert_eq!(img.metadata_chunk("dtAt"), Some(&b"atlas-json"[..]));
}

#[test]
fn every_asset_work_png_decodes() {
    use std::path::Path;
    fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../asset_work");
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(!files.is_empty(), "asset_work corpus is present");
    for path in files {
        let bytes = std::fs::read(&path).expect("read png");
        match decode_png(&bytes) {
            Ok(img) => {
                assert!(img.width > 0 && img.height > 0);
                assert_eq!(
                    img.rgba.len(),
                    img.width as usize * img.height as usize * 4,
                    "{}",
                    path.display()
                );
            }
            Err(e) => panic!("{}: {} ({})", path.display(), e.code, e.message),
        }
    }
}
