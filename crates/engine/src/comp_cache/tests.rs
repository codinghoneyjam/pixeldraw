//! In-crate unit tests for the composite cache.
//!
//! The integration tests (`crates/engine/tests/comp_cache.rs`) assert the
//! behaviour an app sees; what is left here is the part that needs the module
//! internals — the budget's second pass, the `dirty_all` expansion, and the
//! `LayerChangeReason` arm whose JS twin never fires.

use super::{Budget, CompCache};
use crate::model::command::LayerChangeReason;
use crate::model::event::ModelEvent;
use crate::model::settings::{SettingKey, SettingValue};
use crate::view::VisibleChunkRange;

/// A 128×64 cache whose previous frame finished — dirty set empty, `dirty_all`
/// clear, so an assertion about "nothing is pending" is meaningful.
fn cache() -> CompCache {
    let mut c = CompCache::new();
    c.ensure(128, 64);
    c.expand_dirty_all();
    c.dirty.clear();
    c
}

/// The same cache with `invalidateAll` expanded into the 4×2 chunk grid, which is
/// what `_update` does before `paintDirty` runs.
fn all_dirty() -> CompCache {
    let mut c = cache();
    c.invalidate_all();
    c.expand_dirty_all();
    c
}

fn vis(cx0: u32, cy0: u32, cx1: u32, cy1: u32) -> Option<VisibleChunkRange> {
    Some(VisibleChunkRange { cx0, cy0, cx1, cy1 })
}

#[test]
fn the_visible_pass_ignores_the_budget() {
    let mut c = all_dirty();
    let layers: Vec<crate::composite::LayerSource<'_>> = Vec::new();
    let remaining = c.paint_dirty_budget(&layers, vis(0, 0, 1, 0), Budget::Chunks(0));
    assert_eq!(c.last_painted(), 2, "the two visible chunks paint anyway");
    assert_eq!(c.dirty_len(), 6);
    assert!(remaining);

    // The next frame only gets what its budget allows, in (cy, cx) order.
    c.paint_dirty_budget(&layers, vis(0, 0, 1, 0), Budget::Chunks(2));
    assert_eq!(c.last_painted(), 2);
    assert_eq!(c.dirty_len(), 4);

    c.paint_dirty_budget(&layers, None, Budget::Unlimited);
    assert_eq!(c.last_painted(), 4);
    assert!(!c.paint_dirty_budget(&layers, None, Budget::Unlimited));
    assert_eq!(c.dirty_len(), 0);
    assert_eq!(c.last_painted(), 0, "an empty dirty set paints nothing");
}

#[test]
fn a_none_viewport_paints_nothing_without_a_budget() {
    let mut c = all_dirty();
    let layers: Vec<crate::composite::LayerSource<'_>> = Vec::new();
    // JS `inVis` is false for every key when `visibleChunkRange` returned null.
    c.paint_dirty_budget(&layers, None, Budget::Chunks(0));
    assert_eq!(c.last_painted(), 0);
    assert_eq!(c.dirty_len(), 8);
}

#[test]
fn a_budget_exhausted_mid_frame_leaves_the_rest_dirty() {
    let mut c = all_dirty();
    let layers: Vec<crate::composite::LayerSource<'_>> = Vec::new();
    let remaining = c.paint_dirty_budget(&layers, None, Budget::Chunks(3));
    assert_eq!(c.last_painted(), 3);
    assert_eq!(c.dirty_len(), 5);
    assert!(remaining);
}

#[test]
fn out_of_grid_keys_are_dropped_not_retried() {
    let mut c = all_dirty();
    // `invalidate_chunks` already filters these; the paint pass re-checks so a
    // stale key can never index past the buffer.
    c.dirty.insert(crate::constants::chunk_key(9, 9));
    let layers: Vec<crate::composite::LayerSource<'_>> = Vec::new();
    c.paint_dirty_budget(&layers, None, Budget::Unlimited);
    assert_eq!(c.dirty_len(), 0);
    assert_eq!(c.last_painted(), 8);
}

#[test]
fn dirty_all_counts_as_the_whole_grid_until_it_is_expanded() {
    let mut c = cache();
    c.invalidate_all();
    assert_eq!(c.dirty_len(), 8);
    c.expand_dirty_all();
    assert_eq!(c.dirty_len(), 8);
    c.dirty.remove(&crate::constants::chunk_key(0, 0));
    assert_eq!(c.dirty_len(), 7, "one chunk painted, seven still pending");
}

// A canvas whose width is not a multiple of CHUNK_PX cannot exist through
// `is_valid_canvas_size`, which is what lets the row copy run unclipped. The
#[test]
fn canvas_sizes_are_chunk_multiples() {
    assert!(crate::constants::is_valid_canvas_size(512, 512));
    assert!(crate::constants::is_valid_canvas_size(2048, 1088));
    assert!(!crate::constants::is_valid_canvas_size(48, 32));
    assert!(!crate::constants::is_valid_canvas_size(32, 33));
    let mut c = CompCache::new();
    // `ensure` is unvalidated, so a partial edge chunk still has to be safe.
    c.ensure(48, 32);
    c.invalidate_all();
    assert_eq!(c.dirty_len(), 1, "48/32 floors to one chunk column");
    c.invalidate_chunks(&[(1, 0)]);
    assert_eq!(c.dirty_len(), 1, "the past-the-edge column is ignored");
}

#[test]
fn layers_active_is_the_only_event_that_repaints_nothing() {
    let mut c = cache();
    // `reason === "active"` skipped both the invalidation and the render request.
    let active = ModelEvent::layers_changed(LayerChangeReason::Active, vec!["layer-0001".into()]);
    assert!(!c.apply_event(&active));
    assert_eq!(c.dirty_len(), 0);
}

#[test]
fn layers_prop_invalidates_because_the_event_has_no_field_name() {
    let mut c = cache();
    let prop = ModelEvent::layers_changed(LayerChangeReason::Prop, vec!["layer-0001".into()]);
    assert!(c.apply_event(&prop));
    assert_eq!(c.dirty_len(), 8);
    // Every other reason is a structural change: the pixels can move.
    for reason in [
        LayerChangeReason::Add,
        LayerChangeReason::Remove,
        LayerChangeReason::Move,
    ] {
        let mut fresh = cache();
        assert!(fresh.apply_event(&ModelEvent::layers_changed(reason, vec![])));
        assert_eq!(
            fresh.dirty_len(),
            8,
            "{reason:?} must invalidate everything"
        );
    }
}

/// The JS arm that never fires — kept so the day `EventDetail::Layers` grows a
/// field name there is nothing to re-derive.
///
/// `#[ignore]`: nothing in this tree provides `field`, so this cannot be wired
/// to `apply_event` without inventing data the event does not carry (and the
/// engine model is read-only for W3).
#[test]
#[ignore = "EventDetail::Layers carries no field name; docs/roadmap/index.yaml corrections"]
fn layers_prop_with_a_field_name_skips_the_repaint() {
    // The exact JS: `if (d.reason === "prop" && d.field === "name") return;`
    fn invalidates(reason: LayerChangeReason, field: Option<&str>) -> bool {
        if reason == LayerChangeReason::Active {
            return false;
        }
        if reason == LayerChangeReason::Prop && field == Some("name") {
            return false;
        }
        true
    }
    assert!(!invalidates(LayerChangeReason::Prop, Some("name")));
    assert!(!invalidates(LayerChangeReason::Active, None));
    assert!(invalidates(LayerChangeReason::Prop, Some("opacity")));
    assert!(invalidates(LayerChangeReason::Prop, None));
}

#[test]
fn settings_only_asks_for_a_frame_for_the_grid() {
    let mut c = cache();
    let grid = ModelEvent::settings_changed(SettingKey::GridMode, SettingValue::Mode("unit"));
    assert!(c.apply_event(&grid));
    assert_eq!(c.dirty_len(), 0, "the grid changes no composited pixel");
    let pen = ModelEvent::settings_changed(SettingKey::PenSize, SettingValue::Int(2));
    assert!(!c.apply_event(&pen));
    assert_eq!(c.dirty_len(), 0);
}

#[test]
fn the_unsubscribed_events_are_ignored() {
    let mut c = cache();
    for ev in [
        ModelEvent::history_changed(crate::model::history::HistoryState::default()),
        ModelEvent::status_message("warn", "x", "LAYER_LOCKED"),
        ModelEvent::tool_state("rect", "{}"),
    ] {
        assert!(!c.apply_event(&ev), "{} must not request a frame", ev.name);
        assert_eq!(c.dirty_len(), 0);
    }
    // A pixels event always asks for a frame even when it names no chunk.
    assert!(c.apply_event(&ModelEvent::pixels_changed("layer-0001", vec![])));
    assert!(c.apply_event(&ModelEvent::pixels_all_changed("layer-0001")));
}
