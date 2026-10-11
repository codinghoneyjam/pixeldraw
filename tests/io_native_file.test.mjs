// <META - FILE SUMMARY - Native (desktop) file bridge: command names, base64 payloads, File result>
//
// file_io.js picks its runtime by feature detection: `window.__TAURI__` present
// means the Tauri desktop shell (see docs/desktop.md), absent means the browser
// path that run.py uses. These tests install a fake bridge and pin the native
// contract, so a renamed Rust command or a changed payload shape fails here
// instead of in a packaged app.
import { afterEach, describe, it } from "node:test";
import assert from "node:assert/strict";

import { pickFile, saveBinaryFile, saveTextFile } from "../src/io/file_io.js";
import { bytesToBase64 } from "../src/io/base64.js";

// <META - ROLE : Install a recording fake Tauri bridge | L15-27>
function installBridge(replies) {
  const calls = [];
  globalThis.window = {
    __TAURI__: {
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        return replies(cmd, args);
      },
    },
  };
  return calls;
}

afterEach(() => {
  delete globalThis.window;
});

describe("C9 native file bridge", () => {
  // <META - ROLE : Save goes through pick_save_path then write_file_bytes | L40-52>
  it("saves text via pick_save_path + write_file_bytes with a base64 payload", async () => {
    const calls = installBridge((cmd) => (cmd === "pick_save_path" ? "C:\\out\\art.draw.json" : undefined));
    const ok = await saveTextFile("art.draw.json", '{"a":1}', "application/json");
    assert.equal(ok, true);
    assert.deepEqual(calls.map((c) => c.cmd), ["pick_save_path", "write_file_bytes"]);
    assert.deepEqual(calls[0].args, {
      suggested: "art.draw.json",
      filter: { name: "JSON", extensions: ["json"] },
    });
    assert.equal(calls[1].args.path, "C:\\out\\art.draw.json");
    assert.equal(calls[1].args.data, bytesToBase64(new TextEncoder().encode('{"a":1}')));
  });

  // <META - ROLE : PNG export keeps its own filter and raw bytes | L54-66>
  it("saves binaries with a PNG filter derived from the suggested name", async () => {
    const calls = installBridge((cmd) => (cmd === "pick_save_path" ? "out.png" : undefined));
    const png = new Uint8Array([137, 80, 78, 71, 0, 1, 255]);
    const ok = await saveBinaryFile("sprite.png", png, "image/png");
    assert.equal(ok, true);
    assert.deepEqual(calls[0].args.filter, { name: "PNG", extensions: ["png"] });
    assert.equal(calls[1].args.data, bytesToBase64(png));
  });

  // <META - ROLE : Cancelling the save dialog reports false and writes nothing | L68-74>
  it("returns false when the save dialog is cancelled", async () => {
    const calls = installBridge(() => null);
    const ok = await saveTextFile("a.json", "{}");
    assert.equal(ok, false);
    assert.equal(calls.length, 1);
  });

  // <META - ROLE : Open returns a real File so readJsonFile stays untouched | L76-90>
  it("picks a file through the native dialog and returns a File", async () => {
    const calls = installBridge((cmd) => {
      if (cmd === "pick_open_path") return "D:\\art\\scene.draw.json";
      return bytesToBase64(new TextEncoder().encode('{"format":"draw_tool.document"}'));
    });
    const file = await pickFile(".json,application/json");
    assert.equal(calls.length, 2);
    assert.deepEqual(calls[0].args.filter, { name: "JSON", extensions: ["json"] });
    assert.equal(file.name, "scene.draw.json");
    assert.equal(await file.text(), '{"format":"draw_tool.document"}');
  });

  // <META - ROLE : Cancelling the open dialog yields null | L92-97>
  it("returns null when the open dialog is cancelled", async () => {
    installBridge(() => null);
    assert.equal(await pickFile(".json"), null);
  });

  // <META - ROLE : No bridge means the browser path runs instead | L99-105>
  it("falls back to the browser path when the bridge is absent", async () => {
    delete globalThis.window;
    await assert.rejects(() => saveTextFile("a.json", "{}"), /no file save API available/);
    await assert.rejects(() => pickFile(".json"), /no file pick API available/);
  });
});
