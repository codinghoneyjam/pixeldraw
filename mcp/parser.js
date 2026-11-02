/**
 * PixelDraw Query DSL Parser
 * 
 * Supports commands matching pixeldraw internal functions:
 * - canvas w <w> h <h> [bg <color>]
 * - circle x <x> y <y> w <w> h <h> [color <hex>] [fill <hex>] [outline <hex>] [width <num>]
 * - ellipse x <x> y <y> w <w> h <h> [color <hex>] [fill <hex>] [outline <hex>] [width <num>]
 * - rect x <x> y <y> w <w> h <h> [radius <r>] [color <hex>] [fill <hex>] [outline <hex>] [width <num>]
 * - line x0 <x0> y0 <y0> x1 <x1> y1 <y1> [color <hex>] [width <num>]
 * - polygon pts <x,y> <x,y> <x,y> ... [color <hex>] [fill <hex>] [outline <hex>] [width <num>]
 * - pixel x <x> y <y> color <hex>
 */

export function parseQuery(queryText, defaultOptions = {}) {
  const lines = queryText.split(/\r?\n/);
  
  let canvas = {
    w: defaultOptions.width ?? 64,
    h: defaultOptions.height ?? 64,
    bg: defaultOptions.background ?? "transparent",
  };

  const commands = [];

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
    const rawLine = lines[lineIndex].trim();
    if (!rawLine || rawLine.startsWith("#") || rawLine.startsWith("//")) {
      continue;
    }

    const tokens = rawLine.split(/\s+/);
    const cmd = tokens[0].toLowerCase();
    const args = parseArgs(tokens.slice(1));

    switch (cmd) {
      case "canvas": {
        if (args.w !== undefined) canvas.w = Math.max(1, parseInt(args.w, 10));
        if (args.width !== undefined) canvas.w = Math.max(1, parseInt(args.width, 10));
        if (args.h !== undefined) canvas.h = Math.max(1, parseInt(args.h, 10));
        if (args.height !== undefined) canvas.h = Math.max(1, parseInt(args.height, 10));
        if (args.bg !== undefined) canvas.bg = args.bg;
        if (args.background !== undefined) canvas.bg = args.background;
        break;
      }

      case "circle":
      case "ellipse": {
        let x = parseInt(args.x ?? 0, 10);
        let y = parseInt(args.y ?? 0, 10);
        let w = parseInt(args.w ?? args.width ?? 10, 10);
        let h = parseInt(args.h ?? args.height ?? w, 10);

        // Center-radius format support: cx cy r
        if (args.cx !== undefined && args.cy !== undefined && args.r !== undefined) {
          const cx = parseInt(args.cx, 10);
          const cy = parseInt(args.cy, 10);
          const r = parseInt(args.r, 10);
          x = cx - r;
          y = cy - r;
          w = r * 2 + 1;
          h = r * 2 + 1;
        }

        const color = args.color ?? args.fill;
        const outline = args.outline;
        const strokeWidth = parseInt(args.width ?? args.sw ?? 1, 10);

        commands.push({
          type: "ellipse",
          bbox: [x, y, w, h],
          fill: color,
          outline: outline,
          strokeWidth,
        });
        break;
      }

      case "rect":
      case "rectangle":
      case "rrect":
      case "rounded_rect": {
        const x = parseInt(args.x ?? 0, 10);
        const y = parseInt(args.y ?? 0, 10);
        const w = parseInt(args.w ?? args.width ?? 10, 10);
        const h = parseInt(args.h ?? args.height ?? 10, 10);
        const radius = parseInt(args.radius ?? args.r ?? 0, 10);
        const color = args.color ?? args.fill;
        const outline = args.outline;
        const strokeWidth = parseInt(args.width ?? args.sw ?? 1, 10);

        commands.push({
          type: radius > 0 ? "rrect" : "rect",
          bbox: [x, y, w, h],
          radius,
          fill: color,
          outline: outline,
          strokeWidth,
        });
        break;
      }

      case "line": {
        let x0 = 0, y0 = 0, x1 = 0, y1 = 0;
        if (args.x0 !== undefined && args.y0 !== undefined && args.x1 !== undefined && args.y1 !== undefined) {
          x0 = parseInt(args.x0, 10);
          y0 = parseInt(args.y0, 10);
          x1 = parseInt(args.x1, 10);
          y1 = parseInt(args.y1, 10);
        } else if (args.pts) {
          const parts = args.pts.split(",");
          if (parts.length >= 4) {
            x0 = parseInt(parts[0], 10);
            y0 = parseInt(parts[1], 10);
            x1 = parseInt(parts[2], 10);
            y1 = parseInt(parts[3], 10);
          }
        }
        const color = args.color ?? args.fill ?? args.stroke ?? "#000000";
        const strokeWidth = parseInt(args.width ?? args.sw ?? 1, 10);

        commands.push({
          type: "line",
          p0: { x: x0, y: y0 },
          p1: { x: x1, y: y1 },
          color,
          strokeWidth,
        });
        break;
      }

      case "polygon": {
        const points = [];
        // Support `pts 0,0 10,0 5,10`
        if (args.pts) {
          const ptTokens = Array.isArray(args.pts) ? args.pts : [args.pts];
          for (const pt of ptTokens) {
            const [px, py] = pt.split(",").map(Number);
            if (!Number.isNaN(px) && !Number.isNaN(py)) {
              points.push([px, py]);
            }
          }
        }
        const color = args.color ?? args.fill;
        const outline = args.outline;
        const strokeWidth = parseInt(args.width ?? args.sw ?? 1, 10);

        commands.push({
          type: "polygon",
          points,
          fill: color,
          outline: outline,
          strokeWidth,
        });
        break;
      }

      case "pixel":
      case "point": {
        const x = parseInt(args.x ?? 0, 10);
        const y = parseInt(args.y ?? 0, 10);
        const color = args.color ?? args.fill ?? "#000000";
        commands.push({
          type: "pixel",
          x,
          y,
          color,
        });
        break;
      }

      default:
        throw new Error(`Line ${lineIndex + 1}: Unknown command '${cmd}' in query: "${rawLine}"`);
    }
  }

  return { canvas, commands };
}

function parseArgs(tokens) {
  const args = {};
  let i = 0;
  while (i < tokens.length) {
    const key = tokens[i];
    if (key === "pts") {
      // pts can take multiple points until another named key or end of line
      const ptList = [];
      i++;
      while (i < tokens.length && tokens[i].includes(",")) {
        ptList.push(tokens[i]);
        i++;
      }
      args.pts = ptList;
      continue;
    }

    if (i + 1 < tokens.length) {
      args[key] = tokens[i + 1];
      i += 2;
    } else {
      args[key] = true;
      i++;
    }
  }
  return args;
}
