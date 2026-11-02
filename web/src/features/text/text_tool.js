import { packRGBA, parseHex } from "../../core/pixel.js";
import { textMask } from "../../core/raster/text_mask.js";
import { Tool } from "../../tools/tool_base.js";

export class TextTool extends Tool {
  constructor(env, glyphSets = {}) {
    super(env);
    this.glyphSets = glyphSets;
    this.text = "";
    this.font = "default-42";
    this.strokeWidth = 0;
  }

  get id() { return "text"; }
  get cursor() { return "text"; }

  pointerDown(ev) {
    if ((ev.button !== 0 && ev.button !== undefined) || !this.text || !this.session.doc) return;
    const glyphs = this.glyphSets[this.font]?.glyphs;
    if (!glyphs) {
      this.session.notify("warn", "텍스트 글꼴을 불러오지 못했습니다", "TEXT_FONT_MISSING");
      return;
    }
    const unsupported = [...this.text].find((char) => !glyphs[char]);
    if (unsupported) {
      this.session.notify("warn", `지원하지 않는 문자: ${unsupported}`, "TEXT_GLYPH_MISSING");
      return;
    }
    const { widthPx: width, heightPx: height } = this.session.doc.canvas;
    const fill = parseHex(this.session.settings.primaryColor);
    const stroke = parseHex(this.session.settings.secondaryColor);
    const mask = textMask({
      text: this.text,
      glyphs,
      x: Math.round(ev.x),
      y: Math.round(ev.y),
      fill: packRGBA(fill.r, fill.g, fill.b, fill.a),
      stroke: packRGBA(stroke.r, stroke.g, stroke.b, stroke.a),
      strokeWidth: this.strokeWidth,
      w: width,
      h: height,
    });
    const edit = this.session.beginEdit({ label: "텍스트" });
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const color = mask.data[y * width + x];
        if (color !== 0) edit.writer.set(x, y, color);
      }
    }
    edit.commit("텍스트");
    this.env.requestRender?.();
  }
}