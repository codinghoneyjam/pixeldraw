---
name: pixeldraw-raster
description: Draw pixel-art shapes (circle, rect, line, polygon, pixel) using PixelDraw query DSL and output PNG files directly to target project directories.
---

# PixelDraw Raster Tool & Skill

PixelDraw의 순수 자바스크립트 헤드리스 엔진을 활용하여, 쿼리(DSL)를 기반으로 고성능 픽셀아트 이미지를 렌더링하고 타 프로젝트의 특정 폴더(예: `query_result/`)에 PNG로 내보냅니다.

## 1. CLI 실행 방법 (타 프로젝트에서 직접 호출)

```bash
node "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/cli.js" \
  --query "circle x 3 y 4 w 24 h 38 color #77832F" \
  --out "../other_project/query_result/circle.png"
```

### 멀티라인 쿼리 예시 (여러 도형 조합):
```bash
node "C:/Users/ksj20/Your wor(l)d/pixeldraw/mcp/cli.js" \
  --query "canvas w 64 h 64 bg transparent
circle x 8 y 8 w 48 h 48 fill #4CAF50 outline #2E7D32 width 2
rect x 24 y 24 w 16 h 16 fill #FFFFFF
pixel x 32 y 32 color #FF0000" \
  --out "../other_project/query_result/badge.png"
```

## 2. MCP (Model Context Protocol) 연동 설정

Claude Desktop, Antigravity, Cursor, Cline의 설정 파일(`claude_desktop_config.json` 등)에 아래와 같이 추가하여 LLM 툴로 사용합니다:

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

### 사용 가능한 MCP 도구: `draw_image`
- `query` (필수): 도형을 그릴 쿼리 문자열
- `output_path` (필수): 출력할 PNG 파일 경로 (상대 경로 또는 절대 경로)
- `width` (선택): 캔버스 너비 (기본 64)
- `height` (선택): 캔버스 높이 (기본 64)
- `background` (선택): 배경색 (기본: `transparent`)

## 3. 쿼리(DSL) 문법 명세

모든 명령어는 `pixeldraw`의 실제 내부 렌더링 함수(`applyShape`, `PixelWriter`)의 인자와 1:1로 대응됩니다:

- **캔버스 설정**: `canvas w <num> h <num> [bg <hex|transparent>]`
- **원/타원**: `circle x <num> y <num> w <num> h <num> [color|fill <hex>] [outline <hex>] [width <num>]`
  - (또는 중심/반지름: `circle cx <num> cy <num> r <num> color <hex>`)
- **사각형/둥근 사각형**: `rect x <num> y <num> w <num> h <num> [radius <num>] [color|fill <hex>] [outline <hex>] [width <num>]`
- **직선**: `line x0 <num> y0 <num> x1 <num> y1 <num> color <hex> [width <num>]`
- **다각형**: `polygon pts <x1,y1> <x2,y2> <x3,y3> ... [color|fill <hex>] [outline <hex>] [width <num>]`
- **단일 픽셀**: `pixel x <num> y <num> color <hex>`
