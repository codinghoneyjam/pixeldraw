#!/usr/bin/env node
/**
 * PixelDraw CLI Runner
 * 
 * Usage:
 *   node mcp/cli.js --query "circle x 3 y 4 w 24 h 38 color #77832F" --out "query_result/circle.png"
 *   node mcp/cli.js --file "path/to/recipe.txt" --out "query_result/output.png"
 */

import fs from "node:fs";
import { renderQuery } from "./renderer.js";

async function main() {
  const args = process.argv.slice(2);
  let query = "";
  let outPath = "";
  let width = 64;
  let height = 64;
  let background = "transparent";

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--query" || arg === "-q") {
      query = args[++i];
    } else if (arg === "--file" || arg === "-f") {
      const filePath = args[++i];
      query = fs.readFileSync(filePath, "utf-8");
    } else if (arg === "--out" || arg === "-o") {
      outPath = args[++i];
    } else if (arg === "--width" || arg === "-w") {
      width = parseInt(args[++i], 10);
    } else if (arg === "--height" || arg === "-h") {
      height = parseInt(args[++i], 10);
    } else if (arg === "--bg") {
      background = args[++i];
    }
  }

  // If no query passed via args, check if piped via stdin
  if (!query && !process.stdin.isTTY) {
    query = fs.readFileSync(0, "utf-8");
  }

  if (!query) {
    console.error("Error: No query provided. Use --query <dsl> or --file <path>");
    console.error("Example: node mcp/cli.js --query \"circle x 3 y 4 w 24 h 38 color #77832F\" --out \"query_result/test.png\"");
    process.exit(1);
  }

  if (!outPath) {
    outPath = "query_result/output.png";
  }

  try {
    const result = await renderQuery({
      query,
      outputPath: outPath,
      width,
      height,
      background,
    });
    console.log(JSON.stringify(result, null, 2));
  } catch (err) {
    console.error("Render failed:", err.message);
    process.exit(1);
  }
}

main();
