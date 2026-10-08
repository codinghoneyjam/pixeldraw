import { readFileSync } from "node:fs";
const recipe = JSON.parse(readFileSync("asset_work/target/assetdb/entity/weapon/weapon_sword.json", "utf-8"));
for (const cmd of recipe.albedo_layers) {
  const pts = cmd.pts ?? cmd.points ?? [];
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  console.log(cmd.cmd, "fill:", cmd.fill ?? cmd.color, "outline:", cmd.outline, "bbox:", [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]);
}
