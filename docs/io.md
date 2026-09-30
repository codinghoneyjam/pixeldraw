# io — 저장·불러오기·내보내기·자동저장

## 역할 요약

- JSON v2 문서/레이어 직렬화와 PNG 인코딩·디코딩(자체 코덱, 의존성 0).
- 검증은 throw 없이 `{ok, errors}` 수집(최대 50개, 구조 오류 우선).
- 브라우저 파일 저장·열기 + IndexedDB 청크 단위 자동저장.

## 파일별 API

### base64.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `bytesToBase64` | `bytesToBase64(u8)` | Node Buffer 또는 btoa 청크 방식 |
| `base64ToBytes` | `base64ToBytes(s)` | 부정합 시 `SCHEMA` throw |

### png.js (8bit RGBA 자체 코덱)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `encodePng` | `encodePng(rgba, width, height)` | filter0 + 단일 IDAT deflate, 크기 오류 `PNG_UNSUPPORTED` |
| `decodePng` | `decodePng(bytes)` | filter0–4 지원, 시그니처/`PNG_SIGNATURE`, CRC/`PNG_CRC`, 규격 외/`PNG_UNSUPPORTED` |

`CompressionStream` 필요(Chrome 80+, Firefox 113+, Safari 16.4+).

### validate.js (throw 없음)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `validateDocument` | `validateDocument(obj)` | 문서 전체 검증 → `{ok, errors:[{code,path,message}]}` |
| `validateLayerFile` | `validateLayerFile(obj)` | 단일 레이어 파일 검증 |

검사: 스키마 키·캔버스(32배수·상한)·청크(cx 0–59, cy 0–33, png는 `iVBORw0KGgo` 시작 base64)·레이어(1–64, id 중복, 활성층 존재)·청크 중복/범위 밖/32×32 디코드 실패. 미지정 키도 `SCHEMA`.

### serialize.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `documentToJson` | `documentToJson(doc, {onProgress})` | 스키마 키 순서·하위층 먼저·`(cy,cx)` 정렬·빈 청크 제외·64개 배치 인코딩 |
| `layerToJson` | `layerToJson(doc, layerId)` | 단일 레이어 파일 `{format:"draw_tool.layer"}` |
| `jsonToDocument` | `jsonToDocument(obj, {onProgress})` | 검증→vector층 `UNSUPPORTED_LAYER_TYPE`→청크 디코드·정규화 |
| `importLayerJson` | `importLayerJson(doc, obj)` | 문서 미변경, 새 id 발급, 캔버스 밖 청크는 `dropped` 계수 |

불러오기 시 투명 픽셀 RGB 정규화, 빈 디코드 청크는 버림. blend가 normal이 아니면 경고 후 normal 강등.

### export_png.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `flattenToRgba` | `flattenToRgba(doc, {includeBackground=true})` | 배경+가시층 bottom-up `over()` 평탄화 |
| `exportPngBytes` | `exportPngBytes(doc, opts)` | 평탄화 후 `encodePng` |

### file_io.js (Node-safe, DOM 지연 접근)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `sanitizeFileName` | `sanitizeFileName(name)` | 금지문자→`_`, 앞뒤 공백·점 제거, 최대 80자, 빈값 `untitled` |
| `saveTextFile` | `saveTextFile(suggestedName, text, mime)` | File System Access 우선, 폴백 다운로드 |
| `saveBinaryFile` | `saveBinaryFile(suggestedName, bytes, mime)` | 바이너리 저장 |
| `pickFile` | `pickFile(accept)` | 피커 우선, 폴백 `<input type=file>`, 취소 시 null |
| `readJsonFile` | `readJsonFile(file)` | 64MiB 초과 `FILE_TOO_LARGE`, 파싱 실패 `SCHEMA` |

### store_idb.js (Node import 가능, IDB 지연 접근)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `chunkRecordKey` | `chunkRecordKey(layerId, cx, cy)` | `"id\|cy\|cx"` 안정 키 |
| `buildMetaRecord` | `buildMetaRecord(doc)` | 메타 레코드(schema:2, canvas, 층 목록, active, updatedAt) |
| `buildChunkRecords` | `buildChunkRecords(layer, keys)` | ArrayBuffer 복사본 레코드 목록 |
| `AutosaveStore` | `constructor(db)` / `static open()` | IDB 없으면 null |
| — | `attach(session)` / `detach()` | Session 이벤트 구독·해제 + visibility/pagehide 플러시 |
| — | `peek()` / `flushNow()` / `load()` / `clear()` | 메타 요약·즉시 저장·복원·전체 삭제 |
| `_INTERNALS` | `{DB_NAME, DB_VERSION, META_KEY, DEBOUNCE_MS, FORCE_MS}` | DB명 `draw_tool_v2`, 디바운스 800ms·강제 5s |

## 핵심 흐름

- 저장: `documentToJson` → `saveTextFile`. 성공 시 `history.markSaved()` + `history-changed`.
- 열기: `pickFile` → `readJsonFile`(크기·JSON 검사) → `validateDocument` → `jsonToDocument` → `loadDocument`.
- 레이어 가져오기: `importLayerJson` → `insertLayer`. 캔버스 크기 다르면 경고, 범위 밖 청크는 버리고 개수 표시.
- 자동저장: `pixels-changed`→dirty 청크 집합, `layers-changed`/`document-replaced`→메타 dirty. 800ms 디바운스·5s 강제 상한으로 1 트랜잭션 기록. 용량 초과 시 `STORAGE_QUOTA` warn.

## 주의점

- 검증 실패 코드는 `SCHEMA` 외 `DUP_LAYER_ID, ACTIVE_LAYER_MISSING, CHUNK_DUPLICATE, CHUNK_OUT_OF_CANVAS, CHUNK_BAD_PNG`를 확인.
- JSON 64MiB 초과는 읽기 단계에서 `FILE_TOO_LARGE`.
- `load()`는 메타 부정합 시 `clear()` 후 null 반환(복원 불가 알림은 UI 담당).
- 모듈 규약: 상대 경로 ESM import만, `export default`·동적 `import()`·`import.meta` 없음 (`contract.md` §5).
