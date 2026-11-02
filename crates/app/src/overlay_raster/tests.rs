use super::*;
use pixel_engine::view::View;

fn view1() -> View {
    View::new(1.0, 0.0, 0.0).expect("valid view")
}

fn px(fb: &Framebuffer, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * fb.width + x) * 4) as usize;
    [fb.rgba[o], fb.rgba[o + 1], fb.rgba[o + 2], fb.rgba[o + 3]]
}

#[test]
fn mask_blit_scales_by_zoom() {
    let mut fb = Framebuffer::new(16, 16);
    let ops = vec![OverlayOp::MaskBlit {
        x: 1,
        y: 1,
        w: 2,
        h: 1,
        data: std::sync::Arc::from(vec![0xff0000ff, 0].into_boxed_slice()),
    }];
    rasterize_overlay(&mut fb, &ops, view1(), 1.0);
    assert_eq!(px(&fb, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&fb, 2, 1), [0, 0, 0, 0], "zero cells stay transparent");
}

#[test]
fn dashed_frame_skips_the_gaps() {
    let mut fb = Framebuffer::new(16, 4);
    let ops = vec![OverlayOp::StrokeRect {
        x: 0.0,
        y: 0.0,
        w: 12.0,
        h: 2.0,
        rgba: [255, 255, 255, 255],
        dash: Some((2.0, 2.0)),
        difference: false,
    }];
    rasterize_overlay(&mut fb, &ops, view1(), 1.0);
    assert_eq!(px(&fb, 0, 0), [255, 255, 255, 255]);
    assert_eq!(px(&fb, 1, 0), [255, 255, 255, 255]);
    assert_eq!(px(&fb, 2, 0), [0, 0, 0, 0], "dash gap");
    assert_eq!(px(&fb, 3, 0), [0, 0, 0, 0], "dash gap");
    assert_eq!(px(&fb, 4, 0), [255, 255, 255, 255], "dash resumes");
}

#[test]
fn difference_line_inverts() {
    let mut fb = Framebuffer::new(8, 1);
    fb.fill_rect(0, 0, 8, 1, [0, 0, 0, 255]);
    let ops = vec![OverlayOp::Line {
        x0: 0.0,
        y0: 0.0,
        x1: 7.0,
        y1: 0.0,
        rgba: [255, 255, 255, 255],
        width: 1.0,
        difference: true,
    }];
    rasterize_overlay(&mut fb, &ops, view1(), 1.0);
    assert_eq!(px(&fb, 3, 0)[0], 255, "black inverts to white");
}

fn inside(r: (i32, i32, i32, i32), x: u32, y: u32) -> bool {
    let (x, y) = (x as i32, y as i32);
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

#[test]
fn overlay_bbox_is_none_for_an_empty_list() {
    assert_eq!(overlay_bbox(&[], view1(), 1.0), None);
}

/// The `UI-5` contract: the box must contain every pixel the rasterizer
/// could paint, or the erase/repaint upload leaves stale pixels behind.
#[test]
fn overlay_bbox_covers_every_painted_pixel() {
    let base = [9, 9, 9, 255];
    let ops = vec![
        OverlayOp::MaskBlit {
            x: 2,
            y: 1,
            w: 3,
            h: 2,
            data: std::sync::Arc::from(vec![0xff0000ff, 0, 0, 0, 0xff00ff00, 0].into_boxed_slice()),
        },
        OverlayOp::StrokeRect {
            x: 20.0,
            y: 4.0,
            w: 8.0,
            h: 6.0,
            rgba: [255, 255, 255, 255],
            dash: Some((2.0, 2.0)),
            difference: false,
        },
        OverlayOp::Line {
            x0: 2.0,
            y0: 20.0,
            x1: 26.0,
            y1: 21.0,
            rgba: [255, 0, 0, 255],
            width: 1.0,
            difference: true,
        },
        OverlayOp::Circle {
            cx: 16.0,
            cy: 12.0,
            r: 3.0,
            rgba: [0, 255, 0, 255],
            width: 2.0,
            dash: None,
        },
    ];
    let box_ = overlay_bbox(&ops, view1(), 1.0).expect("the ops paint something");
    let mut fb = Framebuffer::new(32, 24);
    fb.fill_rect(0, 0, 32, 24, base);
    rasterize_overlay(&mut fb, &ops, view1(), 1.0);
    let mut painted = 0;
    for y in 0..fb.height {
        for x in 0..fb.width {
            if px(&fb, x, y) != base {
                painted += 1;
                assert!(inside(box_, x, y), "({x},{y}) sits outside {box_:?}");
            }
        }
    }
    assert!(painted > 10, "the fixture must actually paint pixels");
}

#[test]
fn mask_blit_bbox_scales_with_zoom() {
    let ops = vec![OverlayOp::MaskBlit {
        x: 2,
        y: 1,
        w: 3,
        h: 2,
        data: std::sync::Arc::from(vec![1u32, 0, 0, 0, 0, 0].into_boxed_slice()),
    }];
    assert_eq!(overlay_bbox(&ops, view1(), 1.0), Some((2, 1, 3, 2)));
    // zoom 2: cell 2 at (4, 2), last cell (4, 2) → (8, 4) + cell.
    let zoomed = View::new(2.0, 0.0, 0.0).expect("valid view");
    assert_eq!(overlay_bbox(&ops, zoomed, 1.0), Some((4, 2, 6, 4)));
}
