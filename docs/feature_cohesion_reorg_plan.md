# Feature-Cohesion Reorganization Plan

## Current pain points

- Layer-oriented modules scatter one feature across `tools/`, `ui/optionbar/`, and engine raster code.
- Feature ownership is split between tool state and option-bar UI; discoverability is low.
- Existing pub paths must stay stable during migration; avoid broad renames until proven.
- Some UI feature helpers live in partially wired directories and are not compiled yet.

## Target layout

```
crates/app/src/features/
  mod.rs                  # facades first, no moves
  arc.rs or arc/mod.rs    # future: tools::arc + ui::optionbar::arc
  gradient.rs
  pen.rs
  shape.rs
  ...

Engine (later):
  crates/engine/src/features/
    arc.rs, gradient.rs, pen.rs, shape.rs
```

Feature facades group tool modules, preview helpers, and option-bar UI under one entry point.

## Phases

1. Assessment
   - map feature code and call sites
   - identify public paths and tests
   - record gap between layer modules and intended features
2. Facade
   - add `features` module with aliases/re-exports only
   - keep all old module declarations intact
   - add smoke tests (`features::arc::...`)
3. Incremental move
   - move implementation into feature modules one at a time
   - update call sites through `features::*`
   - leave compatibility re-exports in old locations
4. Cleanup
   - remove empty forwarding modules after all callers migrate
   - update docs and roadmap mappings
   - run full build/test and roadmap checks

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