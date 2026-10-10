// Phase 0.3 — oracle freeze manifest. Run: node tools/oracle_manifest.mjs [--check]
//
// `src/`, `tests/` and `reference/` become READ-ONLY for the duration of the
// native port (docs/native_plan.md §6 Phase 0.3). They are the oracle: the Rust
// engine must reproduce them, so they must not move under us mid-port.
//
//   node tools/oracle_manifest.mjs          # write reference/oracle-manifest.json
//   node tools/oracle_manifest.mjs --check  # fail if any file changed since
//
// Unlike a git tag this survives an uncommitted tree, and unlike a git diff it
// says exactly which file drifted.

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

const ROOTS = ["src", "tests", "reference"];
// Deliberately outside the frozen roots: the manifest describes them, so it
// cannot be one of them or it would always report itself as added.
const OUT = "tools/oracle-manifest.json";

// <META - ROLE : every file under the frozen roots, sorted, POSIX-separated | L18-30>
function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "__pycache__") continue;
    const p = join(dir, entry.name);
    if (entry.isDirectory()) walk(p, out);
    else if (statSync(p).isFile()) out.push(p);
  }
  return out;
}

const files = [];
for (const root of ROOTS) {
  if (!existsSync(root)) continue;
  walk(root, files);
}
files.sort();

const manifest = {
  frozen_at: new Date().toISOString(),
  roots: ROOTS,
  files: {},
};
for (const f of files) {
  const rel = relative(process.cwd(), f).split(sep).join("/");
  manifest.files[rel] = createHash("sha256").update(readFileSync(f)).digest("hex");
}
manifest.digest = createHash("sha256")
  .update(Object.entries(manifest.files).map(([k, v]) => `${k} ${v}`).join("\n"))
  .digest("hex");

if (process.argv.includes("--check")) {
  let before = null;
  try {
    before = JSON.parse(readFileSync(OUT, "utf8"));
  } catch {
    console.error(`oracle manifest missing — run \`node tools/oracle_manifest.mjs\` first`);
    process.exit(2);
  }
  const drifted = [];
  const added = [];
  const removed = [];
  for (const [k, v] of Object.entries(manifest.files)) {
    if (!(k in before.files)) added.push(k);
    else if (before.files[k] !== v) drifted.push(k);
  }
  for (const k of Object.keys(before.files)) if (!(k in manifest.files)) removed.push(k);

  const n = Object.keys(manifest.files).length;
  if (drifted.length === 0 && added.length === 0 && removed.length === 0) {
    console.log(`oracle intact: ${n} files, digest ${manifest.digest.slice(0, 16)}`);
    process.exit(0);
  }
  for (const k of drifted) console.error(`DRIFTED  ${k}`);
  for (const k of added) console.error(`ADDED    ${k}`);
  for (const k of removed) console.error(`REMOVED  ${k}`);
  console.error(
    `\noracle moved: ${driftCount(drifted, added, removed)} change(s) against ${OUT}.\n` +
      `If the port is finished, that is expected — otherwise the Rust parity target\n` +
      `just changed underneath you.`,
  );
  process.exit(1);
}

writeFileSync(OUT, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(
  `froze ${Object.keys(manifest.files).length} files over [${ROOTS.join(", ")}]`,
);
console.log(`digest ${manifest.digest}`);
console.log(`wrote ${OUT}`);

function driftCount(...groups) {
  return groups.reduce((n, g) => n + g.length, 0);
}
