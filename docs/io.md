# io — 직렬화·PNG·검증·파일·자동저장

## 1. 개요와 책임

io 모듈은 문서의 바깥 경계를 담당한다. 메모리 안의 Document를 JSON v2 파일로 내보내고, 다시 검증해서 복원하며, PNG로 평탄화하고, 브라우저 파일과 IndexedDB에 저장한다. 의존성은 아래 방향으로만 흐른다. core의 상수·에러·청크 저장소와 model의 Document·Layer를 읽되, 그 반대 방향 참조는 없다.
다섯 가지 책임은 다음과 같다. 첫째 직렬화는 빈 청크 제외와 결정적 정렬을 보장한다. 둘째 PNG는 의존성 없는 자체 코덱으로 8비트 RGBA만 다룬다. 셋째 검증은 절대 throw하지 않고 구조 검사 뒤 의미 검사를 수행한다. 넷째 파일 입출력은 브라우저 API를 지연 접근으로 감싸 Node import를 깨지 않는다. 다섯째 자동저장은 청크 단위 dirty 집합을 하나의 트랜잭션으로 기록한다.

## 2. base64.js — 청크 PNG 페이로드 변환

`bytesToBase64(u8)`는 바이트 배열을 base64 문자열로 바꾼다. Node에서는 Buffer를 우선 사용하고, 브라우저에서는 0x8000 바이트씩 잘라 `String.fromCharCode.apply`로 이진 문자열을 만든 뒤 `btoa`한다. 청크로 나누는 이유는 대용량 청크에서 호출 스택 오버플로를 막기 위해서다.
`base64ToBytes(s)`는 역변환이다. 정규식으로 허용 문자와 패딩을 먼저 검사하고, 어긋나면 `SCHEMA` 에러를 던진다. Node에서는 `Buffer.from(s, base64)`로 복원하고, 브라우저에서는 `atob` 뒤 문자 코드를 바이트로 펼친다. 디코드 실패 역시 `SCHEMA`로 통일해서 상위 검증이 한 가지 코드만 보게 한다.

## 3. png.js — 8비트 RGBA 자체 코덱

`encodePng(rgba, width, height)`는 너비·높이가 양의 정수가 아니면 `PNG_UNSUPPORTED`를 던진다. 행마다 필터 바이트 0을 앞에 붙인 raw 버퍼를 만들고, `CompressionStream(deflate)`으로 압축한 뒤 IHDR 한 개와 IDAT 한 개, IEND 순서로 조립한다. IHDR은 폭·높이·비트깊이 8·컬러타입 6·압축·필터·인터레이스 0을 기록한다. 각 청크는 길이와 타입·데이터에 대한 CRC32를 덧붙인다.
`decodePng(bytes)`는 먼저 8바이트 시그니처를 대조하고, 틀리면 `PNG_SIGNATURE`를 던진다. 이후 길이·타입·데이터·CRC 순서로 순회하며 CRC가 어긋나거나 헤더가 잘리면 `PNG_CRC`를 던진다. IHDR에서 비트깊이 8·컬러타입 6·인터레이스 0이 아니면 `PNG_UNSUPPORTED`이며, 면적이 core 상한을 넘겨도 같은 코드다. IDAT 조각을 이어 `DecompressionStream`으로 풀고, 스캔라인 길이가 맞지 않으면 `PNG_UNSUPPORTED`다. 필터 0부터 4까지 복원한다. Sub·Up·Average·Paeth 순서로 이전 픽셀과 윗줄을 참조하며, 알 수 없는 필터 번호는 `PNG_UNSUPPORTED`다. 반환은 너비·높이·RGBA 버퍼 묶음이다. 압축에 WebStreams를 쓰므로 Chrome 80 이상, Firefox 113 이상, Safari 16.4 이상이 필요하다.

## 4. 검증 2단계 — validate·structural·semantic

### 4.1 validate.js — 진입점과 50개 상한

`validateDocument(obj)`와 `validateLayerFile(obj)`는 절대 throw하지 않고 `{ok, errors}`를 반환한다. errors 항목은 코드·경로·메시지 형태이며 최대 50개까지만 수집한다. 문서 검사는 최상위 허용 키 검사부터 시작한다. 허용되지 않은 키가 있으면 `SCHEMA`로 기록한다. 스키마 버전은 2.0.0 고정, 포맷은 문서용 `draw_tool.document`와 레이어용 `draw_tool.layer`를 구분한다. 문서 id는 비어 있지 않은 문자열, 이름은 최대 128자, 활성층 id는 비어 있지 않은 문자열이어야 한다. 레이어 배열은 1개 이상 64개 이하여야 한다. 구조 오류가 50개를 채우면 의미 단계로 넘어가지 않고 바로 반환한다. 의미 단계에서는 층 id 중복을 `DUP_LAYER_ID`로, 활성층이 목록에 없으면 `ACTIVE_LAYER_MISSING`으로 보고한다.
`validateLayerFile`은 최상위 키 네 가지와 원본 캔버스, 단일 층을 같은 방식으로 검사한다.

### 4.2 validate_structural.js — 값 모양 검사

`checkCanvas`는 타일 64와 유닛 32 고정, 너비 32부터 2048까지와 높이 32부터 1088까지를 32배수 조건으로 검사한다. 배경은 transparent 또는 6자리·8자리 16진 색상만 허용한다. 너비·높이가 모두 유효할 때만 치수 객체를 돌려줘서 이후 의미 검사가 캔버스 밖 청크를 판단할 수 있게 한다. 상한은 전부 `core/constants.js`에서 import하며 값을 다시 적지 않는다(스키마 JSON·Python 오라클·테스트가 같은 수를 읽는다).
`checkChunk`는 cx가 0부터 63까지, cy가 0부터 33까지 정수인지 보고, png 문자열이 `iVBORw0KGgo`로 시작하는 base64인지 정규식으로 검사한다. 하나라도 어긋나면 `SCHEMA`를 남기고 false를 돌려준다.
`checkRaster`는 청크 픽셀 32 고정과 인코딩 `png_base64` 고정을 검사하고, 청크 배열 상한 `MAX_CHUNKS_X * MAX_CHUNKS_Y`(2176)개를 넘기면 `SCHEMA`로 기록한다. 각 청크 결과를 불리언 배열로 돌려줘서 의미 단계가 구조적으로 통과한 항목만 실제로 디코드하게 한다.
`checkLayer`는 id 길이 1부터 64까지, 이름 최대 128자, 타입 raster 또는 vector, 가시·잠금 불리언, 불투명도 0부터 1까지, 블렌드 여섯 가지 중 하나를 검사한다. raster층이 shapes를 가지거나 raster가 없으면 `SCHEMA`이며, vector층이 raster를 가지거나 shapes 배열이 없으면 역시 `SCHEMA`다.

### 4.3 validate_semantic.js — 실제 디코드 검사

`checkChunksSemantic(chunks, structuralOk, dims, base, push)`는 구조 검사를 통과한 청크만 base64 디코드 뒤 PNG 디코드까지 수행한다. 같은 좌표가 두 번 나오면 `CHUNK_DUPLICATE`를, 캔버스 치수를 넘어서면 `CHUNK_OUT_OF_CANVAS`를 기록한다. 디코드 결과가 32 곱하기 32가 아니거나 디코드 자체가 실패하면 `CHUNK_BAD_PNG`를 기록한다. 객체가 아닌 항목은 건너뛰어 수집기 폭주를 막는다. 이 단계가 끝까지 수행되는 이유는 정규식만으로는 깨진 PNG를 걸러낼 수 없기 때문이다.

## 5. serialize.js·serialize_vector.js·serialize_recipe.js — 여덟 가지 변환 함수

원래 `serialize.js` 하나가 맡던 세 책임이 파일 세 개로 갈라져 있다. `serialize.js`는
래스터 문서·층 JSON만 갖고, 벡터와 레시피 반대편은 형제 모듈로 나갔다. `serialize.js`
하단에 두 모듈의 공개 함수를 다시 노출하는 배럴을 두어 `file_actions.js`와
`tools/recipe/*`의 import 경로는 그대로 한 곳으로 유지한다.

`documentToJson(doc, {onProgress})`는 전체 문서를 스키마 키 순서대로 만든다. 스키마 버전·포맷·문서 id·이름·캔버스·활성층·층 배열 순서다. 각 층에서는 비어 있는 청크를 제외하고 cy 우선 cx 차선으로 정렬한 뒤, 64개씩 묶어 병렬로 PNG 인코딩한다. 진행 콜백은 누적 완료 수와 전체 청크 수를 받는다. 층 순서는 문서의 아래층부터 위층 순서를 그대로 유지한다.
`layerToJson(doc, layerId)`는 단일 층을 `draw_tool.layer` 포맷으로 감싼다. 원본 캔버스 정보를 함께 넣어 나중에 크기가 달라졌는지 비교할 수 있게 한다.
`jsonToDocument(obj, {onProgress})`는 먼저 전체 검증을 수행하고 실패하면 첫 오류 코드로 throw한다. vector층이 하나라도 있으면 `UNSUPPORTED_LAYER_TYPE`으로 중단한다. 블렌드는 그대로 통과시킨다 — 여섯 종 모두 실제로 합성되므로(`contract.md` §1-1) 강등할 이유가 없고, 알 수 없는 이름은 구조 검증 단계에서 이미 `SCHEMA`로 거절된다. 각 청크는 base64와 PNG를 풀고 투명 픽셀 RGB를 0으로 정규화한 뒤, 완전히 비어 있으면 버린다. 64개 배치마다 진행 콜백을 호출한다. 활성층이 없으면 마지막 층을 활성층으로 삼고, 층 id들을 IdGen에 심어서 이후 발생과 충돌하지 않게 한다. 반환은 문서와 경고 묶음이다.
`importLayerJson(doc, obj)`는 원본 문서를 바꾸지 않고 새 층 객체를 만든다. 검증 뒤 원본 캔버스 크기가 다르면 경고를 남긴다. vector층은 `UNSUPPORTED_LAYER_TYPE`으로 거절한다. 현재 문서 캔버스를 벗어난 청크는 저장하지 않고 버린 개수를 `dropped`으로 돌려준다. 새 층 id는 문서의 IdGen에서 발급받아 중복을 피한다.

### 5.1 serialize_vector.js — 벡터 명령 변환

`documentToVectorJson(doc)`는 모든 층의 비어 있지 않은 청크를 훑어 같은 색이 이어진 직사각형을 `rect`, 외로운 픽셀을 `pixel` 명령으로 바꾼다. 한 청크 안에서만 확장하므로 32 픽셀 경계를 넘는 도형은 여러 명령으로 쪼개진다.
`vectorJsonToDocument(obj)`는 그 역방향이다. 캔버스와 층 배열을 검증한 뒤 명령을 `applyCommand`에 통과시키는데, `fill`·`color` 값이 `$`로 시작하면 팔레트에서 이름을 찾아 치환하고 없으면 검정으로 떨어진다. 층 id가 없거나 빈 문자열이면 위치에서 `layer_N`을 만들어 IdGen에 심는다.

### 5.2 serialize_recipe.js — 레시피 가져오기

`recipeJsonToDocument(obj, {layerKey, slotId, paletteOverrides})`는 브라우저에서 Node 트랜스파일러와 같은 결과를 내도록 네 가지로 분기한다. 순서는 표면 매니페스트 → 팬토그래프 프로파일 → 아틀라스 슬롯 → 표준 레시피다. 팬토그래프 시트는 슬롯 인덱스에 셀 너비를 곱한 만큼 `shiftCommand`로 명령을 밀어 한 장에 이어 붙인다.
`shiftCommand(cmd, dx, dy)`는 명령 하나의 기하 필드(`box`·`bbox`·`center`·`points`·`pts`·`pen`·`xy`·`pos`·`segments`·`lines`·재귀 `shape`)를 모두 평행 이동한다. 이 두 함수가 `serialize.js`에 다시 노출되는 이유는 호출측이 한 경로만 알게 하려는 배럴 규칙 때문이다.

## 6. export_png.js — 평탄화와 PNG 내보내기

`flattenToRgba(doc, {includeBackground=true})`는 배경을 바닥에 깔고 보이는 층을 아래부터 위로 `over()` 합성한다. 배경 포함이 참이면 캔버스 배경색을 파싱하고, transparent이거나 꺼져 있으면 투명 검정으로 시작한다. 잘못된 배경 문자열은 `INVALID_STATE`를 던진다. 보이지 않는 층과 불투명도 0인 층, 저장소가 없는 층은 건너뛴다. 불투명 알파 255에 층 불투명도 1인 픽셀은 합성 함수 없이 바로 덮어써서 속도를 낸다. 그 외에는 core의 `over()`에 목적지·출발지·불투명도를 넘겨 혼합한다. 최종 알파가 0이면 RGB도 0으로 정규화한다.
`exportPngBytes(doc, opts)`는 평탄화 결과를 그대로 `encodePng`에 넘긴다. 캔버스 전체 크기로 인코딩하므로 호출자는 문서만 넘기면 된다.

## 7. file_io.js — 다섯 가지 파일 함수와 64MiB 제한

`sanitizeFileName(name)`은 금지 문자와 제어 문자를 밑줄로 바꾸고 앞뒤 공백과 점을 제거한 뒤 최대 80자로 자른다. 결과가 비면 `untitled`을 돌려준다.
`saveTextFile(suggestedName, text, mime)`과 `saveBinaryFile(suggestedName, bytes, mime)`은 내부적으로 같은 저장 경로를 쓴다. File System Access의 저장 피커가 있으면 그곳에 쓰고, 사용자가 취소하면 false를 돌려준다. 피커가 없으면 Blob을 만들어 앵커 다운로드로 저장한다. 문서 환경 자체가 없으면 `INVALID_STATE`를 던진다.
`pickFile(accept)`은 열기 피커가 있으면 단일 파일을 받아 File 객체로 돌려주고, 취소하면 null을 돌려준다. 피커가 없으면 숨은 파일 입력 요소를 만들어 같은 동작을 흉내 낸다. 어떤 API도 없으면 `INVALID_STATE`다.
`readJsonFile(file)`은 크기 필드와 텍스트 읽기 함수가 없으면 `SCHEMA`로 거절한다. 크기가 64MiB를 넘으면 `FILE_TOO_LARGE`로 먼저 차단하고 파일을 읽지 않는다. 이후 텍스트를 JSON으로 파싱하고 실패하면 `SCHEMA`로 던진다.

## 8. 자동저장 — AutosaveStore와 네 위임 모듈

### 8.1 store_idb.js — 세션 바인딩과 dirty 추적

`AutosaveStore`는 세션 이벤트를 구독하는 자동저장 본체다. `static open()`은 IndexedDB가 없으면 null을 돌려주고, 열기 실패도 null로 흡수해서 호출자가 저장소 없이 동작하게 한다. `attach(session)`은 기존 구독을 끊고 현재 층 id 집합을 기록한 뒤 픽셀 변경·층 변경·문서 교체를 구독한다. 가시성 변경과 페이지 숨김에도 즉시 저장을 걸어 탭을 닫기 전 기록을 남긴다. `detach()`는 세션과 문서·전역 리스너를 모두 해제하고 스케줄러를 취소한다.
픽셀 변경은 층별 dirty 맵에 모은다. 전체 다시 쓰기 표시가 있으면 `all`로, 부분 변경이면 좌표 집합으로 누적한다. 층 구조 변경은 메타 dirty로, 문서 교체는 전체 다시 쓰기로 기록한다. `peek()`은 메타 요약만 읽어 문서 id·이름·갱신 시각을 돌려준다. `flushNow()`는 진행 중인 저장이 있으면 같은 약속을 공유하고, 아니면 내부 저장을 한 번 수행한다. 내부 저장은 메타 레코드를 만들고 단일 트랜잭션으로 쓴 뒤 dirty를 비운다. 용량 초과는 `STORAGE_QUOTA` 경고로, 그 외 실패는 `INVALID_STATE` 경고로 세션에 알린다.

### 8.2 idb_schema.js — 데이터베이스 뼈대

`openDatabase(idb)`는 이름 `draw_tool_v2`와 버전 1로 연다. 업그레이드 시 키 경로가 key인 meta 저장소와 같은 키 경로에 `byLayer` 인덱스를 가진 chunks 저장소를 만든다. `promisify(request)`는 IDB 요청의 성공과 실패를 약속으로 바꾼다.

### 8.3 idb_record_builder.js — 순수 레코드 생성

`chunkRecordKey(layerId, cx, cy)`는 층 id와 cy·cx를 파이프로 이은 안정 키를 만든다. cy를 앞에 두는 이유는 저장소 키 정렬과 직렬화 정렬 관행을 맞추기 위해서다.
`buildMetaRecord(doc)`는 키 current, 스키마 숫자 2, 문서 id·이름·캔버스·층 요약·활성층·현재 시각을 담은 메타 객체를 만든다.
`buildChunkRecords(layer, keys)`는 요청 좌표마다 청크 복사본을 꺼내 ArrayBuffer 형태로 담은 레코드 목록을 만든다. 없는 청크는 건너뛴다. 원본 버퍼를 그대로 넣지 않고 복사하는 이유는 저장 중 편집이 기록을 오염시키는 일을 막기 위해서다.

### 8.4 idb_scheduler.js — 800밀리초와 5초 스케줄

`FlushScheduler`는 디바운스와 강제 상한을 함께 둔다. 첫 변경 뒤 800밀리초 타이머를 걸고, 같은 타이머가 살아 있는 동안 추가 변경이 와도 타이머를 늘리지 않는다. 동시에 5초 강제 타이머를 걸어 계속 그리는 중에도 최대 5초마다 한 번은 저장하게 한다. 어느 쪽이 먼저 울리든 상대 타이머를 취소하고 콜백을 한 번만 호출한다. `cancel()`은 두 타이머를 모두 지운다.

### 8.5 idb_loader.js — 메타 부정합과 복원 규칙

`isMetaValid(meta)`는 스키마 숫자 2, 비어 있지 않은 문서 id, 유효한 캔버스 크기, 1개 이상 64개 이하 층 배열, 각 층의 id·이름·가시·잠금·불투명도 형태를 검사한다. 하나라도 어긋나면 false다.
`load(db)`는 메타와 청크를 읽기 트랜잭션으로 가져온다. 메타가 없으면 null을 돌려준다. 메타가 부정합하면 저장소 전체를 지우고 null을 돌려준다. 복원 불가 알림은 저장소가 아니라 UI가 맡는다. 청크 레코드는 층 id가 메타에 있을 때만 살리고, 좌표가 정수가 아니거나 데이터가 4096바이트 ArrayBuffer가 아니면 버린다. 활성층이 목록에 없으면 마지막 층을 활성층으로 삼는다. 문서 생성자가 실패해도 저장소를 지우고 null을 돌려준다.
`clear(db)`는 메타와 청크 저장소를 모두 비운다.

## 9. 대표 흐름 두 가지

저장과 열기 흐름은 검증이 가운데에 있다. 저장할 때는 문서 객체를 `documentToJson`으로 JSON화한 뒤 `saveTextFile`로 쓰고, 성공하면 히스토리에 저장 표시를 남긴다. 열 때는 `pickFile`로 고른 파일을 `readJsonFile`이 크기부터 검사하고 JSON으로 푼 뒤, `validateDocument`가 구조와 의미를 검사하고, `jsonToDocument`가 실제 픽셀로 복원한 뒤 세션에 적재한다. 레이어 가져오기는 `importLayerJson`이 새 층과 버린 청크 수, 경고를 돌려주면 호출자가 층 삽입과 안내 문구를 처리한다.
자동저장 흐름은 이벤트에서 시작한다. 픽셀 변경은 dirty 청크 집합에, 층 변경과 문서 교체는 메타와 전체 다시 쓰기 표시에 쌓인다. 스케줄러가 800밀리초 조용함을 기다리거나 5초 상한에 걸리면 한 트랜잭션으로 메타와 변경 청크를 함께 쓴다. 삭제된 층의 청크는 인덱스로 찾아 지우고, 전체 다시 쓰기 때는 저장소를 비운 뒤 메타와 전 층을 다시 채운다.

## 10. 브라우저와 Node 경계

브라우저 전용 접촉은 file_io와 자동저장, PNG 압축 경로에 모여 있다. file_io는 저장·열기 피커와 문서·URL 객체를 함수 안에서만 꺼내 쓰고, 모듈 최상위에서는 건드리지 않는다. 자동저장은 IndexedDB와 가시성·페이지 이벤트를 생성자와 구독 함수 안에서만 접근한다. PNG 압축은 WebStreams에 의존하므로 Node 테스트에서는 해당 경로를 모의하거나 건너뛰어야 한다. base64와 검증, 레코드 생성, 스케줄러는 양쪽에서 그대로 import할 수 있는 순수 계층이다. 이 분리의 목적은 Node에서 직렬화와 검증 단위 테스트를 브라우저 없이 돌리면서도, 브라우저에서는 같은 코드를 파일과 저장소에 그대로 붙이는 데 있다.

## 11. 파일 구성과 회귀 사항

`src/io/`는 15개 파일이다. 2026-10-08 `serialize.js`가 래스터·벡터·레시피 세 책임을
한 파일에 들고 있어 573줄이었는데, `serialize_vector.js`와 `serialize_recipe.js`로
갈라내고 `serialize.js`는 배럴 겸 래스터 절반만 남겼다. 호출측 import 경로는 그대로다.

| 파일 | 줄 | 비고 |
|:---|:---:|:---|
| `store_idb.js` | 245 | 분리 후 코어. `AutosaveStore` + `_INTERNALS` 재export |
| `serialize.js` | 246 | 분리 후 래스터 절반 + 벡터/레시피 재export 배럴 |
| `serialize_recipe.js` | 168 | 분리됨 (표면/팬토그래프/아틀라스/표준 레시피) |
| `serialize_vector.js` | 156 | 분리됨 (청크→명령, 명령→문서) |
| `validate_structural.js` | 139 | 분리됨 |
| `png.js` | 138 | 단일 책임 |
| `export_png.js` | 105 | 단일 책임 |
| `file_io.js` | 96 | 단일 책임 |
| `validate.js` | 96 | 분리 후 진입점 |
| `idb_loader.js` | 72 | 분리됨 |
| `idb_scheduler.js` | 43 | 분리됨 |
| `base64.js` | 38 | 단일 책임 |
| `validate_semantic.js` | 38 | 분리됨 |
| `idb_record_builder.js` | 37 | 분리됨 |
| `idb_schema.js` | 33 | 분리됨 |

### 해결된 회귀反倒 regress — 해소됨 (2026-10-01)

분리 후 `store_idb.js`는 `buildMetaRecord`·`buildChunkRecords`·`chunkRecordKey`를
`idb_record_builder.js`에서 **import만** 하고 재노출하지 않는다. `tests/io_basic.test.mjs:16`이
구 경로에서 이 세 심볼을 가져오려 하여 아래 오류로 모듈 로드 자체가 실패했다.

```
SyntaxError: The requested module '../src/io/store_idb.js' does not provide
an export named 'buildChunkRecords'
```

테스트 파일 전체가 로드되지 않아 io 계층 검증 35건이 **0건 실행**되던 상태였다.
**해소**: 테스트 import 경로를 `../src/io/idb_record_builder.js`로 변경했다.
파사드 re-export를 `store_idb.js`에 추가하는 대안도 있었으나, §5의 계층 규칙상
테스트 경로 수정이 더 순수하므로 이를 택했다. 현재 io 계층 35/35 통과.

## Handoff
- **Wrote**: `draw_tool_v2/docs/io.md`
- **Result**: 분리 후 13파일 라인 수 표 + `io_basic.test.mjs` 회귀 해소 기록
- **Next**: 없음. io 계층 검증 35/35 통과
