// <META - FILE SUMMARY - Toast notifications stacked in #dt-toast>

// <META - ROLE : Append an auto-expiring toast item | L3-14>
export function toast(text, level = "info") {
  if (typeof document === "undefined") return;
  const box = document.getElementById("dt-toast");
  if (!box) return;
  const item = document.createElement("div");
  item.className = `dt-toast-item dt-toast-${level}`;
  item.textContent = text;
  box.append(item);
  setTimeout(() => item.remove(), level === "error" ? 8000 : 4000);
}
