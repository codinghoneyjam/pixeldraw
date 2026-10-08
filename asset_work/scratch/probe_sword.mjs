import { readFileSync } from "node:fs";
const d = JSON.parse(readFileSync("asset_work/target/assetdb/entity/weapon/weapon_sword.json", "utf-8"));
console.log("TOP KEYS:", Object.keys(d));
const layers = d.layers ?? d;
console.log("LAYERS TYPE:", Array.isArray(layers) ? "array" : typeof layers);
const dump = JSON.stringify(layers).slice(0, 400);
console.log(dump);
