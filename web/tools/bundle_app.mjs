// <META - FILE SUMMARY - Bundle the staged ESM graph into one inline classic script>
//
// Why this exists: inside the Tauri desktop shell the page is served from the
// http://tauri.localhost custom protocol. `fetch()` of a .js file works there
// (200 text/javascript) but Chromium refuses every ES module script load on that
// origin, so the editor never executes a single module. The browser entry
// (`python run.py`) keeps loading the modules directly - this script only
// rewrites the staged copy that goes into the exe.
//
// The graph is bundler-friendly by contract (docs/contract.md §5): relative ESM
// imports only, no `export default`, no dynamic `import()`, no `import.meta`, no
// `export *`. So this is a topological concatenation, not a general bundler.
//
// Run: node tools/bundle_app.mjs [--dir build/app]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");

const FLAG = process.argv.indexOf("--dir");
const DIR = FLAG > -1 && process.argv[FLAG + 1]
  ? path.resolve(REPO_ROOT, process.argv[FLAG + 1])
  : path.join(REPO_ROOT, "build", "app");

const ENTRY = "src/ui/app.js";

// import { a, b as c } from "./x.js"  /  import X from "./x.js" (contract forbids
// the latter, but recognise it so we can fail loudly instead of silently).
const IMPORT_RE = /^[ \t]*import\s+(?:\{([^}]*)\}|([A-Za-z_$][\w$]*)\s*,\s*\{([^}]*)\}|([A-Za-z_$][\w$]*))\s+from\s+["']([^"']+)["'][ \t]*;?[ \t]*$/;
const EXPORT_FROM_RE = /^[ \t]*export\s+\{([^}]*)\}\s+from\s+["']([^"']+)["'][ \t]*;?[ \t]*$/;
const EXPORT_LIST_RE = /^[ \t]*export\s+\{([^}]*)\}[ \t]*;?[ \t]*$/;
const EXPORT_DECL_RE = /^([ \t]*)export\s+(async\s+function|function|const|let|var|class)\s+([A-Za-z_$][\w$]*)/;

// <META - ROLE : Resolve a relative specifier inside DIR | L...>
function resolveSpec(fromFile, spec) {
  if (!spec.startsWith("./") && !spec.startsWith("../")) {
    throw new Error(`bundle_app: bare specifier "${spec}" in ${fromFile}`);
  }
  return path.relative(DIR, path.resolve(path.dirname(path.join(DIR, fromFile)), spec)).split(path.sep).join("/");
}

// <META - ROLE : Split a `a, b as c` clause list into [local, imported] pairs | L...>
function parseClauses(text) {
  return text
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((clause) => {
      const m = /^([A-Za-z_$][\w$]*)(?:\s+as\s+([A-Za-z_$][\w$]*))?$/.exec(clause);
      if (!m) throw new Error(`bundle_app: unparsable clause "${clause}"`);
      return { local: m[2] || m[1], imported: m[1] };
    });
}

// <META - ROLE : Topological order of the module graph reachable from ENTRY | L...>
function topologicalOrder(entry) {
  const order = [];
  const state = new Map();
  function visit(file) {
    if (state.get(file) === "done") return;
    if (state.get(file) === "visiting") throw new Error(`bundle_app: cycle at ${file}`);
    state.set(file, "visiting");
    const text = fs.readFileSync(path.join(DIR, file), "utf8");
    for (const line of text.split("\n")) {
      const m = IMPORT_RE.exec(line) || EXPORT_FROM_RE.exec(line);
      if (m) {
        const spec = m[4] !== undefined && m[5] !== undefined ? m[5] : m[m.length - 1];
        visit(resolveSpec(file, spec));
      }
    }
    state.set(file, "done");
    order.push(file);
  }
  visit(entry);
  return order;
}

// <META - ROLE : Rewrite one module into an IIFE assigned to __mN | L...>
function wrapModule(file, index) {
  const varName = `__m${index}`;
  const text = fs.readFileSync(path.join(DIR, file), "utf8");
  const importBindings = new Map(); // local -> { from, imported }
  const exportNames = new Set();

  const kept = [];
  for (const line of text.split("\n")) {
    const imp = IMPORT_RE.exec(line);
    if (imp) {
      if (imp[2] || imp[4]) throw new Error(`bundle_app: default import in ${file} (contract §5 forbids it)`);
      for (const { local, imported } of parseClauses(imp[1])) importBindings.set(local, { from: imp[5], imported });
      continue;
    }
    const reExport = EXPORT_FROM_RE.exec(line);
    if (reExport) {
      for (const { local, imported } of parseClauses(reExport[1])) {
        importBindings.set(local, { from: reExport[2], imported });
        exportNames.add(imported === local ? local : imported);
      }
      continue;
    }
    const list = EXPORT_LIST_RE.exec(line);
    if (list) {
      for (const { local, imported } of parseClauses(list[1])) exportNames.add(imported);
      continue;
    }
    kept.push(line);
  }

  let body = kept.join("\n");
  const decls = [];
  for (const line of kept) {
    const m = EXPORT_DECL_RE.exec(line);
    if (m) {
      exportNames.add(m[3]);
      decls.push(m[3]);
    }
  }
  body = body.replace(/^([ \t]*)export\s+(async\s+function|function|const|let|var|class)\s/gm, "$1$2 ");

  const leftovers = body.split("\n").filter((l) => /^[ \t]*(import|export)\b/.test(l));
  if (leftovers.length) throw new Error(`bundle_app: ${file} still has module syntax:\n${leftovers.slice(0, 3).join("\n")}`);

  // Import bindings become destructured constants at the top of the IIFE. Every
  // dependency is already assigned by then, because the order is topological.
  const bySource = new Map();
  for (const [local, binding] of importBindings) {
    if (!bySource.has(binding.from)) bySource.set(binding.from, []);
    bySource.get(binding.from).push(binding.imported === local ? local : `${binding.imported}: ${local}`);
  }
  const binds = [...bySource].map(([spec, names]) => {
    const target = `__m${indexOfFile(spec)}`;
    return `const { ${names.join(", ")} } = ${target};`;
  });

  const names = [...exportNames];
  return [
    `var ${varName} = (function () {`,
    ...binds,
    body,
    `return { ${names.join(", ")} };`,
    `})();`,
    "",
  ].join("\n");

  function indexOfFile(spec) {
    const rel = resolveSpec(file, spec);
    const idx = order.indexOf(rel);
    if (idx < 0) throw new Error(`bundle_app: ${file} imports unvisited ${rel}`);
    return idx;
  }
}

// <META - ROLE : Point index.html at the inline bundle instead of the module graph | L...>
function rewriteHtml(html) {
  const tag = /[ \t]*<script type="module" src="\.\/src\/ui\/app\.js"><\/script>\r?\n?/;
  if (!tag.test(html)) throw new Error("bundle_app: module script tag not found in index.html");
  return { html: html.replace(tag, '  <script src="./app.bundle.js"></script>\n') };
}

function main() {
  const order = topologicalOrder(ENTRY);
  const indexOf = new Map(order.map((f, i) => [f, i]));
  const parts = order.map((f, i) => wrapModule(f, i));
  const bundle = [
    "// Generated by tools/bundle_app.mjs - do not edit.",
    "// The ESM graph inlined as one classic script for the Tauri shell, where",
    "// http://tauri.localhost refuses ES module script loads.",
    "(function () {",
    '"use strict";',
    ...parts,
    "})();",
    "",
  ].join("\n");

  fs.writeFileSync(path.join(DIR, "app.bundle.js"), bundle);

  const htmlPath = path.join(DIR, "index.html");
  const html = fs.readFileSync(htmlPath, "utf8");
  fs.writeFileSync(htmlPath, rewriteHtml(html).html);

  console.log(`bundle_app: ${order.length} modules -> app.bundle.js (${(bundle.length / 1024).toFixed(0)} KiB)`);
}

main();
