import { readFileSync } from "node:fs";
const recipe = JSON.parse(readFileSync("asset_work/target/assetdb/entity/weapon/weapon_sword.json", "utf-8"));
for (const cmd of recipe.albedo_layers) {
  const t = cmd.cmd ?? cmd.type;
  const extra = t === "line" ? JSON.stringify(cmd.points ?? cmd.pts) : t === "rect" || t === "rectangle" ? JSON.stringify(cmd.box ?? cmd.bbox) : `pts=${(cmd.pts ?? cmd.points ?? []).length}`;
  console.log(t, "| fill:", cmd.fill ?? cmd.color, "| outline:", cmd.outline, "| width:", cmd.width, "|", extra);
}
