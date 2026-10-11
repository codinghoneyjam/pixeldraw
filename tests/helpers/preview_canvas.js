export function withPreviewCanvas(run) {
  const previousDocument = globalThis.document;
  let drawCount = 0;
  const previewContext = {
    createImageData: (width, height) => ({ data: new Uint8ClampedArray(width * height * 4) }),
    putImageData() {},
  };
  globalThis.document = {
    createElement: () => ({ width: 0, height: 0, getContext: () => previewContext }),
  };
  const ctx = {
    save() {},
    restore() {},
    drawImage() { drawCount++; },
    beginPath() {},
    setLineDash() {},
    arc() {},
    stroke() {},
  };
  try {
    return run({ ctx, drawCount: () => drawCount });
  } finally {
    if (previousDocument === undefined) delete globalThis.document;
    else globalThis.document = previousDocument;
  }
}