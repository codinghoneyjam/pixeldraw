#!/usr/bin/env node
/**
 * PixelDraw MCP (Model Context Protocol) Server
 * 
 * Implements standard JSON-RPC 2.0 over stdio for Claude Desktop, Cursor, Antigravity, Cline, etc.
 * Zero external dependencies: runs with vanilla Node.js 18+.
 */

import readline from "node:readline";
import { renderQuery } from "./renderer.js";

const SERVER_NAME = "pixeldraw-mcp";
const SERVER_VERSION = "1.0.0";
const PROTOCOL_VERSION = "2024-11-05";

const TOOLS = [
  {
    name: "draw_image",
    description: "Draw pixel-art shapes onto a PNG file using PixelDraw query DSL. The PNG is saved directly to the requested output directory (e.g. in another project).",
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "Drawing DSL query. Commands: 'canvas w 64 h 64 bg #00000000', 'circle x 3 y 4 w 24 h 38 color #77832F', 'rect x 0 y 0 w 10 h 10 fill #FF0000 outline #000000 width 1', 'line x0 0 y0 0 x1 10 y1 10 color #FFF', 'polygon pts 0,0 10,0 5,10 color #0F0', 'pixel x 1 y 1 color #FFF'",
        },
        output_path: {
          type: "string",
          description: "Target file path to save the generated PNG (e.g., '../target_project/query_result/item.png'). Directories will be created automatically.",
        },
        width: {
          type: "number",
          description: "Canvas width in pixels (optional, default: 64)",
        },
        height: {
          type: "number",
          description: "Canvas height in pixels (optional, default: 64)",
        },
        background: {
          type: "string",
          description: "Canvas background hex color or 'transparent' (optional, default: 'transparent')",
        },
      },
      required: ["query", "output_path"],
    },
  },
];

function sendResponse(id, result, error = null) {
  const resp = {
    jsonrpc: "2.0",
    id,
  };
  if (error) {
    resp.error = error;
  } else {
    resp.result = result;
  }
  process.stdout.write(JSON.stringify(resp) + "\n");
}

async function handleMessage(msg) {
  if (!msg || typeof msg !== "object") return;
  const { id, method, params } = msg;

  if (method === "initialize") {
    sendResponse(id, {
      protocolVersion: PROTOCOL_VERSION,
      capabilities: {
        tools: {},
      },
      serverInfo: {
        name: SERVER_NAME,
        version: SERVER_VERSION,
      },
    });
    return;
  }

  if (method === "notifications/initialized") {
    // Client notification, no reply needed
    return;
  }

  if (method === "tools/list") {
    sendResponse(id, {
      tools: TOOLS,
    });
    return;
  }

  if (method === "tools/call") {
    const { name, arguments: args } = params ?? {};
    if (name === "draw_image") {
      try {
        const renderResult = await renderQuery({
          query: args.query,
          outputPath: args.output_path,
          width: args.width,
          height: args.height,
          background: args.background,
        });

        sendResponse(id, {
          content: [
            {
              type: "text",
              text: `Image successfully rendered and saved to: ${renderResult.outputPath}\nSize: ${renderResult.width}x${renderResult.height}px, Drawn pixels: ${renderResult.drawnPixels}, PNG file size: ${renderResult.fileSize} bytes.`,
            },
          ],
          isError: false,
        });
      } catch (err) {
        sendResponse(id, {
          content: [
            {
              type: "text",
              text: `Rendering failed: ${err.message}`,
            },
          ],
          isError: true,
        });
      }
      return;
    }

    sendResponse(id, null, {
      code: -32601,
      message: `Tool not found: ${name}`,
    });
    return;
  }

  // Ping or keepalive
  if (method === "ping") {
    sendResponse(id, {});
    return;
  }

  if (id !== undefined) {
    sendResponse(id, null, {
      code: -32601,
      message: `Method not found: ${method}`,
    });
  }
}

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false,
});

rl.on("line", async (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;
  try {
    const json = JSON.parse(trimmed);
    await handleMessage(json);
  } catch (err) {
    // Malformed JSON
    sendResponse(null, null, {
      code: -32700,
      message: `Parse error: ${err.message}`,
    });
  }
});
