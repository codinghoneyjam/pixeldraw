export function mountTextOptions(root, { toolManager } = {}) {
  const text = root?.querySelector("#dt-text-value");
  const font = root?.querySelector("#dt-text-font");
  const strokeWidth = root?.querySelector("#dt-text-stroke-width");
  if (!text || !font || !strokeWidth) return null;

  const tool = () => toolManager?.get("text") ?? null;
  const update = () => {
    const active = tool();
    if (!active) return;
    active.text = text.value;
    active.font = font.value;
    const width = Number(strokeWidth.value);
    active.strokeWidth = Number.isInteger(width) ? Math.max(0, Math.min(16, width)) : 0;
  };
  const onText = () => update();
  text.addEventListener("input", onText);
  font.addEventListener("change", onText);
  strokeWidth.addEventListener("change", onText);
  update();

  return {
    sync: update,
    dispose() {
      text.removeEventListener("input", onText);
      font.removeEventListener("change", onText);
      strokeWidth.removeEventListener("change", onText);
    },
  };
}