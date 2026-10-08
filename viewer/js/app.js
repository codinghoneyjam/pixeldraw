// Layer Gallery - 34종 x (레이어 / 레이어로 만든 PNG / 기존 작업) 검증 뷰어
//
// 데이터는 tools/build_layer_gallery.mjs 가 생성한 viewer/js/gallery_data.js.
// 이 파일은 뷰만 담당한다 (비교 연산은 comparator.js, 데이터는 gallery_data.js).

const GROUP_LABELS = {
  all: '전체',
  entity: '엔티티 (적·플레이어·무기)',
  world: '월드',
  keycap: '키캡',
  ui: 'UI',
  title: '타이틀',
};

const MODES = {
  render: { label: '트랜스파일러', path: (a) => a.render.path, cmp: (a) => a.render },
  layers: { label: '레이어 합성', path: (a) => a.fromLayers.path, cmp: (a) => a.fromLayers },
  diff: { label: '델타', path: (a) => a.diff, cmp: (a) => a.fromLayers },
};

const esc = (s) =>
  String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const pct = (v) => `${(v * 100).toFixed(2)}%`;

class LayerGallery {
  constructor() {
    this.data = LAYER_GALLERY;
    this.assets = this.data.assets;
    this.group = 'all';
    this.query = '';
    // 카드별 UI 상태: 카드 id -> { mode, unit }
    this.state = new Map(this.assets.map((a) => [a.key, { mode: 'render', unit: null }]));
    this.deltaCache = new Map();
    this.init();
  }

  init() {
    this.renderSummary();
    this.renderTabs();
    this.renderCards();
    this.bindEvents();
  }

  // <META - ROLE : Render the header totals and the pass/fail gate line | L52-70>
  renderSummary() {
    const t = this.data.totals;
    const items = [
      { v: `${t.assets}`, c: '', l: 'ACTIVE 에셋 (numbered 11-44)' },
      { v: `${t.withLayers}/${t.assets}`, c: 'violet', l: '레이어 변환 완료' },
      { v: `${t.parityPass}/${t.assets}`, c: 'green', l: '기존 작업과 99% 일치' },
      { v: `${t.distinctUnits}`, c: 'violet', l: '추출된 레이어 유닛' },
      { v: pct(t.minParity), c: 'green', l: '최저 패리티' },
    ];
    document.getElementById('summary').innerHTML = items
      .map(
        (i) =>
          `<div class="summary-item"><div class="summary-value ${i.c}">${i.v}</div>` +
          `<div class="summary-label">${i.l}</div></div>`,
      )
      .join('');

    const gate = document.getElementById('gateLine');
    if (this.data.problems.length === 0) {
      gate.innerHTML =
        `<span class="ok">게이트 통과</span> — 34/34 레이어 전환, 전 항목 패리티 >= ` +
        `${this.data.threshold} · 기준 ${esc(this.data.generatedBy)}`;
    } else {
      gate.innerHTML =
        `<span class="bad">게이트 실패</span> — ` +
        this.data.problems.map((p) => `<br>${esc(p)}`).join('');
    }
  }

  renderTabs() {
    const counts = { all: this.assets.length };
    for (const a of this.assets) counts[a.group] = (counts[a.group] ?? 0) + 1;
    const ids = ['all', ...Object.keys(GROUP_LABELS).filter((k) => k !== 'all' && counts[k])];
    const box = document.getElementById('categoryTabs');
    box.innerHTML = ids
      .map(
        (id) =>
          `<button class="tab-btn ${id === this.group ? 'active' : ''}" data-group="${id}">` +
          `${GROUP_LABELS[id]}<span class="count">${counts[id]}</span></button>`,
      )
      .join('');
  }

  filterAssets() {
    const q = this.query.trim().toLowerCase();
    return this.assets.filter((a) => {
      if (this.group !== 'all' && a.group !== this.group) return false;
      if (!q) return true;
      const units = (a.layerSet?.units ?? []).map((u) => u.unit).join(' ');
      return `${a.key} ${a.name} ${a.no} ${a.layerSet?.slug ?? ''} ${units}`
        .toLowerCase()
        .includes(q);
    });
  }

  renderCards() {
    const grid = document.getElementById('assetGrid');
    const list = this.filterAssets();
    if (list.length === 0) {
      grid.innerHTML = '<div class="empty">조건에 맞는 에셋이 없습니다.</div>';
      return;
    }
    grid.innerHTML = list.map((a) => this.cardHtml(a)).join('');
  }

  // <META - ROLE : One asset card: layer pane, render pane, legacy pane | L116-210>
  cardHtml(a) {
    const st = this.state.get(a.key);
    const cmp = MODES[st.mode].cmp(a);
    const pass = cmp.matchFraction >= this.data.threshold;
    const units = a.layerSet?.units ?? [];
    const usedCount = units.filter((u) => u.used).length;

    const chips = units
      .map(
        (u) =>
          `<div class="layer-chip ${u.used ? '' : 'unused'} ${st.unit === u.unit ? 'selected' : ''}"
                data-key="${esc(a.key)}" data-unit="${esc(u.unit)}"
                title="${esc(u.unit)} · ${esc(u.png)}">
             <img src="${esc(u.png)}" alt="${esc(u.unit)}" loading="lazy">
             <div class="u-name">${esc(u.unit)}</div>
           </div>`,
      )
      .join('');

    const docs = units
      .map(
        (u) =>
          `<a class="doc-link" href="${esc(u.doc)}" title="${esc(u.doc)}">${esc(u.unit)}.json</a>`,
      )
      .join('');

    const srcBtns = Object.entries(MODES)
      .map(
        ([id, m]) =>
          `<button class="src-btn ${st.mode === id ? 'active' : ''}"
                   data-key="${esc(a.key)}" data-mode="${id}">${m.label}</button>`,
      )
      .join('');

    const unit = st.unit ? units.find((u) => u.unit === st.unit) : null;
    const shownSrc = unit ? unit.png : MODES[st.mode].path(a);
    const meta = [
      `U${a.no}`,
      `${a.size.w}×${a.size.h}`,
      `${units.length} 레이어`,
      `recipe: ${a.recipe?.rel ?? '—'}`,
      a.recipe?.paletteOverride ? '팔레트 오버라이드' : null,
      `case: ${a.recipe?.caseId ?? '—'} (${a.recipe?.resolvedBy ?? '—'})`,
    ].filter(Boolean);

    return `
      <div class="asset-card" data-key="${esc(a.key)}">
        <div class="card-header">
          <div>
            <div class="card-title"><span class="uid">U${a.no}</span>${esc(a.name)}</div>
            <div class="card-category">${GROUP_LABELS[a.group] ?? a.group} · ${esc(a.key)}</div>
          </div>
          <div class="parity-pill ${pass ? 'pass' : 'fail'}">${pct(cmp.matchFraction)}</div>
        </div>

        <div class="triple">
          <div class="pane">
            <div class="pane-head">
              <span class="n">① 레이어</span>
              <span class="tag layer">${units.length}개 · 합성 ${usedCount}개</span>
            </div>
            <div class="layer-list">${chips || '<span class="empty">레이어 없음</span>'}</div>
            <div class="layer-doc-links">${docs}</div>
          </div>

          <div class="pane">
            <div class="pane-head">
              <span class="n">② 레이어로 만든 PNG</span>
              <span class="src-toggle">${srcBtns}</span>
            </div>
            <div class="image-box checkerboard">
              <img src="${esc(shownSrc)}" alt="${esc(a.key)} render" loading="lazy">
              <span class="dims">${a.size.w}×${a.size.h}</span>
            </div>
            <div class="pane-foot">
              <span>${unit ? `레이어 <b>${esc(unit.unit)}</b> 단독` : esc(MODES[st.mode].label)}</span>
              <span>패리티 ${pct(cmp.matchFraction)}</span>
              <span>${esc(cmp.classification)}</span>
              <span>maxΔ ${cmp.maxDelta}</span>
            </div>
            <button class="delta-btn" data-key="${esc(a.key)}">델타 계산 (기존 작업 대비)</button>
            <div class="delta-container" id="delta-${esc(a.key)}" style="display:none"></div>
          </div>

          <div class="pane">
            <div class="pane-head">
              <span class="n">③ 기존 작업</span>
              <span class="tag legacy">numbered/${esc(a.key)}.png</span>
            </div>
            <div class="image-box checkerboard">
              <img src="${esc(a.legacy.path)}" alt="${esc(a.key)} legacy" loading="lazy">
              <span class="dims">${a.size.w}×${a.size.h}</span>
            </div>
            <div class="pane-foot">
              <span>${esc(a.legacy.path.split('/').pop())}</span>
              <span>${a.legacy.bytes} B</span>
              <span>sha ${esc(a.sha.slice(0, 8))}</span>
            </div>
          </div>
        </div>

        <div class="card-foot">
          <div class="meta-row">${meta.map((m) => `<span class="mono">${esc(m)}</span>`).join('')}</div>
          ${(a.notes ?? []).map((n) => `<div class="note">${esc(n)}</div>`).join('')}
        </div>
      </div>`;
  }

  bindEvents() {
    document.getElementById('categoryTabs').addEventListener('click', (e) => {
      const btn = e.target.closest('.tab-btn');
      if (!btn) return;
      this.group = btn.dataset.group;
      this.renderTabs();
      this.renderCards();
    });

    const search = document.getElementById('search');
    search.addEventListener('input', (e) => {
      this.query = e.target.value;
      this.renderCards();
    });

    document.getElementById('assetGrid').addEventListener('click', (e) => {
      const chip = e.target.closest('.layer-chip');
      if (chip) {
        const st = this.state.get(chip.dataset.key);
        st.unit = st.unit === chip.dataset.unit ? null : chip.dataset.unit;
        this.renderCards();
        return;
      }
      const srcBtn = e.target.closest('.src-btn');
      if (srcBtn) {
        const st = this.state.get(srcBtn.dataset.key);
        st.mode = srcBtn.dataset.mode;
        st.unit = null;
        this.renderCards();
        return;
      }
      const deltaBtn = e.target.closest('.delta-btn');
      if (deltaBtn) {
        this.toggleDelta(deltaBtn.dataset.key);
      }
    });
  }

  // <META - ROLE : Compute and toggle the per-asset delta view | L212-260>
  async toggleDelta(key) {
    const box = document.getElementById(`delta-${key}`);
    if (!box) return;
    if (box.style.display !== 'none') {
      box.style.display = 'none';
      return;
    }
    box.style.display = 'block';
    const asset = this.assets.find((a) => a.key === key);
    const st = this.state.get(key);
    const mode = st.mode === 'layers' ? 'layers' : 'render';
    const cacheKey = `${key}|${mode}`;
    if (!this.deltaCache.has(cacheKey)) {
      box.innerHTML = '<div class="empty">델타 계산 중…</div>';
      const target = MODES[mode].path(asset);
      this.deltaCache.set(cacheKey, await comparator.compareImages(asset.legacy.path, target));
    }
    this.renderDelta(box, this.deltaCache.get(cacheKey), asset, mode);
  }

  renderDelta(box, result, asset, mode) {
    const cmp = MODES[mode].cmp(asset);
    const legend = `
      <div class="delta-legend">
        <span class="legend-item"><span class="legend-color" style="background:rgba(239,68,68,0.85)"></span>다른 픽셀</span>
        <span class="legend-item"><span class="legend-color" style="background:rgba(148,163,184,0.4)"></span>일치 픽셀</span>
        <span class="legend-item">${esc(MODES[mode].label)} vs 기존 작업</span>
      </div>`;
    box.innerHTML = `
      <div class="delta-images">
        <div class="image-box checkerboard">
          <span class="parity-pill fail" style="position:absolute;top:.3rem;left:.3rem;font-size:.65rem">델타</span>
          <img src="${esc(asset.diff)}" alt="diff" loading="lazy">
        </div>
        <div class="image-box checkerboard">
          <span class="parity-pill pass" style="position:absolute;top:.3rem;left:.3rem;font-size:.65rem">사이드바이사이드</span>
          <canvas id="sbs-${esc(asset.key)}"></canvas>
        </div>
      </div>
      ${legend}
      <div class="pane-foot">
        <span>일치 ${pct(result.matchFraction)}</span>
        <span>서로 다름 ${(result.diffPixels).toLocaleString()} px</span>
        <span>maxΔ ${result.maxDelta}</span>
        <span>${esc(cmp.classification)}</span>
      </div>`;
    const canvas = box.querySelector(`#sbs-${CSS.escape(asset.key)}`);
    if (!canvas) return;
    const w = result.width;
    canvas.width = w * 2;
    canvas.height = result.height;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(result.legacyImg, 0, 0);
    ctx.drawImage(result.currentImg, w, 0);
  }
}

document.addEventListener('DOMContentLoaded', () => {
  new LayerGallery();
});