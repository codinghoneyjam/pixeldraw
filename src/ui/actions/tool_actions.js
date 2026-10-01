// <META - FILE SUMMARY - Tool actions: brush step, color swap, color reset>

// <META - ROLE : Brush size step and color swap/reset actions | L1-20>
export function brushStep(session, delta) {
  const cur = session.settings.penSize;
  session.setSetting("penSize", Math.max(1, Math.min(64, cur + delta)));
}

export function colorSwap(session) {
  const s = session.settings;
  session.setSetting("primaryColor", s.secondaryColor);
  session.setSetting("secondaryColor", s.primaryColor);
}

export function colorReset(session) {
  session.setSetting("primaryColor", "#000000");
  session.setSetting("secondaryColor", "#ffffff");
}
