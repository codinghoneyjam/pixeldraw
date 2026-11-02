// <META - FILE SUMMARY - Colour actions: swap FG/BG and reset to black/white>
// Keyboard/menu twin of the FG/BG buttons in color_slots.js. Both only write
// settings: color_events.js repaints the panel and records the new values on
// SETTINGS_CHANGED, so no path needs a handle on the mounted panel.
//
// `session.settings` returns a fresh frozen copy, so a single read is a
// snapshot - the swap is safe even though it writes twice.
const PRIMARY = "primaryColor";
const SECONDARY = "secondaryColor";

// <META - ROLE : Exchange the two slots | L14-19>
export function colorSwap(session) {
  const s = session.settings;
  session.setSetting(PRIMARY, s[SECONDARY]);
  session.setSetting(SECONDARY, s[PRIMARY]);
}

// <META - ROLE : Back to the defaults: black foreground, white background | L21-25>
export function colorReset(session) {
  session.setSetting(PRIMARY, "#000000");
  session.setSetting(SECONDARY, "#ffffff");
}
