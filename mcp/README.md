# PixelDraw MCP Configuration & Integration Guide

이 문서는 다른 프로젝트에서 `pixeldraw`의 픽셀아트 렌더링 엔진을 MCP(Model Context Protocol) 또는 CLI로 연동하여, AI 에이전트가 쿼리를 날려 PNG 이미지를 생성하도록 설정하는 방법을 설명합니다.

---

## 1. MCP 서버 등록 설정 (클라이언트별)

각 AI 툴의 MCP 설정 파일에 아래 구성을 등록합니다.  
*(주의: Windows 경로의 공백과 괄호 `Your wor(l)d`를 슬래시 `/`로 명시해야 합니다.)*

### A. Claude Desktop
- 설정 파일: `%APPDATA%\Claude\claude_desktop_config.json`
```json
{
  "mcpServers": {
    "pixeldraw": {
      "command": "node",
      "args": [
        "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/server.js"
      ]
    }
  }
}
```

### B. Cursor IDE
1. `Settings` (`Ctrl+,`) -> `Features` -> `MCP` 이동
2. `+ Add New MCP Server` 클릭 후 입력:
   - **Name**: `pixeldraw`
   - **Type**: `command`
   - **Command**: `node "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/server.js"`

### C. Antigravity / Gemini CLI
- 설정 파일: `~/.gemini/antigravity/mcp_servers.json`
```json
{
  "mcpServers": {
    "pixeldraw": {
      "command": "node",
      "args": [
        "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/server.js"
      ]
    }
  }
}
```

### D. Cline / Roo Code (VS Code 확장)
- `cline_mcp_settings.json`에 동일하게 추가:
```json
{
  "mcpServers": {
    "pixeldraw": {
      "command": "node",
      "args": [
        "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/server.js"
      ],
      "disabled": false,
      "autoApprove": ["draw_image"]
    }
  }
}
```

---

## 2. 타 프로젝트 AI 에이전트 지침 주입 (System Prompt / Rules)

타 프로젝트의 루트에 있는 `.cursorrules`, `CLAUDE.md`, 또는 시스템 프롬프트에 아래 내용을 추가하면, 해당 프로젝트의 AI가 픽셀아트 이미지가 필요할 때 자동으로 `draw_image` 도구를 호출합니다.

```markdown
### 🎨 Pixel-Art Generation Guidelines
When asked to create pixel-art, icons, sprites, or game assets:
1. Always use the `draw_image` tool (PixelDraw MCP).
2. Save the output to `query_result/<filename>.png` or the user-specified directory.
3. Build the query DSL sequentially:
   - Base canvas: `canvas w 64 h 64 bg transparent`
   - Shapes: `circle x 3 y 4 w 24 h 38 color #77832F`
   - Rectangles: `rect x <x> y <y> w <w> h <h> fill <hex> outline <hex> width <px>`
   - Lines: `line x0 <x0> y0 <y0> x1 <x1> y1 <y1> color <hex>`
   - Pixels: `pixel x <x> y <y> color <hex>`
4. Read `C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/AI_AGENT_GUIDE.md` for complete DSL specs.
```

---

## 3. CLI를 통한 직접 호출 (MCP 없이 타 프로젝트 코드에서 실행할 때)

타 프로젝트의 Node.js 빌드 스크립트나 커맨드라인에서 직접 호출할 수도 있습니다:

```bash
node "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/cli.js" \
  --query "circle x 3 y 4 w 24 h 38 color #77832F" \
  --out "query_result/circle.png"
```

---

## 4. 파일 구조 및 역할

- [`mcp/server.js`](file:///c:/Users/ksj20/Your%20wor(l)d/pixeldraw/mcp/server.js): 표준 JSON-RPC 2.0 stdio MCP 서버
- [`mcp/parser.js`](file:///c:/Users/ksj20/Your%20wor(l)d/pixeldraw/mcp/parser.js): DSL 텍스트를 내부 함수 파라미터로 변환하는 파서
- [`mcp/renderer.js`](file:///c:/Users/ksj20/Your%20wor(l)d/pixeldraw/mcp/renderer.js): PixelDraw 순수 JS 엔진을 사용해 PNG로 저장하는 렌더러
- [`mcp/cli.js`](file:///c:/Users/ksj20/Your%20wor(l)d/pixeldraw/mcp/cli.js): 터미널/쉘 호출용 엔트리포인트
- [`mcp/AI_AGENT_GUIDE.md`](file:///c:/Users/ksj20/Your%20wor(l)d/pixeldraw/mcp/AI_AGENT_GUIDE.md): 타 AI 에이전트가 참조하는 DSL 상세 사양서
