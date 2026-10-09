function canInline(value) {
  if (Array.isArray(value)) return value.every(canInline);
  return value === null || typeof value !== "object";
}

export function stringifyReadableJson(value) {
  function format(item, depth, key = "") {
    const indent = "  ".repeat(depth);
    if (Array.isArray(item)) {
      if (item.length === 0) return "[]";
      if (key === "commands") {
        const commandIndent = "  ".repeat(depth + 1);
        return `[\n${item.map((command) => `${commandIndent}${JSON.stringify(command)}`).join(",\n")}\n${indent}]`;
      }
      if (canInline(item)) return JSON.stringify(item);
      const childIndent = "  ".repeat(depth + 1);
      return `[\n${item.map((entry) => `${childIndent}${format(entry, depth + 1)}`).join(",\n")}\n${indent}]`;
    }
    if (item && typeof item === "object") {
      const entries = Object.entries(item);
      if (entries.length === 0) return "{}";
      const childIndent = "  ".repeat(depth + 1);
      return `{\n${entries.map(([name, entry]) => `${childIndent}${JSON.stringify(name)}: ${format(entry, depth + 1, name)}`).join(",\n")}\n${indent}}`;
    }
    return JSON.stringify(item);
  }
  return format(value, 0);
}