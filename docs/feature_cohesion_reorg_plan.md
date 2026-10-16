# Feature-Cohesion Reorganization Plan

## Current pain points

- ~~Layer-oriented modules scatter one feature across `tools/`, `ui/optionbar/`, and engine raster code.~~ resolved
- ~~Feature ownership is split between tool state and option-bar UI; discoverability is low.~~ resolved
- ~~Existing pub paths must stay stable during migration; avoid broad renames until proven.~~ resolved
- ~~Some UI feature helpers live in partially wired directories and are not compiled yet.~~ resolved

## Target layout

```
crates/app/src/features/
  mod.rs                  # eight façades, no logic
  arc.rs  + arc/{tool,preview,optionbar,tests}.rs
  gradient.rs + gradient/{tool,preview,optionbar,tests}.rs
  pen.rs + pen/{core,preview,optionbar}.rs
  shape.rs + shape/{state,geom,pending,keys,commit,overlay,optionbar,tests}.rs
  text.rs + text/optionbar.rs
  eyedropper.rs   fill.rs   hand.rs

Engine (later):
  crates/engine/src/features/
    arc.rs, gradient.rs, pen.rs, shape.rs
```

Each façade owns the tool implementation AND its option-bar block, so one feature
is readable in one place. `tools::…` and `ui::optionbar::…` are now thin
re-export shims over these.

## Phases

1. Assessment — done
2. Facade — done
3. Incremental move — done
   - all eight tools moved out of `tools/` into `features/`
   - `tools::{arc,gradient,pen,shape,text,fill,hand,eyedropper}.rs` are now
     one-line re-export shims; the old sub-directories are gone
   - `ui/optionbar/*.rs` keep re-exporting `features::*::optionbar::*`
   - internal call sites use `crate::features::*` (no live `crate::tools::<tool>` refs)
4. Cleanup — docs updated; feature shims in `tools/` removed after all
   internal references migrated (compatibility no longer needed)

## Migration rules

- No large file moves in phase 1.
- Backward-compatible re-exports are mandatory.
- New feature code should prefer `features::` paths in incremental phases.
- Do not remove old paths until no internal references remain and tests pass.
- Keep public engine and app API behavior unchanged.

## Ownership

- App feature facades: app crate
- Tool behavior: app/tools + engine raster
- Option-bar UI: app/ui/optionbar
- Shared compatibility shims: module owners for each migrated area
- Review required before deleting compatibility paths