// Shared case manifest for the Node reference parity runner and native Rust port.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TARGET_ROOT = path.resolve(__dirname, "..", "asset_work", "target");
const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "recipe_cases.json"), "utf8"));

export const CASES = manifest.cases;
export const LAYER_CASES = manifest.layer_cases;
export const PALETTE_SPEC = manifest.palette_spec;
export const resolveCasePath = (rel) =>
  path.join(TARGET_ROOT, rel.startsWith("pixeldraw/") ? rel.slice("pixeldraw/".length) : rel);
