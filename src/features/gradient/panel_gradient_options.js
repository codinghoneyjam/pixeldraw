export function mountGradientOptions(root, { session, toolManager } = {}) {
  const innerRadius = root?.querySelector("#dt-gradient-inner-radius");
  const edgeMode = root?.querySelector("#dt-gradient-edge-mode");
  if (!innerRadius || !edgeMode) return null;

  const activeTool = () => session?.settings?.activeTool === "radial_gradient"
    ? toolManager?.get("radial_gradient")
    : null;
  const sync = () => {
    const tool = activeTool();
    if (!tool) return;
    if (document.activeElement !== innerRadius) innerRadius.value = String(tool.innerRadius);
    edgeMode.value = tool.edgeMode;
  };
  const update = () => {
    const tool = activeTool();
    if (!tool) return;
    const radius = Number(innerRadius.value);
    if (Number.isInteger(radius)) tool.innerRadius = Math.max(0, Math.min(2048, radius));
    if (edgeMode.value === "transparent" || edgeMode.value === "solid") tool.edgeMode = edgeMode.value;
  };
  innerRadius.addEventListener("change", update);
  edgeMode.addEventListener("change", update);
  sync();
  return {
    sync,
    dispose() {
      innerRadius.removeEventListener("change", update);
      edgeMode.removeEventListener("change", update);
    },
  };
}