// <META - FILE SUMMARY - Build the 34-asset layer gallery: audit + bake + emit viewer data>
//
// Pipeline for every ACTIVE PNG (asset_work/tidy/numbered/NN_*.png, the 34 files
// the game actually loads):
//   1. resolve the parity-gate case that owns it (exact id, else SHA-256 of the
//      case's legacy PNG -- that is how the kb_* / *_sheet alias pairs resolve);
//   2. read the layer units extract_layers.mjs produced for that case's recipe;
//   3. bake the transpiler render  -> gallery/render/U<NN>.png   ("레이어로 만든 png")
//   4. reassemble the layer PNGs   -> gallery/from_layers/U<NN>.png (unit pixels only)
//   5. diff both against the legacy PNG and bake gallery/diff/U<NN>.png;
//   6. emit viewer/js/gallery_data.js, fold the result into progress.json, and
//      print the coverage table.
//
// Run: node tools/build_layer_gallery.mjs [--out DIR]

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "./recipe/transpile.mjs";
import { decodePng, encodePng } from "../src/io/png.js";
import { CASES, resolveCasePath } from "./recipe_cases.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const NUMBERED_DIR = path.join(REPO_ROOT, "asset_work", "tidy", "numbered");
const LAYERS_DIR = path.join(REPO_ROOT, "asset_work", "layers");
const LAYER_INDEX = path.join(LAYERS_DIR, "index.json");

const MATCH_THRESHOLD = 0.99;
const ELEMENT_DELTA_CHANNEL_THRESHOLD = 128;

const sha256 = (buf) => crypto.createHash("sha256").update(buf).digest("hex");

// Numbered-file group -> viewer category, in ACTIVE_ASSET_LIST.md order.
const GROUPS = [
  { id: "entity", from: 11, to: 25 },
  { id: "world", from: 26, to: 29 },
  { id: "keycap", from: 30, to: 33 },
  { id: "ui", from: 34, to: 39 },
  { id: "title", from: 40, to: 44 },
];
const groupOf = (no) => GROUPS.find((g) => no >= g.from && no <= g.to)?.id ?? "misc";

// Display names. Key = numbered basename without the NN_ prefix.
const NAMES = {
  kb_z_amber_t1: "키봇 Z-앰버 T1 (C-001)",
  kb_f_crimson_t2: "키봇 F-크림슨 T2 (T-002)",
  kb_q_cobalt_t3: "키봇 Q-코발트 T3 (R-003)",
  kb_b_obsidian_t4: "키봇 B-오브시디안 T4 (B-001 보스)",
  keycap_debris: "키캡 파편",
  mouse_hero_sheet: "마우스 히어로 시트",
  hero_classic_gem_sheet: "히어로 클래식 젬 시트",
  hero_trackball_cyber_sheet: "히어로 트랙볼 사이버 시트",
  shockwave_ring: "쇼크웨이브 링",
  sword_albedo: "검 알베도",
  sword_mask: "검 마스크",
  bow_albedo: "활 알베도",
  bow_mask: "활 마스크",
  spear_albedo: "창 알베도",
  spear_mask: "창 마스크",
  room_tileset: "방 타일셋",
  lobby_tileset: "로비 타일셋",
  portal_keycap_unpressed: "포탈 키캡 (해제)",
  portal_keycap_pressed: "포탈 키캡 (눌림)",
  keycap_sheet: "키캡 시트",
  fused_keycap_9slice: "퓨즈드 키캡 9슬라이스",
  fused_keycap_hover_9slice: "퓨즈드 키캡 호버 9슬라이스",
  fused_keycap_pressed_9slice: "퓨즈드 키캡 눌림 9슬라이스",
  pantograph_sheet: "판토그래프 시트",
  hud_icons_128_atlas: "HUD 아이콘 128 아틀라스",
  keycap_screen_9slice: "키캡 스크린 9슬라이스",
  scanline_tile_fine: "스캔라인 타일 (파인)",
  keyboard_plate_block: "키보드 플레이트 블록",
  floating_visor_window: "플로팅 바이저 윈도우",
  title_weapons_atlas: "타이틀 무기 아틀라스",
  title_diorama_sword_body: "타이틀 디오라마 검 몸체",
  title_logo_text_our: "타이틀 로고 텍스트 OUR",
  title_logo_text_or: "타이틀 로고 텍스트 OR",
  title_logo_text_d: "타이틀 로고 텍스트 D",
};

// Player sheets are one flattened layer each; the per-concept palettes below are
// the actual body/visor/face decompositions. Surfaced as a related-layer hint.
const RELATED_LAYER_SETS = {
  mouse_hero_sheet: ["player_mouse__pal", "base_chassis_gem__pal", "emoticons_cyber_neon__pal"],
  hero_classic_gem_sheet: ["player_mouse__pal", "base_chassis_gem__pal", "emoticons_cyber_neon__pal"],
  hero_trackball_cyber_sheet: ["base_chassis_trackball__pal", "emoticons_cyber_neon__pal"],
};

// <META - ROLE : Compare two decoded RGBA buffers with the parity gate's rules | L99-147>
function compareRgba(legacy, candidate) {
  if (legacy.width !== candidate.width || legacy.height !== candidate.height) {
    return { sizeMismatch: true, matchFraction: 0, maxDelta: 0, classification: "SIZE_MISMATCH" };
  }
  const total = legacy.width * legacy.height;
  const a = legacy.rgba;
  const b = candidate.rgba;
  let matches = 0;
  let transparentResidue = 0;
  let maxDelta = 0;
  let elementDelta = false;
  const histogram = { "1-31": 0, "32-63": 0, "64-127": 0, "128-255": 0 };
  for (let i = 0; i < total * 4; i += 4) {
    const dR = Math.abs(a[i] - b[i]);
    const dG = Math.abs(a[i + 1] - b[i + 1]);
    const dB = Math.abs(a[i + 2] - b[i + 2]);
    const dA = Math.abs(a[i + 3] - b[i + 3]);
    const pixelMax = Math.max(dR, dG, dB, dA);
    if (pixelMax === 0) {
      matches++;
      continue;
    }
    // RGB under alpha 0 is undefined in PNG; PIL legacy bakers leak colour there.
    if (dA === 0 && a[i + 3] === 0) {
      transparentResidue++;
      matches++;
      continue;
    }
    if (pixelMax > maxDelta) maxDelta = pixelMax;
    if (pixelMax >= ELEMENT_DELTA_CHANNEL_THRESHOLD) elementDelta = true;
    if (pixelMax <= 31) histogram["1-31"]++;
    else if (pixelMax <= 63) histogram["32-63"]++;
    else if (pixelMax <= 127) histogram["64-127"]++;
    else histogram["128-255"]++;
  }
  return {
    sizeMismatch: false,
    total,
    matches,
    transparentResidue,
    differing: total - matches,
    matchFraction: matches / total,
    strictMatchFraction: (matches - transparentResidue) / total,
    maxDelta,
    classification: elementDelta ? "ELEMENT_DELTA" : "RENDER_NOISE",
    histogram,
  };
}

// <META - ROLE : Composite unit images into one strip, source-over per pixel | L149-177>
function composite(units) {
  const first = units[0];
  const w = first.rect[2] * units.length;
  const h = first.rect[3];
  const out = new Uint8Array(w * h * 4);
  for (const u of units) {
    const cw = u.rect[2];
    const ch = u.rect[3];
    const dx = u.dx;
    for (let y = 0; y < ch; y++) {
      for (let x = 0; x < cw; x++) {
        const src = (y * u.width + x) * 4;
        const sa = u.rgba[src + 3] / 255;
        if (sa === 0) continue;
        const dst = ((y + u.dy) * w + (x + dx)) * 4;
        const da = out[dst + 3] / 255;
        const oa = sa + da * (1 - sa);
        if (oa === 0) continue;
        for (let c = 0; c < 3; c++) {
          out[dst + c] = Math.round((u.rgba[src + c] * sa + out[dst + c] * da * (1 - sa)) / oa);
        }
        out[dst + 3] = Math.round(oa * 255);
      }
    }
  }
  return { width: w, height: h, rgba: out };
}

// <META - ROLE : Paint a match/diff map: matching pixels grey, differing red | L179-200>
function diffImage(legacy, candidate) {
  const out = new Uint8ClampedArray(legacy.width * legacy.height * 4);
  for (let i = 0; i < legacy.width * legacy.height; i++) {
    const o = i * 4;
    let maxDelta = 0;
    for (let c = 0; c < 4; c++) {
      maxDelta = Math.max(maxDelta, Math.abs(legacy.rgba[o + c] - candidate.rgba[o + c]));
    }
    if (maxDelta === 0 || (maxDelta === legacy.rgba[o + 3] && legacy.rgba[o + 3] === 0)) {
      out[o] = 148;
      out[o + 1] = 163;
      out[o + 2] = 184;
      out[o + 3] = 40;
    } else {
      out[o] = 248;
      out[o + 1] = 40;
      out[o + 2] = 40;
      out[o + 3] = 255;
    }
  }
  return out;
}

async function bakeDiff(file, legacy, candidate) {
  const png = await encodePng(diffImage(legacy, candidate), legacy.width, legacy.height);
  fs.writeFileSync(file, Buffer.from(png));
}

// <META - ROLE : Bind each numbered asset to the parity case that produced it | L208-238>
function resolveCases() {
  const byLegacySha = new Map();
  for (const c of CASES) {
    const key = sha256(fs.readFileSync(resolveCasePath(c.legacy)));
    if (!byLegacySha.has(key)) byLegacySha.set(key, []);
    byLegacySha.get(key).push(c);
  }
  const numbered = fs
    .readdirSync(NUMBERED_DIR)
    .filter((f) => f.endsWith(".png"))
    .sort((a, b) => Number(a.slice(0, 2)) - Number(b.slice(0, 2)));
  return numbered.map((file) => {
    const base = file.slice(3, -4);
    const digest = sha256(fs.readFileSync(path.join(NUMBERED_DIR, file)));
    const byId = CASES.find((c) => c.id === base);
    const bySha = byLegacySha.get(digest) ?? [];
    return {
      no: Number(file.slice(0, 2)),
      file,
      base,
      digest,
      case: byId ?? bySha[0],
      resolvedBy: byId ? "case-id" : bySha.length ? "sha256" : "NONE",
      aliases: bySha.map((c) => c.id),
    };
  });
}

function unitListFor(layerIndex, recipe, hasPalette) {
  const key = `${recipe}|${hasPalette ? "pal" : "default"}`;
  return layerIndex.find((r) => `${r.recipe}|${r.slug.endsWith("__pal") ? "pal" : "default"}` === key) ?? null;
}

// <META - ROLE : Fold the gate result into asset_work/progress.json (project checklist) | L246-284>
function recordProgress(coverage) {
  const progressPath = path.join(REPO_ROOT, "asset_work", "progress.json");
  if (!fs.existsSync(progressPath)) return;
  const progress = JSON.parse(fs.readFileSync(progressPath, "utf-8"));
  progress.layer_gallery = {
    status: coverage.problems.length === 0 ? "done" : "failed",
    tool: "tools/build_layer_gallery.mjs",
    site: "viewer/index.html",
    data: "asset_work/layers/gallery/data.json",
    view_data: "viewer/js/gallery_data.js (GENERATED)",
    check_command: "node tools/build_layer_gallery.mjs",
    scope: "the 34 ACTIVE PNGs (asset_work/tidy/numbered 11-44), 3 items each",
    triples: "layer units (png + _drawtool.json) / png rendered from that layer set / legacy numbered png",
    assets: coverage.totals.assets,
    with_layers: coverage.totals.withLayers,
    parity_pass: coverage.totals.parityPass,
    min_parity: Number(coverage.totals.minParity.toFixed(6)),
    layer_units_referenced_by_34: coverage.totals.distinctUnits,
    layer_set_slugs_referenced: new Set(coverage.assets.map((a) => a.layerSet?.slug)).size,
    multi_unit_assets: coverage.totals.multiUnitAssets,
    threshold: coverage.threshold,
    problems: coverage.problems,
    note:
      "each asset resolves to its parity case by exact id, else by sha256 identity with that case's " +
      "legacy PNG (this is how the kb_*_t* / enemy_keybot_*_sheet alias pairs resolve). parity_pass is " +
      "measured on the reassembled layer PNGs (no recipe, no transpiler); the transpiler render scores a " +
      "few counts higher on the 4 enemy atlases because only that path replays the legacy self-paste " +
      "alpha law (src/core/raster/atlas.js).",
  };
  const cmd = "node tools/build_layer_gallery.mjs";
  progress.check_commands = [...new Set([...(progress.check_commands ?? []), cmd])];
  progress.gates = {
    ...progress.gates,
    layer_gallery: `${coverage.totals.parityPass}/${coverage.totals.assets} assets, min parity ${coverage.totals.minParity.toFixed(6)}`,
  };
  progress.last_checked = new Date().toISOString().slice(0, 10);
  fs.writeFileSync(progressPath, `${JSON.stringify(progress, null, 2)}\n`, "utf-8");
}

// <META - ROLE : Build one gallery record per active asset and write every output | L286-470>
async function main() {
  const args = process.argv.slice(2);
  let outDir = path.join(LAYERS_DIR, "gallery");
  for (let i = 0; i < args.length; i++) if (args[i] === "--out") outDir = path.resolve(args[++i]);

  const layerIndex = JSON.parse(fs.readFileSync(LAYER_INDEX, "utf-8")).recipes;
  const assets = resolveCases();
  for (const sub of ["render", "from_layers", "diff"]) {
    fs.mkdirSync(path.join(outDir, sub), { recursive: true });
  }

  console.log(`layer_gallery: ${assets.length} active assets -> ${outDir}\n`);
  const rows = [];
  const problems = [];

  for (const asset of assets) {
    const caseInfo = asset.case;
    const legacyPath = path.join(NUMBERED_DIR, asset.file);
    const legacy = await decodePng(fs.readFileSync(legacyPath));
    const record = {
      no: asset.no,
      key: asset.base,
      name: NAMES[asset.base] ?? asset.base,
      group: groupOf(asset.no),
      size: { w: legacy.width, h: legacy.height },
      sha: asset.digest,
      legacy: {
        path: `../asset_work/tidy/numbered/${asset.file}`,
        bytes: fs.statSync(legacyPath).size,
      },
      recipe: null,
      layerSet: null,
      render: null,
      fromLayers: null,
      notes: [],
    };

    if (!caseInfo) {
      record.notes.push("레거시 대응 케이스 없음 (SHA 불일치)");
      problems.push(`U${asset.no}: no parity case`);
      rows.push(record);
      continue;
    }

    const recipeRel = caseInfo.recipe;
    const recipePath = resolveCasePath(recipeRel);
    const slugEntry = unitListFor(layerIndex, recipeRel, Boolean(caseInfo.palette));
    const units = (slugEntry?.units ?? []).filter((u) => u.png);
    const unitImages = new Map();
    for (const u of units) {
      unitImages.set(u.unit, await decodePng(fs.readFileSync(path.join(LAYERS_DIR, u.png))));
    }
    // A case that names a layer owns exactly that unit (albedo vs mask, one
    // nine-patch variant, one surface). A case that names no layer owns every
    // unit sized like the asset: the 16 atlas slots, or the single flat sheet.
    const named = caseInfo.layer ? units.filter((u) => u.unit === caseInfo.layer) : [];
    const sized = units.filter((u) => {
      const img = unitImages.get(u.unit);
      return img.width === legacy.width && img.height === legacy.height;
    });
    const selected = named.length ? named : sized.length ? sized : units;

    record.recipe = {
      path: `../asset_work/target/${recipePath.slice(REPO_ROOT.length + 1).replace(/\\/g, "/")}`,
      rel: recipeRel,
      caseId: caseInfo.id,
      layerKey: caseInfo.layer,
      paletteOverride: Boolean(caseInfo.palette),
      resolvedBy: asset.resolvedBy,
      aliases: asset.aliases,
    };
    record.layerSet = {
      slug: slugEntry?.slug ?? null,
      unitCount: units.length,
      selectedCount: selected.length,
      units: units.map((u) => ({
        unit: u.unit,
        png: `../asset_work/layers/${u.png.replace(/\\/g, "/")}`,
        doc: `../asset_work/layers/${u.doc.replace(/\\/g, "/")}`,
        used: selected.some((s) => s.unit === u.unit),
      })),
    };
    for (const u of slugEntry?.units ?? []) {
      if (u.error) record.notes.push(`레이어 ${u.unit} 렌더 실패: ${u.error}`);
    }
    if (asset.aliases.length > 1) {
      record.notes.push(`레거시 별칭 ${asset.aliases.join(", ")} 와 바이트 동일 -> 동일 레이어 세트`);
    }
    for (const s of RELATED_LAYER_SETS[asset.base] ?? []) {
      record.notes.push(`관련 레이어 세트: ${s}`);
    }
    if (selected.length === 0) {
      problems.push(`U${asset.no}: no layer unit for ${recipeRel}#${caseInfo.layer}`);
      record.notes.push("선택된 레이어 유닛 없음");
      rows.push(record);
      continue;
    }

    // 1. transpiler render of the asset's layer set
    const renderPng = path.join(outDir, "render", `U${asset.no}_${asset.base}.png`);
    await transpileAndRender({
      recipePath,
      layerKey: caseInfo.layer ?? null,
      slotId: null,
      paletteOverrides: caseInfo.palette ?? {},
      outputDocPath: renderPng.replace(/\.png$/i, "_drawtool.json"),
      outputPngPath: renderPng,
    });
    const rendered = await decodePng(fs.readFileSync(renderPng));
    const renderCmp = compareRgba(legacy, rendered);
    record.render = {
      path: `../asset_work/layers/gallery/render/U${asset.no}_${asset.base}.png`,
      doc: `../asset_work/layers/gallery/render/U${asset.no}_${asset.base}_drawtool.json`,
      ...renderCmp,
    };

    // 2. reassemble the extracted layer PNGs only (no recipe, no transpiler)
    const recipe = JSON.parse(fs.readFileSync(recipePath, "utf-8"));
    const atlas = recipe.export?.atlas ?? recipe.atlas;
    const slots = atlas?.slots ?? [];
    const cellW = atlas?.cell_width ?? atlas?.cellWidth ?? null;
    const cellH = atlas?.cell_height ?? atlas?.cellHeight ?? null;
    const decodes = selected.map((u) => {
      const img = unitImages.get(u.unit);
      const idx = Math.max(0, slots.findIndex((s) => s.id === u.unit));
      return {
        ...img,
        dx: cellW ? idx * cellW : 0,
        dy: 0,
        rect: [0, 0, cellW ? Math.min(cellW, img.width) : img.width, cellH ? Math.min(cellH, img.height) : img.height],
      };
    });
    const sheet = composite(decodes);
    const fromLayersPath = path.join(outDir, "from_layers", `U${asset.no}_${asset.base}.png`);
    fs.writeFileSync(fromLayersPath, Buffer.from(await encodePng(sheet.rgba, sheet.width, sheet.height)));
    const fromLayers = await decodePng(fs.readFileSync(fromLayersPath));
    const fromCmp = compareRgba(legacy, fromLayers);
    const vsRender = compareRgba(rendered, fromLayers);
    record.fromLayers = {
      path: `../asset_work/layers/gallery/from_layers/U${asset.no}_${asset.base}.png`,
      size: { w: fromLayers.width, h: fromLayers.height },
      units: selected.map((u) => u.unit),
      ...fromCmp,
      matchesRender: vsRender.matchFraction,
      identicalToRender: vsRender.differing === 0 && !vsRender.sizeMismatch,
    };

    // 3. diff bake
    await bakeDiff(path.join(outDir, "diff", `U${asset.no}_${asset.base}.png`), legacy, fromLayers);
    record.diff = `../asset_work/layers/gallery/diff/U${asset.no}_${asset.base}.png`;

    if (fromCmp.matchFraction < MATCH_THRESHOLD) {
      problems.push(`U${asset.no}: layer reassembly parity ${fromCmp.matchFraction.toFixed(6)} < ${MATCH_THRESHOLD}`);
    }
    if (renderCmp.matchFraction < MATCH_THRESHOLD) {
      problems.push(`U${asset.no}: render parity ${renderCmp.matchFraction.toFixed(6)} < ${MATCH_THRESHOLD}`);
    }
    if (record.layerSet.unitCount === 0) {
      problems.push(`U${asset.no}: recipe has no extracted layer units`);
    }
    if (!record.fromLayers.identicalToRender) {
      record.notes.push(
        "레이어 PNG 단순 합성(0.99+)과 트랜스파일러 렌더가 미세하게 다름 -- 레거시 베이커의 " +
          "셀 자기합성 알파 제곱 법칙(src/core/raster/atlas.js)을 렌더 경로만 재현",
      );
    }
    rows.push(record);

    const tag = fromCmp.matchFraction >= MATCH_THRESHOLD ? "PASS" : "FAIL";
    const same = record.fromLayers.identicalToRender ? "==" : `~${vsRender.matchFraction.toFixed(4)}`;
    console.log(
      `[${tag}] U${String(asset.no).padStart(2, "0")} ${asset.base.padEnd(30)} ` +
        `legacy=${legacy.width}x${legacy.height} units=${String(selected.length).padStart(2)} ` +
        `render=${renderCmp.matchFraction.toFixed(6)} layers=${fromCmp.matchFraction.toFixed(6)} ` +
        `render~layers=${same}`,
    );
  }

  const coverage = {
    schema: "draw_tool.layer_gallery",
    version: 1,
    generatedBy: "tools/build_layer_gallery.mjs",
    threshold: MATCH_THRESHOLD,
    totals: {
      assets: rows.length,
      withLayers: rows.filter((r) => (r.layerSet?.unitCount ?? 0) > 0).length,
      parityPass: rows.filter((r) => (r.fromLayers?.matchFraction ?? 0) >= MATCH_THRESHOLD).length,
      referencedUnits: rows.reduce((n, r) => n + (r.layerSet?.unitCount ?? 0), 0),
      distinctUnits: new Set(
        rows.flatMap((r) => (r.layerSet?.units ?? []).map((u) => `${r.layerSet.slug}/${u.unit}`)),
      ).size,
      multiUnitAssets: rows.filter((r) => (r.layerSet?.unitCount ?? 0) > 1).length,
      minParity: rows.reduce((m, r) => Math.min(m, r.fromLayers?.matchFraction ?? 1), 1),
    },
    groups: GROUPS.map((g) => ({ id: g.id, count: rows.filter((r) => r.group === g.id).length })),
    problems,
    assets: rows,
  };
  fs.writeFileSync(path.join(outDir, "data.json"), JSON.stringify(coverage, null, 2), "utf-8");
  fs.writeFileSync(
    path.join(REPO_ROOT, "viewer", "js", "gallery_data.js"),
    `// GENERATED by tools/build_layer_gallery.mjs -- do not edit by hand.\n` +
      `// Regenerate: node tools/build_layer_gallery.mjs\n` +
      `const LAYER_GALLERY = ${JSON.stringify(coverage, null, 2)};\n`,
    "utf-8",
  );

  console.log("--");
  console.log(
    `assets=${coverage.totals.assets}  withLayers=${coverage.totals.withLayers}  ` +
      `parityPass=${coverage.totals.parityPass}  minParity=${coverage.totals.minParity.toFixed(6)}  ` +
      `layerUnits=${coverage.totals.distinctUnits} distinct ` +
      `(${coverage.totals.referencedUnits} referenced, ${coverage.totals.multiUnitAssets} multi-unit assets)`,
  );
  for (const p of problems) console.log(`  PROBLEM ${p}`);
  recordProgress(coverage);
  console.log(coverage.problems.length ? "layer gallery: FAILED" : "layer gallery: OK");
  process.exit(coverage.problems.length ? 1 : 0);
}

await main();