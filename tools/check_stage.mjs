// <META - FILE SUMMARY - Verify every static import in the staged app resolves inside it>
//
// build/app is what goes into the exe, so a missing module is a runtime 404 that
// only shows up on a machine with no dev tooling. This walks the staged module
// graph the same way the browser does and fails on the first broken relative
// specifier.
//
// Run: node tools/check_stage.mjs [--dir build/app]
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const FLAG = process.argv.indexOf("--dir");
const DIR = FLAG > -1 && process.argv[FLAG + 1] ? path.resolve(ROOT, process.argv[FLAG + 1]) : path.join(ROOT, "build", "app");

const seen = new Set();
const missing = [];

function check(relFile) {
  const abs = path.join(DIR, relFile);
  if (seen.has(abs)) return;
  seen.add(abs);
  if (!fs.existsSync(abs)) {
    missing.push(relFile);
    return;
  }
  const text = fs.readFileSync(abs, "utf8");
  for (const m of text.matchAll(/^\s*import\s[^;]*?["']([^"']+)["']/gm)) {
    const spec = m[1];
    if (spec.startsWith("./") || spec.startsWith("../")) {
      const resolved = path.relative(DIR, path.resolve(path.dirname(abs), spec)).split(path.sep).join("/");
      check(resolved);
    }
  }
}

for (const css of ["style_base.css", "style_polish.css", "style_juicy.css", "style_picker.css"]) {
  if (!fs.existsSync(path.join(DIR, css))) missing.push(css);
}
check("index.html");
const html = fs.readFileSync(path.join(DIR, "index.html"), "utf8");
for (const m of html.matchAll(/(?:href|src)="\.\/([^"]+)"/g)) {
  if (!fs.existsSync(path.join(DIR, m[1]))) missing.push(m[1]);
}
check("src/ui/app.js");

if (missing.length) {
  console.error(`check_stage: BROKEN -> ${missing.join(", ")}`);
  process.exit(1);
}
console.log(`check_stage: ok (${seen.size} modules reachable, all present)`);
