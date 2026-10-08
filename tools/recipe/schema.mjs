// <META - FILE SUMMARY - Recipe schema detection + command collection shared by the Node
// transpiler and the browser document builder (no fs/DOM imports here).
//
// The draw order and box spelling mirror tidy/tools/gen_pantograph_atlas.py.

// <META - ROLE : Pantograph profile is geometry+colors+slots, not raw commands | L10-12>
export function isPantographProfile(recipe) {
  return Boolean(recipe && recipe.geometry && recipe.colors && Array.isArray(recipe.slots));
}

// <META - ROLE : Surface manifest is an array of id'd surfaces | L15-17>
export function isSurfaceManifest(recipe) {
  return Boolean(recipe && Array.isArray(recipe.surfaces));
}

function pantographRoundedCmd(bboxFlat, radius, fill, outline, width) {
  const [x0, y0, x1, y1] = bboxFlat;
  const cmd = { cmd: "rounded_rect", box: [[x0, y0], [x1, y1]], radius };
  if (fill) cmd.fill = fill;
  if (outline) cmd.outline = outline;
  if (width !== null && width !== undefined) cmd.width = width;
  return cmd;
}

// <META - ROLE : Mirror the python generator's draw order per layer list | L27-55>
export function pantographCommandsForLayers(profile, layers) {
  const geometry = profile.geometry;
  const colors = profile.colors;
  const cmds = [];
  if (layers.includes("outer_frame")) {
    const spec = geometry.outer_frame;
    cmds.push(pantographRoundedCmd(spec.bbox, spec.corner_radius,
      colors.outer_frame_bg_hex, colors.outer_frame_border_hex, spec.border_width));
  }
  if (layers.includes("socket_well")) {
    const spec = geometry.socket_well;
    cmds.push(pantographRoundedCmd(spec.bbox, spec.corner_radius,
      colors.socket_well_bg_hex, null, null));
  }
  const innerByLayer = {
    inner_keycap_normal: ["normal_bbox", "keycap_body_normal_hex", "keycap_border_normal_hex"],
    inner_keycap_pressed: ["pressed_bbox", "keycap_body_pressed_hex", "keycap_border_pressed_hex"],
    inner_keycap_hover: ["normal_bbox", "keycap_body_hover_hex", "keycap_border_hover_hex"],
    inner_keycap_disabled: ["normal_bbox", "keycap_body_disabled_hex", "keycap_border_disabled_hex"],
  };
  for (const [layer, [bboxName, bodyKey, borderKey]] of Object.entries(innerByLayer)) {
    if (layers.includes(layer)) {
      const spec = geometry.inner_keycap;
      cmds.push(pantographRoundedCmd(spec[bboxName], spec.corner_radius,
        colors[bodyKey], colors[borderKey], spec.border_width));
    }
  }
  if (layers.includes("visor_window_normal") || layers.includes("visor_window_pressed")) {
    const pressed = layers.includes("visor_window_pressed");
    const spec = geometry.visor_window;
    const bbox = pressed ? spec.pressed_bbox : spec.normal_bbox;
    cmds.push(pantographRoundedCmd(bbox, spec.corner_radius,
      colors.visor_bg_hex, colors.visor_border_hex, spec.border_width));
  }
  return cmds;
}

// <META - ROLE : One surface's transpiler-dialect commands with bbox->box | L58-77>
export function surfaceCommandsFor(recipe, key) {
  const surface = (recipe.surfaces ?? []).find((s) => s.id === key);
  if (!surface) {
    const known = (recipe.surfaces ?? []).map((s) => s.id).join(", ");
    throw new Error(`[recipe] unknown surface '${key}' (known: ${known})`);
  }
  const commands = (surface.layers ?? []).map((layer) => {
    const cmd = { ...layer, cmd: layer.type ?? layer.cmd };
    if (cmd.bbox && !cmd.box) cmd.box = cmd.bbox;
    return cmd;
  });
  return { surface, commands };
}
