export function mountArcOptions(root, { session, toolManager } = {}) {
  const start = root?.querySelector("#dt-arc-start");
  const end = root?.querySelector("#dt-arc-end");
  const width = root?.querySelector("#dt-arc-width");
  const fill = root?.querySelector("#dt-chord-fill");
  const outline = root?.querySelector("#dt-chord-outline");
  if (!start || !end || !width || !fill || !outline) return null;

  const activeTool = () => {
    const id = session?.settings?.activeTool;
    return id === "arc" || id === "chord" ? toolManager?.get(id) : null;
  };
  const sync = () => {
    const tool = activeTool();
    if (!tool) return;
    if (document.activeElement !== start) start.value = String(tool.start);
    if (document.activeElement !== end) end.value = String(tool.end);
    if (document.activeElement !== width) width.value = String(tool.width);
    fill.checked = tool.fill;
    outline.checked = tool.outline;
  };
  const update = () => {
    const tool = activeTool();
    if (!tool) return;
    const startAngle = Number(start.value);
    const endAngle = Number(end.value);
    const lineWidth = Number(width.value);
    if (Number.isFinite(startAngle)) tool.start = startAngle;
    if (Number.isFinite(endAngle)) tool.end = endAngle;
    if (Number.isInteger(lineWidth)) tool.width = Math.max(1, Math.min(64, lineWidth));
    tool.fill = fill.checked;
    tool.outline = outline.checked;
  };
  const controls = [start, end, width, fill, outline];
  for (const control of controls) control.addEventListener("change", update);
  sync();
  return {
    sync,
    dispose() { for (const control of controls) control.removeEventListener("change", update); },
  };
}