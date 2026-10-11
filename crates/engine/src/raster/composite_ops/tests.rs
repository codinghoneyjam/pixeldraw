use super::common::{alpha_of, channels_of};
use super::*;
use crate::chunkstore::ChunkStore;
use crate::pixel::pack_rgba_bytes;
use crate::raster::PixelSink;
use crate::raster::shape_raster::ShapeKind;

/// A store-backed sink so these tests exercise the real read-back path.
struct Store {
    store: ChunkStore,
}

impl Store {
    fn new(w: usize, h: usize) -> Self {
        Self {
            store: ChunkStore::new(w as u32 * 32, h as u32 * 32).unwrap(),
        }
    }
}

impl PixelSink for Store {
    fn set(&mut self, x: i32, y: i32, p: u32) {
        if x < 0 || y < 0 {
            return;
        }
        self.store.set_pixel(x as u32, y as u32, p);
    }
    fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            return 0;
        }
        self.store.get_pixel(x as u32, y as u32)
    }
}

#[test]
fn channels_and_alpha_helpers_unpack_little_endian() {
    assert_eq!(channels_of(0x11223344), [0x44, 0x33, 0x22, 0x11]);
    assert_eq!(alpha_of(0x11223344), 0x11);
}

#[test]
fn shape_fill_mask_fails_closed_for_unknown_shapes() {
    let m = shape_fill_mask(&RecipeShape::Polygon { points: vec![] }, 8, 8);
    assert!(m.data.iter().all(|&v| v == 0));
}

#[test]
fn clip_alpha_clears_outside_the_rounded_rect() {
    let mut s = Store::new(8, 8);
    for y in 0..64 {
        for x in 0..64 {
            s.store.set_pixel(x, y, 0xff0000ff);
        }
    }
    let before = s.store.chunk_count();
    clip_alpha(
        &mut s,
        &Clip {
            x0: 10.0,
            y0: 10.0,
            x1: 20.0,
            y1: 20.0,
            radius: 0,
        },
        64,
        64,
    );
    assert!(before > 0);
    assert_eq!(s.store.get_pixel(15, 15), 0xff0000ff, "inside survives");
    assert_eq!(s.store.get_pixel(0, 0), 0, "outside is cleared");
}

#[test]
fn erase_polygon_punches_to_transparent() {
    let mut s = Store::new(8, 8);
    for y in 0..64 {
        for x in 0..64 {
            s.store.set_pixel(x, y, 0xff0000ff);
        }
    }
    erase_polygon(&mut s, &[[10, 10], [20, 10], [20, 20], [10, 20]], 64, 64);
    assert_eq!(s.store.get_pixel(15, 15), 0);
    assert_eq!(s.store.get_pixel(25, 25), 0xff0000ff);
}

#[test]
fn hud_brackets_paints_eight_l_corners() {
    let mut s = Store::new(8, 8);
    hud_brackets(&mut s, &Hud::default());
    // The four corners of the box must all be painted.
    for (x, y) in [(36, 60), (92, 60), (36, 82), (92, 82)] {
        assert_eq!(
            s.store.get_pixel(x as u32, y as u32),
            pack_rgba_bytes(255, 255, 255, 235),
            "corner ({x},{y})"
        );
    }
}

#[test]
fn visor_layer_darkens_only_inside_the_shape() {
    let mut s = Store::new(8, 8);
    for y in 0..64 {
        for x in 0..64 {
            s.store.set_pixel(x, y, 0xffffffff);
        }
    }
    let shape = VisorShape {
        kind: ShapeKind::Rrect,
        bbox: Some([10, 10, 54, 54]),
        points: vec![],
        radius: 0,
        width: 1,
        has_fill: true,
        has_outline: true,
    };
    visor_layer(
        &mut s,
        &shape,
        Some(&CrtLines::default()),
        64,
        64,
        0x808080ff,
        0x000000ff,
    );
    // A scanline pixel inside the visor is darker than the untouched base.
    let dark = s.store.get_pixel(40, 50);
    assert!(
        dark < 0xffffffff,
        "scanline pixel {dark:08x} was not darkened"
    );
}

#[test]
fn sheen_overlay_composites_white_onto_the_shape() {
    let mut s = Store::new(8, 8);
    for y in 0..64 {
        for x in 0..64 {
            s.store.set_pixel(x, y, 0x000000ff);
        }
    }
    let shape = VisorShape {
        kind: ShapeKind::Rrect,
        bbox: Some([0, 0, 63, 63]),
        points: vec![],
        radius: 0,
        width: 1,
        has_fill: true,
        has_outline: true,
    };
    sheen_overlay(
        &mut s,
        &shape,
        Some(&Sheen {
            p1: [10.0, 10.0],
            p2: [50.0, 50.0],
            dx: 5.0,
        }),
        64,
        64,
    );
    // Some pixel must now carry white-ish luminance from the sheen.
    let mut any = false;
    for y in 0..64 {
        for x in 0..64 {
            let c = channels_of(s.store.get_pixel(x, y));
            if c[0] > 100 {
                any = true;
            }
        }
    }
    assert!(any, "the sheen line never landed");
}
