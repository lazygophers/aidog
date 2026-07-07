#!/usr/bin/env python3
"""读 platform-presets.json + models.json → 生成单 HTML（内嵌 JSON + vanilla JS + 内联 CSS）。

零第三方依赖（stdlib only）。HTML 由 JS 运行时渲染，初始只画平台卡片。
"""
import html
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PRESETS = ROOT / "src-tauri/defaults/platform-presets.json"
MODELS = ROOT / "src-tauri/defaults/models.json"
OUT = ROOT / ".aidoc/presets.html"

# i18n 回退顺序：zh-Hans → en-US → 第一可用
NAME_FALLBACK = ("zh-Hans", "en-US")


def i18n(d, key_hint=""):
    """从 i18n dict 取 zh-Hans，否则 en-US，否则 key_hint。"""
    if not isinstance(d, dict):
        return key_hint
    for k in NAME_FALLBACK:
        if d.get(k):
            return d[k]
    for v in d.values():
        if v:
            return v
    return key_hint


def per_m(tok):
    """per-token → $/M tokens，None → None。"""
    if tok is None:
        return None
    return tok * 1_000_000


def fmt_price(v):
    if v is None:
        return None
    if v < 0.01:
        return f"{v:.4f}"
    if v < 1:
        return f"{v:.3f}"
    return f"{v:.2f}"


def resolve_price(model_id, protocol_key, models):
    """返回 (input_M, output_M, cache_M, source, meta)。source ∈ override|top-level|no_price。"""
    m = models.get(model_id)
    if not m:
        return None, None, None, "no_price", {}
    meta = {
        "default_platform": m.get("default_platform"),
        "max_input_tokens": m.get("max_input_tokens"),
        "max_output_tokens": m.get("max_output_tokens"),
        "context_window": m.get("context_window"),
    }
    override = (m.get("pricing") or {}).get(protocol_key)
    if override:
        return (
            per_m(override.get("input_cost_per_token")),
            per_m(override.get("output_cost_per_token")),
            per_m(override.get("cache_read_input_token_cost")),
            "override",
            meta,
        )
    top_input = m.get("input_cost_per_token")
    if top_input is None and m.get("output_cost_per_token") is None:
        return None, None, None, "no_price", meta
    return (
        per_m(top_input),
        per_m(m.get("output_cost_per_token")),
        per_m(m.get("cache_read_input_token_cost")),
        "top-level",
        meta,
    )


def build_view_data():
    presets = json.loads(PRESETS.read_text(encoding="utf-8"))
    models_doc = json.loads(MODELS.read_text(encoding="utf-8"))
    models = models_doc.get("models", {})

    protocols = []
    for key, p in (presets.get("protocols") or {}).items():
        endpoints_default = [
            {
                "protocol": e.get("protocol"),
                "base_url": e.get("base_url"),
                "client_type": e.get("client_type"),
                "coding_plan": bool(e.get("coding_plan")),
            }
            for e in (p.get("endpoints") or {}).get("default", [])
        ]
        endpoints_cp = [
            {
                "protocol": e.get("protocol"),
                "base_url": e.get("base_url"),
                "client_type": e.get("client_type"),
                "coding_plan": True,
            }
            for e in (p.get("endpoints") or {}).get("coding_plan", [])
        ]
        model_ids = ((p.get("model_list") or {}).get("default") or [])
        model_rows = []
        for mid in model_ids:
            inp, outp, cache, src, meta = resolve_price(mid, key, models)
            model_rows.append({
                "id": mid,
                "default_platform": meta.get("default_platform"),
                "input_M": inp,
                "output_M": outp,
                "cache_M": cache,
                "source": src,
                "max_input": meta.get("max_input_tokens"),
                "max_output": meta.get("max_output_tokens"),
                "context_window": meta.get("context_window"),
            })
        protocols.append({
            "key": key,
            "name": i18n(p.get("name"), key),
            "desc": i18n(p.get("desc"), ""),
            "client_type": p.get("client_type"),
            "homepage": p.get("homepage"),
            "logo_url": p.get("logo_url"),
            "source_urls": p.get("source_urls") or {},
            "endpoints_default": endpoints_default,
            "endpoints_coding_plan": endpoints_cp,
            "models": model_rows,
            "model_count": len(model_rows),
            "has_override": any(r["source"] == "override" for r in model_rows),
            "min_input_M": min(
                (r["input_M"] for r in model_rows if r["input_M"] is not None),
                default=None,
            ),
        })

    return {
        "presets_version": presets.get("version"),
        "presets_last_updated": presets.get("last_updated"),
        "models_version": models_doc.get("version"),
        "models_generated_at": models_doc.get("generated_at"),
        "protocols": protocols,
    }


HTML_TEMPLATE = """<!doctype html>
<html lang="zh-Hans">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>aidog · 平台预设可视化</title>
<style>
  :root {
    --bg: #0f1115;
    --panel: rgba(255,255,255,0.04);
    --panel-hover: rgba(255,255,255,0.07);
    --border: rgba(255,255,255,0.10);
    --text: #e6e9ef;
    --muted: #8a93a3;
    --accent: #6aa6ff;
    --accent-2: #b48cff;
    --green: #5ad1a0;
    --amber: #f0c674;
    --red: #ff7b8a;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    background: radial-gradient(1200px 800px at 20% -10%, #1a1f2c 0%, var(--bg) 60%);
    color: var(--text);
    font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", sans-serif;
    min-height: 100vh;
  }
  header.topbar {
    position: sticky; top: 0; z-index: 10;
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    background: rgba(15,17,21,0.6);
    border-bottom: 1px solid var(--border);
    padding: 14px 24px;
    display: flex; gap: 12px; align-items: center; flex-wrap: wrap;
  }
  header.topbar h1 { font-size: 16px; margin: 0 16px 0 0; font-weight: 600; }
  header.topbar .meta { color: var(--muted); font-size: 12px; margin-right: auto; }
  header.topbar input[type=search] {
    background: var(--panel); border: 1px solid var(--border); color: var(--text);
    padding: 7px 12px; border-radius: 8px; min-width: 220px; outline: none;
  }
  header.topbar input[type=search]:focus { border-color: var(--accent); }
  header.topbar select, header.topbar label {
    background: var(--panel); border: 1px solid var(--border); color: var(--text);
    padding: 6px 10px; border-radius: 8px; font-size: 13px;
  }
  header.topbar label { cursor: pointer; display: inline-flex; align-items: center; gap: 6px; }
  header.topbar label input { accent-color: var(--accent); }

  main { padding: 20px 24px 80px; max-width: 1400px; margin: 0 auto; }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 14px;
  }
  .card {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 14px;
    overflow: hidden;
    transition: background .15s, border-color .15s;
  }
  .card:hover { background: var(--panel-hover); }
  .card.expanded { border-color: rgba(106,166,255,0.4); }
  .card.hidden { display: none; }
  .card-head {
    padding: 14px 16px;
    cursor: pointer;
    display: flex; align-items: flex-start; gap: 12px;
  }
  .logo {
    width: 36px; height: 36px; border-radius: 9px; flex-shrink: 0;
    background: #1d2230; display: grid; place-items: center;
    color: var(--muted); font-size: 14px; font-weight: 600;
    overflow: hidden;
  }
  .logo img { width: 22px; height: 22px; object-fit: contain; }
  .head-main { flex: 1; min-width: 0; }
  .head-row { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; }
  .name { font-weight: 600; font-size: 15px; }
  .proto { color: var(--muted); font-size: 12px; font-family: ui-monospace, Menlo, monospace; }
  .badge {
    display: inline-block; font-size: 10.5px; padding: 2px 7px; border-radius: 5px;
    background: rgba(106,166,255,0.14); color: var(--accent);
    border: 1px solid rgba(106,166,255,0.25);
  }
  .badge.cp { background: rgba(180,140,255,0.14); color: var(--accent-2); border-color: rgba(180,140,255,0.25); }
  .badge.override { background: rgba(90,209,160,0.14); color: var(--green); border-color: rgba(90,209,160,0.3); }
  .desc { color: var(--muted); font-size: 12.5px; margin-top: 4px; }
  .links { margin-top: 6px; display: flex; gap: 10px; font-size: 11.5px; }
  .links a { color: var(--accent); text-decoration: none; opacity: 0.85; }
  .links a:hover { opacity: 1; text-decoration: underline; }
  .endpoints { margin-top: 8px; font-size: 11.5px; color: var(--muted); }
  .ep { font-family: ui-monospace, Menlo, monospace; }
  .chev { color: var(--muted); transition: transform .2s; align-self: center; }
  .card.expanded .chev { transform: rotate(90deg); }
  .card-body { display: none; border-top: 1px solid var(--border); padding: 8px 12px 12px; }
  .card.expanded .card-body { display: block; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  th, td { padding: 6px 8px; text-align: left; vertical-align: top; }
  th { color: var(--muted); font-weight: 500; font-size: 11px; text-transform: uppercase; letter-spacing: .04em; }
  tr:hover td { background: rgba(255,255,255,0.03); }
  td.num, th.num { text-align: right; font-family: ui-monospace, Menlo, monospace; white-space: nowrap; }
  .src-tag { font-size: 10px; padding: 1px 6px; border-radius: 4px; margin-left: 4px; }
  .src-override { background: rgba(90,209,160,0.16); color: var(--green); }
  .src-toplevel { background: rgba(255,255,255,0.08); color: var(--muted); }
  .src-noprice { background: rgba(255,123,138,0.16); color: var(--red); }
  .empty { color: var(--muted); padding: 40px; text-align: center; }
  code.k { font-family: ui-monospace, Menlo, monospace; color: var(--accent-2); }
</style>
</head>
<body>
<header class="topbar">
  <h1>📦 平台预设</h1>
  <span class="meta" id="meta"></span>
  <input id="search" type="search" placeholder="搜索 平台 / 模型 / protocol…" autofocus>
  <select id="sort">
    <option value="name">按名称 A-Z</option>
    <option value="models">按模型数 ↓</option>
    <option value="min_input">按最低 input 价 ↑</option>
  </select>
  <label><input type="checkbox" id="only-override"> 仅显示有 override 的平台</label>
</header>
<main>
  <div id="count" style="color:var(--muted);font-size:12.5px;margin:0 0 14px 2px;"></div>
  <div class="grid" id="grid"></div>
</main>
<script type="application/json" id="view-data">__DATA__</script>
<script>
const DATA = JSON.parse(document.getElementById('view-data').textContent);
const grid = document.getElementById('grid');
const searchEl = document.getElementById('search');
const sortEl = document.getElementById('sort');
const onlyOverrideEl = document.getElementById('only-override');
const countEl = document.getElementById('count');
const metaEl = document.getElementById('meta');

metaEl.textContent = DATA.protocols.length + ' 协议 · models v' + (DATA.models_version ?? '?');

function esc(s) {
  if (s == null) return '';
  return String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}
function fmtNum(n) {
  if (n == null) return '—';
  if (n >= 1000) return (n/1000).toFixed(0) + 'K';
  return String(n);
}
function fmtPrice(v) {
  if (v == null) return '—';
  if (v < 0.01) return v.toFixed(4);
  if (v < 1) return v.toFixed(3);
  return v.toFixed(2);
}

function logoHtml(p) {
  const name = p.logo_url;
  if (name && /^[a-z0-9-]+$/i.test(name)) {
    // simpleicons CDN（slug 匹配）
    return '<img src="https://cdn.simpleicons.org/' + encodeURIComponent(name) + '" alt="">';
  }
  return esc((p.name || '?').slice(0,1).toUpperCase());
}

function cardHtml(p) {
  const cp = p.endpoints_coding_plan.length > 0;
  const eps = p.endpoints_default.concat(p.endpoints_coding_plan)
    .map(e => '<span class="ep"><span class="badge ' + (e.coding_plan?'cp':'') + '">' + esc(e.protocol) + (e.coding_plan?' · cp':'') + '</span> ' + esc(e.base_url) + '</span>')
    .join('<br>');
  const links = [];
  if (p.homepage) links.push('<a href="' + esc(p.homepage) + '" target="_blank">官网</a>');
  if (p.source_urls.pricing) links.push('<a href="' + esc(p.source_urls.pricing) + '" target="_blank">定价</a>');
  if (p.source_urls.docs) links.push('<a href="' + esc(p.source_urls.docs) + '" target="_blank">文档</a>');

  const rows = p.models.map(m => {
    const srcCls = m.source === 'override' ? 'src-override' : (m.source === 'top-level' ? 'src-toplevel' : 'src-noprice');
    const srcLabel = m.source === 'override' ? 'override' : (m.source === 'top-level' ? 'top-level' : 'no_price');
    const dpBadge = m.default_platform ? '<span class="badge">' + esc(m.default_platform) + '</span>' : '';
    return '<tr>'
      + '<td><code class="k">' + esc(m.id) + '</code> ' + dpBadge + '</td>'
      + '<td class="num">' + fmtPrice(m.input_M) + '</td>'
      + '<td class="num">' + fmtPrice(m.output_M) + '</td>'
      + '<td class="num">' + fmtPrice(m.cache_M) + '</td>'
      + '<td class="num">' + fmtNum(m.max_input) + '</td>'
      + '<td class="num">' + fmtNum(m.max_output) + '</td>'
      + '<td class="num">' + fmtNum(m.context_window) + '</td>'
      + '<td><span class="src-tag ' + srcCls + '">' + srcLabel + '</span></td>'
      + '</tr>';
  }).join('');

  return '<div class="card" data-key="' + esc(p.key) + '">'
    + '<div class="card-head">'
      + '<div class="logo">' + logoHtml(p) + '</div>'
      + '<div class="head-main">'
        + '<div class="head-row">'
          + '<span class="name">' + esc(p.name) + '</span>'
          + '<span class="proto">' + esc(p.key) + '</span>'
          + (p.client_type ? '<span class="badge">' + esc(p.client_type) + '</span>' : '')
          + (cp ? '<span class="badge cp">coding_plan</span>' : '')
          + (p.has_override ? '<span class="badge override">override</span>' : '')
          + '<span class="badge">models: ' + p.model_count + '</span>'
        + '</div>'
        + (p.desc ? '<div class="desc">' + esc(p.desc) + '</div>' : '')
        + (links.length ? '<div class="links">' + links.join('') + '</div>' : '')
        + (eps ? '<div class="endpoints">' + eps + '</div>' : '')
      + '</div>'
      + '<span class="chev">▶</span>'
    + '</div>'
    + '<div class="card-body">'
      + (rows ? '<table>'
        + '<thead><tr>'
        + '<th>模型</th><th class="num">input $/M</th><th class="num">output $/M</th><th class="num">cache $/M</th>'
        + '<th class="num">max_in</th><th class="num">max_out</th><th class="num">ctx</th><th>来源</th>'
        + '</tr></thead><tbody>' + rows + '</tbody></table>'
        : '<div class="empty">无 model_list.default</div>')
    + '</div>'
    + '</div>';
}

function applyFilter() {
  const q = searchEl.value.trim().toLowerCase();
  const onlyOv = onlyOverrideEl.checked;
  const sortKey = sortEl.value;
  const proto = DATA.protocols.slice();

  proto.sort((a, b) => {
    if (sortKey === 'name') return (a.name || '').localeCompare(b.name || '');
    if (sortKey === 'models') return b.model_count - a.model_count;
    if (sortKey === 'min_input') {
      const av = a.min_input_M == null ? Infinity : a.min_input_M;
      const bv = b.min_input_M == null ? Infinity : b.min_input_M;
      return av - bv;
    }
    return 0;
  });

  let shown = 0;
  grid.innerHTML = '';
  for (const p of proto) {
    if (onlyOv && !p.has_override) continue;
    const hit = !q
      || p.name.toLowerCase().includes(q)
      || p.key.toLowerCase().includes(q)
      || p.models.some(m => m.id.toLowerCase().includes(q));
    if (!hit) continue;
    grid.insertAdjacentHTML('beforeend', cardHtml(p));
    shown++;
  }
  countEl.textContent = shown + ' / ' + DATA.protocols.length + ' 平台';
}

grid.addEventListener('click', (e) => {
  const head = e.target.closest('.card-head');
  if (!head) return;
  head.parentElement.classList.toggle('expanded');
});

searchEl.addEventListener('input', applyFilter);
sortEl.addEventListener('change', applyFilter);
onlyOverrideEl.addEventListener('change', applyFilter);
applyFilter();
</script>
</body>
</html>
"""


def render(view_data):
    payload = json.dumps(view_data, ensure_ascii=False)
    # ponytail: <script type="application/json"> 的 textContent 不反转 HTML 实体，
    # html.escape 会把 " 转 &quot; 致浏览器 JSON.parse 炸。
    # 仅 escape "<" 防 </script> 注入；< 不影响 JSON.parse（JS 解码 < 回 <）。
    return HTML_TEMPLATE.replace("__DATA__", payload.replace("<", "\\u003c"))


def main():
    if not PRESETS.exists():
        sys.exit(f"missing: {PRESETS}")
    if not MODELS.exists():
        sys.exit(f"missing: {MODELS}")
    data = build_view_data()
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(render(data), encoding="utf-8")
    print(f"✓ {OUT} ({OUT.stat().st_size} bytes, {len(data['protocols'])} protocols)")


if __name__ == "__main__":
    main()
