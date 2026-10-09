// <META - FILE SUMMARY - Brush action: step the pen size by a delta>
// `[` / `]` and the wheel shortcuts land here. The clamp mirrors the setting
// validator, so a rejected write never reaches the session.
import { PEN_MIN, PEN_MAX } from "../../core/constants.js";

// <META - ROLE : Add delta to penSize, clamped to the legal range | L7-11>
export function brushStep(session, delta) {
  const cur = session.settings.penSize;
  session.setSetting("penSize", Math.max(PEN_MIN, Math.min(PEN_MAX, cur + delta)));
}
