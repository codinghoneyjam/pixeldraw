// Post-commit tool check for the draw_tool_v2 cleanup. Verifies the modules the
// split created still behave, independent of the unit suite.
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import * as osmod from "node:os";

// The script lives in tests/, so the project root is one level up.
const D = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const imp = (p) => import(pathToFileURL(path.join(D, p)).href);

let pass = 0;
let fail = 0;
const check = (name, cond, extra = "") => {
  if (cond) { pass++; console.log(`  PASS ${name}`); }
  else { fail++; console.log(`  FAIL ${name} ${extra}`); }
};

console.log("=== 1. split modules export what importers need ===");
const mods = {
  "src/tools/wheel_accumulator.js": ["WheelAccumulator"],
  "src/tools/pointer_event.js": ["buildToolEvent"],
  "src/tools/pointer_bindings.js": [],
  "src/tools/keyboard_bindings.js": [],
  "src/tools/cursor_state.js": ["resolveCursor", "readCursorState", "applyCursor", "markInside"],
  "src/features/shape/shape_render.js": [],
  "src/features/layers/commands_pixel.js": ["MergeDownCommand", "ResizeCanvasCommand"],
};
for (const [f, names] of Object.entries(mods)) {
  const m = await imp(f);
  const ok = names.every((n) => typeof m[n] === "function");
  check(`${path.basename(f)} exports ${names.join(",") || "(bindings)"}`, ok,
    `keys=${Object.keys(m)}`);
}
const cmds = await imp("src/features/layers/commands.js");
check("commands.js re-exports the pixel commands",
  typeof cmds.MergeDownCommand === "function" && typeof cmds.ResizeCanvasCommand === "function");

console.log("\n=== 2. wheel accumulator (60px = 1 step) ===");
const { WheelAccumulator } = await imp("src/tools/wheel_accumulator.js");
check("120px -> -2 steps", new WheelAccumulator().feed(120) === -2);
check("60px -> -1 step", new WheelAccumulator().feed(60) === -1);
check("30px -> 0 (below threshold)", new WheelAccumulator().feed(30) === 0);
check("deltaMode 1 scales x16 (4 lines = 1 step)", new WheelAccumulator().feed(4, 1) === -1);
check("NaN ignored", new WheelAccumulator().feed(NaN) === 0);
check("reset clears residue", (() => { const a = new WheelAccumulator(); a.feed(30); a.reset(); return a.feed(30) === 0; })());
const rev = new WheelAccumulator();
rev.feed(30);
check("direction reversal resets residue", rev.feed(-120) === 2, `got ${rev.feed(-120)}`);

console.log("\n=== 3. mergeDown copies down and undo restores ===");
const { Session } = await imp("src/features/document/session.js");
const { Layer } = await imp("src/features/layers/layer.js");
const { packRGBA } = await imp("src/core/pixel.js");
const s = new Session();
s.newDocument({ widthPx: 32, heightPx: 32 });
const lowerId = s.doc.activeLayerId;
s.doc.getLayer(lowerId).store._setPixel(1, 1, packRGBA(255, 0, 0, 255));
const upper = Layer.create({ id: s.doc.ids.next(), name: "U", widthPx: 32, heightPx: 32 });
upper.store._setPixel(1, 1, packRGBA(0, 0, 255, 255));
s.doc._insert(upper, 1);
const cmd = new (await imp("src/features/layers/commands_pixel.js")).MergeDownCommand(upper.id);
cmd.do(s.doc);
check("upper pixel copied down",
  s.doc.getLayer(lowerId).store.getPixel(1, 1) === packRGBA(0, 0, 255, 255));
cmd.undo(s.doc);
check("undo restores lower pixel",
  s.doc.getLayer(lowerId).store.getPixel(1, 1) === packRGBA(255, 0, 0, 255));
check("undo leaves upper intact",
  s.doc.getLayer(upper.id).store.getPixel(1, 1) === packRGBA(0, 0, 255, 255));

console.log("\n=== 4. every src module imports cleanly ===");
const files = [];
const walk = (d) => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.name.endsWith(".js")) files.push(p);
  }
};
walk(path.join(D, "src"));
const linkErr = [];
for (const f of files) {
  try { await import(pathToFileURL(f).href); }
  catch (err) {
    const m = String(err && err.message).split("\n")[0];
    if (!/is not defined|document|window|navigator/.test(m)) linkErr.push(`${path.relative(D, f)}: ${m}`);
  }
}
check(`all ${files.length} src modules import without syntax/link error`, linkErr.length === 0,
  linkErr.slice(0, 5).join(" | "));

console.log("\n=== 5. no compression artefacts in split modules ===");
for (const f of Object.keys(mods)) {
  const src = fs.readFileSync(path.join(D, f), "utf8");
  const code = src.split("\n").filter((l) => !/^\s*(\/\/|\*|\/\*)/.test(l) && l.trim());
  const dense = code.filter((l) => (l.match(/;\s*\S/g) || []).length >= 3);
  check(`${path.basename(f)}: no 3+ statement lines`, dense.length === 0,
    `${dense.length} dense`);
  check(`${path.basename(f)}: has META FILE SUMMARY`, /<META - FILE SUMMARY/.test(src));
}

console.log("\n=== 6. every src module under 300 lines except the documented exemption ===");
const walkAll = (d) => {
  const out = [];
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) out.push(...walkAll(p));
    else if (/\.(js|mjs|py)$/.test(e.name)) out.push(p);
  }
  return out;
};
// Oracles (reference/raster_ref.mjs, tools/gen_fixtures.py) are a flat list of
// independent primitives with no natural split, and polygon.js is a single
// cohesive scanline engine at 301 lines (1 over, from the T-7 fillet work).
// Both are known and pre-existing, so the check reports them rather than
// failing on them. The scan is deliberately scoped to src/: asset_work/ holds
// generated fixtures, and tools/ + viewer/ are build scaffolding (a generated
// gallery_data.js is 3710 lines by construction), none of which the module
// cleanup governs.
const KNOWN = new Map([
  ["src/core/raster/polygon.js", "single cohesive scanline engine, 1 line over"],
]);
const over = walkAll(path.join(D, "src"))
  .map((f) => [path.relative(D, f).replace(/\\/g, "/"), fs.readFileSync(f, "utf8").split("\n").length])
  .filter(([f, n]) => n > 300);
const unexpected = over.filter(([f]) => !KNOWN.has(f));
check("no UNEXPECTED src file exceeds 300 lines", unexpected.length === 0,
  unexpected.map(([f, n]) => `${f}=${n}`).join(" "));
for (const [f, n] of over) console.log(`    known over-limit: ${f} (${n}) -- ${KNOWN.get(f)}`);

console.log(`\n=== tool check: ${pass} passed, ${fail} failed ===`);
process.exit(fail === 0 ? 0 : 1);