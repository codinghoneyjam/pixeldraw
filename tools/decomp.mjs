import { transpileAndRender } from "./recipe/transpile.mjs";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const cmds = JSON.parse(fs.readFileSync(process.argv[2], "utf-8"));
const outDir = process.argv[3];
for (let i = 0; i < cmds.length; i++) {
  const recipe = { name: "t", canvas: [128, 128], layers: [{ id: "b", commands: [cmds[i]] }] };
  const rec = path.join(outDir, `trec${i}.json`);
  fs.writeFileSync(rec, JSON.stringify(recipe));
  await transpileAndRender({ recipePath: rec, layerKey: "b",
    outputDocPath: path.join(outDir, `tdoc${i}.json`),
    outputPngPath: path.join(outDir, `dt${i}.png`) });
}
console.log("done");
