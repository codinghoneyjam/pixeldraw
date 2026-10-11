// <META - FILE SUMMARY - Stage the runtime-only files into build/app for the Tauri bundle>
//
// The editor has no build step, so the desktop shell packages exactly the files
// the boot path loads at runtime and leaves everything else in the repository.
// That keeps tests, tools, asset_work, docs, viewer, schema and reference out of
// the executable.
//
// Staged layout mirrors the repository layout, because index.html resolves its
// stylesheets and scripts relative to itself and app.js fetches the baked glyph
// JSON at "./tools/recipe/text_glyphs.json":
//
//   index.html
//   style_base.css style_polish.css style_juicy.css style_picker.css
//   src/**/*.js
//   tools/recipe/text_glyphs.json
//
// Run: node tools/stage_app.mjs [--out DIR] [--clean]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");

// Files index.html pulls in through href/src, plus the glyph JSON that app.js
// fetches. Kept as an explicit list so a rename that breaks the bundle fails
// here instead of shipping an app with a missing stylesheet.
const ENTRY_FILES = [
  "index.html",
  "style_base.css",
  "style_polish.css",
  "style_juicy.css",
  "style_picker.css",
  "tools/recipe/text_glyphs.json",
];

const OUT_FLAG = process.argv.indexOf("--out");
const OUT_DIR = OUT_FLAG > -1 && process.argv[OUT_FLAG + 1]
  ? path.resolve(process.cwd(), process.argv[OUT_FLAG + 1])
  : path.join(REPO_ROOT, "build", "app");

// <META - ROLE : Copy one file, creating parent directories | L...>
function copyFile(relative) {
  const from = path.join(REPO_ROOT, relative);
  const to = path.join(OUT_DIR, relative);
  if (!fs.existsSync(from)) {
    throw new Error(`stage_app: missing runtime file "${relative}"`);
  }
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(from, to);
  return fs.statSync(to).size;
}

// <META - ROLE : Walk src/ for the .js modules the browser loads | L...>
function listSrcModules() {
  const root = path.join(REPO_ROOT, "src");
  const out = [];
  (function walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const abs = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(abs);
      else if (entry.name.endsWith(".js")) out.push(path.relative(REPO_ROOT, abs).split(path.sep).join("/"));
    }
  })(root);
  return out.sort();
}

function main() {
  if (process.argv.includes("--clean") && fs.existsSync(OUT_DIR)) {
    fs.rmSync(OUT_DIR, { recursive: true, force: true });
  }
  let files = 0;
  let bytes = 0;
  for (const relative of [...ENTRY_FILES, ...listSrcModules()]) {
    bytes += copyFile(relative);
    files++;
  }
  console.log(`stage_app: ${files} files, ${(bytes / 1024).toFixed(0)} KiB -> ${path.relative(REPO_ROOT, OUT_DIR).split(path.sep).join("/")}`);
}

main();
