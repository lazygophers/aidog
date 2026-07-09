#!/usr/bin/env python3
"""读 platform-presets.json + models.json → 生成单 HTML（内嵌 JSON + vanilla JS + 内联 CSS）。

零第三方依赖（stdlib only）。HTML 由 JS 运行时渲染，初始只画平台卡片。
"""
import json
import statistics
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
    """返回 (input_M, output_M, cache_M, source, meta)。source ∈ override|top-level|no_price。

    meta 含 default_platform / max_*_tokens / context_window / context_tiers / pricing
    （后两者供模型详情弹窗展示全量定价 override 与分级 context 定价）。
    """
    m = models.get(model_id)
    if not m:
        return None, None, None, "no_price", {}
    meta = {
        "default_platform": m.get("default_platform"),
        "max_input_tokens": m.get("max_input_tokens"),
        "max_output_tokens": m.get("max_output_tokens"),
        "context_window": m.get("context_window"),
        "context_tiers": m.get("context_tiers") or [],
        "pricing": m.get("pricing") or {},
        "input_cost_per_token": m.get("input_cost_per_token"),
        "output_cost_per_token": m.get("output_cost_per_token"),
        "cache_read_input_token_cost": m.get("cache_read_input_token_cost"),
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


def _model_row(mid, protocol_key, models):
    """构造单个 model_row（含 context_tiers / pricing 全量供 modal）。"""
    inp, outp, cache, src, meta = resolve_price(mid, protocol_key, models)
    return {
        "id": mid,
        "default_platform": meta.get("default_platform"),
        "input_M": inp,
        "output_M": outp,
        "cache_M": cache,
        "source": src,
        "max_input": meta.get("max_input_tokens"),
        "max_output": meta.get("max_output_tokens"),
        "context_window": meta.get("context_window"),
        "context_tiers": meta.get("context_tiers"),
        "pricing": meta.get("pricing"),
    }


def build_view_data():
    presets = json.loads(PRESETS.read_text(encoding="utf-8"))
    models_doc = json.loads(MODELS.read_text(encoding="utf-8"))
    models = models_doc.get("models", {})

    protocols = []
    # 聚合统计累积器
    all_model_ids = set()           # 全局去重 model_id
    priced_model_ids = set()        # 有定价的 model_id
    all_input_M = []                # 全局 input $/M 分布
    protocols_with_peak = 0
    protocols_with_cp = 0           # is_coding_plan=True 的协议数
    protocols_with_override = 0
    client_types = set()

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

        # default 分支模型
        model_ids = ((p.get("model_list") or {}).get("default")) or []
        model_rows = [_model_row(mid, key, models) for mid in model_ids]

        # coding_plan 分支独立模型
        cp_model_ids = ((p.get("model_list") or {}).get("coding_plan")) or []
        cp_model_rows = [_model_row(mid, key, models) for mid in cp_model_ids]

        # peak_hours（per-protocol，UTC+0；absent = 无调整）
        peak_hours = p.get("peak_hours") or []
        if peak_hours:
            protocols_with_peak += 1

        # 槽位映射（default / coding_plan 分支）
        slots_default = ((p.get("models") or {}).get("default")) or {}
        slots_coding_plan = ((p.get("models") or {}).get("coding_plan")) or {}

        is_cp = bool(p.get("is_coding_plan"))
        if is_cp:
            protocols_with_cp += 1

        ct = p.get("client_type")
        if ct:
            client_types.add(ct)

        # 聚合
        for r in model_rows:
            all_model_ids.add(r["id"])
            if r["source"] != "no_price":
                priced_model_ids.add(r["id"])
            if r["input_M"] is not None:
                all_input_M.append(r["input_M"])
        for r in cp_model_rows:
            all_model_ids.add(r["id"])
            if r["source"] != "no_price":
                priced_model_ids.add(r["id"])
            if r["input_M"] is not None:
                all_input_M.append(r["input_M"])

        has_override = any(r["source"] == "override" for r in model_rows) or \
            any(r["source"] == "override" for r in cp_model_rows)
        if has_override:
            protocols_with_override += 1

        min_input = min(
            (r["input_M"] for r in model_rows if r["input_M"] is not None),
            default=None,
        )

        protocols.append({
            "key": key,
            "name": i18n(p.get("name"), key),
            "desc": i18n(p.get("desc"), ""),
            "client_type": ct,
            "homepage": p.get("homepage"),
            "logo_url": p.get("logo_url"),
            "source_urls": p.get("source_urls") or {},
            "endpoints_default": endpoints_default,
            "endpoints_coding_plan": endpoints_cp,
            "models": model_rows,
            "models_coding_plan": cp_model_rows,
            "model_count": len(model_rows),
            "has_override": has_override,
            "min_input_M": min_input,
            "peak_hours": peak_hours,
            "slots_default": slots_default,
            "slots_coding_plan": slots_coding_plan,
            "is_coding_plan": is_cp,
        })

    # 全局分布分位
    sorted_inputs = sorted(all_input_M)
    if len(sorted_inputs) >= 4:
        q = statistics.quantiles(sorted_inputs, n=4)
        p25, median_in, p75 = q[0], q[1], q[2]
    elif sorted_inputs:
        p25 = sorted_inputs[0]
        median_in = statistics.median(sorted_inputs)
        p75 = sorted_inputs[-1]
    else:
        p25 = median_in = p75 = None

    pricing_coverage_pct = (
        round(len(priced_model_ids) / len(all_model_ids) * 100, 1)
        if all_model_ids else 0.0
    )

    stats = {
        "total_protocols": len(protocols),
        "total_models": len(all_model_ids),
        "pricing_coverage_pct": pricing_coverage_pct,
        "protocols_with_peak": protocols_with_peak,
        "protocols_with_cp": protocols_with_cp,
        "protocols_with_override": protocols_with_override,
        "overall_min_input_M": sorted_inputs[0] if sorted_inputs else None,
        "overall_p25_input_M": p25,
        "overall_median_input_M": median_in,
        "overall_p75_input_M": p75,
        "overall_max_input_M": sorted_inputs[-1] if sorted_inputs else None,
    }

    return {
        "presets_version": presets.get("version"),
        "presets_last_updated": presets.get("last_updated"),
        "models_version": models_doc.get("version"),
        "models_generated_at": models_doc.get("generated_at"),
        "protocols": protocols,
        "stats": stats,
        "client_types": sorted(client_types),
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

  /* 聚合统计条 */
  .stats-bar {
    max-width: 1400px; margin: 0 auto; padding: 16px 24px 0;
  }
  .stats-card {
    background: var(--panel); border: 1px solid var(--border); border-radius: 12px;
    padding: 14px 18px; display: flex; flex-wrap: wrap; gap: 22px; align-items: center;
  }
  .stat-item { display: flex; flex-direction: column; gap: 2px; }
  .stat-item .label { color: var(--muted); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; }
  .stat-item .value { font-size: 18px; font-weight: 600; font-family: ui-monospace, Menlo, monospace; }
  .stat-item .value .unit { font-size: 11px; color: var(--muted); margin-left: 2px; font-weight: 400; }
  .stat-item.accent .value { color: var(--accent); }
  .stat-item.amber .value { color: var(--amber); }
  .stat-item.green .value { color: var(--green); }
  .stat-item.purple .value { color: var(--accent-2); }

  /* 全局 input 分布条 */
  .dist-wrap {
    max-width: 1400px; margin: 0 auto; padding: 10px 24px 0;
  }
  .dist-card {
    background: var(--panel); border: 1px solid var(--border); border-radius: 12px;
    padding: 12px 18px 16px;
  }
  .dist-title { color: var(--muted); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; margin-bottom: 8px; }
  .dist-bar {
    position: relative; height: 8px; border-radius: 4px;
    background: linear-gradient(90deg, var(--green) 0%, var(--amber) 50%, var(--red) 100%);
    margin: 18px 4px 8px;
  }
  .dist-tick {
    position: absolute; top: -6px; width: 1px; height: 20px; background: var(--text);
    transform: translateX(-50%);
  }
  .dist-tick::after {
    content: attr(data-label); position: absolute; top: -18px; left: 50%;
    transform: translateX(-50%); font-size: 10.5px; color: var(--text);
    white-space: nowrap; font-family: ui-monospace, Menlo, monospace;
  }
  .dist-tick.pct25 { background: var(--green); }
  .dist-tick.pct50 { background: var(--amber); }
  .dist-tick.pct75 { background: var(--red); }
  .dist-legend {
    display: flex; justify-content: space-between; color: var(--muted); font-size: 10.5px;
    font-family: ui-monospace, Menlo, monospace; margin-top: 4px;
  }

  main { padding: 16px 24px 80px; max-width: 1400px; margin: 0 auto; }
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
  .badge.peak { background: rgba(240,198,116,0.16); color: var(--amber); border-color: rgba(240,198,116,0.3); }
  .badge.iscp { background: rgba(255,123,138,0.14); color: var(--red); border-color: rgba(255,123,138,0.3); }
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
  .model-link { color: var(--accent-2); cursor: pointer; text-decoration: none; border-bottom: 1px dashed rgba(180,140,255,0.4); }
  .model-link:hover { color: var(--accent); border-bottom-color: var(--accent); }

  /* 子区段标题 */
  .subsec { margin-top: 14px; }
  .subsec-title {
    font-size: 11px; text-transform: uppercase; letter-spacing: .04em; color: var(--muted);
    margin: 0 0 4px; display: flex; align-items: center; gap: 6px;
  }
  .subsec-title.cp-title { color: var(--accent-2); }
  .subsec-title.peak-title { color: var(--amber); }

  /* 价格 bar cell */
  .price-bar-wrap {
    width: 90px; height: 8px; background: rgba(255,255,255,0.06); border-radius: 4px;
    overflow: hidden; display: inline-block; vertical-align: middle; margin-right: 6px;
  }
  .price-bar-fill { height: 100%; border-radius: 4px; }
  .price-bar-fill.low { background: var(--green); }
  .price-bar-fill.mid { background: var(--amber); }
  .price-bar-fill.high { background: var(--red); }

  /* 槽位映射 / peak_hours 紧凑表 */
  .compact td, .compact th { padding: 4px 6px; font-size: 11.5px; }
  .slot-name { font-family: ui-monospace, Menlo, monospace; color: var(--accent); }

  /* peak_hours 表 */
  .peak-mult { font-family: ui-monospace, Menlo, monospace; font-weight: 600; }
  .peak-mult.up { color: var(--red); }
  .peak-mult.down { color: var(--green); }
  .peak-scope { font-size: 10.5px; color: var(--muted); }

  /* coding_plan 独立子区视觉分隔 */
  .cp-section {
    margin-top: 14px; padding: 10px 12px; border-radius: 10px;
    background: rgba(180,140,255,0.05); border: 1px solid rgba(180,140,255,0.18);
  }

  /* 模态弹窗（单例，createPortal 等效：fixed inset + backdrop） */
  .modal-backdrop {
    position: fixed; inset: 0; z-index: 100;
    background: rgba(0,0,0,0.6); backdrop-filter: blur(6px);
    display: none; align-items: center; justify-content: center;
  }
  .modal-backdrop.open { display: flex; }
  .modal {
    background: #161922; border: 1px solid var(--border); border-radius: 14px;
    max-width: 720px; width: calc(100% - 40px); max-height: 84vh; overflow: auto;
    padding: 20px 24px;
  }
  .modal-head { display: flex; align-items: baseline; gap: 10px; margin-bottom: 12px; flex-wrap: wrap; }
  .modal-head h2 { margin: 0; font-size: 17px; font-family: ui-monospace, Menlo, monospace; color: var(--accent-2); }
  .modal-close {
    margin-left: auto; background: none; border: none; color: var(--muted);
    font-size: 20px; cursor: pointer; padding: 0 6px; line-height: 1;
  }
  .modal-close:hover { color: var(--text); }
  .modal-section { margin-top: 14px; }
  .modal-section h3 {
    margin: 0 0 6px; font-size: 11px; color: var(--muted);
    text-transform: uppercase; letter-spacing: .04em; font-weight: 500;
  }
  .kv { display: grid; grid-template-columns: max-content 1fr; gap: 4px 14px; font-size: 12.5px; }
  .kv .k { color: var(--muted); }
  .kv .v { font-family: ui-monospace, Menlo, monospace; }
  .used-by { display: flex; flex-wrap: wrap; gap: 4px; }
  .used-by .badge { font-size: 10.5px; }
</style>
</head>
<body>
<header class="topbar">
  <h1>📦 平台预设</h1>
  <span class="meta" id="meta"></span>
  <input id="search" type="search" placeholder="搜索 平台 / 模型 / 槽位 / peak scope…" autofocus>
  <select id="sort">
    <option value="name">按名称 A-Z</option>
    <option value="models">按模型数 ↓</option>
    <option value="min_input">按最低 input $/M ↑</option>
    <option value="output_M">按最低 output $/M ↑</option>
    <option value="ctx">按最大 ctx ↓</option>
    <option value="has_peak">有 peak 优先</option>
  </select>
  <select id="client-type">
    <option value="">所有 client_type</option>
  </select>
  <label><input type="checkbox" id="only-override"> 仅 override</label>
  <label><input type="checkbox" id="has-peak"> 有 peak_hours</label>
  <label><input type="checkbox" id="has-cp"> 有 coding_plan 端点</label>
  <label><input type="checkbox" id="is-cp"> is_coding_plan</label>
</header>
<div class="stats-bar" id="stats-bar"></div>
<div class="dist-wrap" id="dist-wrap"></div>
<main>
  <div id="count" style="color:var(--muted);font-size:12.5px;margin:0 0 14px 2px;"></div>
  <div class="grid" id="grid"></div>
</main>
<div class="modal-backdrop" id="modal-backdrop">
  <div class="modal" id="modal"></div>
</div>
<script type="application/json" id="view-data">__DATA__</script>
<script>
const DATA = JSON.parse(document.getElementById('view-data').textContent);
const grid = document.getElementById('grid');
const searchEl = document.getElementById('search');
const sortEl = document.getElementById('sort');
const clientTypeEl = document.getElementById('client-type');
const onlyOverrideEl = document.getElementById('only-override');
const hasPeakEl = document.getElementById('has-peak');
const hasCpEl = document.getElementById('has-cp');
const isCpEl = document.getElementById('is-cp');
const countEl = document.getElementById('count');
const metaEl = document.getElementById('meta');
const statsBarEl = document.getElementById('stats-bar');
const distWrapEl = document.getElementById('dist-wrap');
const modalBackdrop = document.getElementById('modal-backdrop');
const modalEl = document.getElementById('modal');

metaEl.textContent = DATA.stats.total_protocols + ' 协议 · models v' + (DATA.models_version ?? '?');

// 价格档阈值（全局分位）：绿 < p25(改用 median) / 黄 < p75 / 红 ≥ p75
const PRICE_TIERS = {
  median: DATA.stats.overall_median_input_M ?? 0,
  p75: DATA.stats.overall_p75_input_M ?? Infinity,
  max: DATA.stats.overall_max_input_M ?? 1,
};

// 构建 model_id → 出现在哪些 protocol 的索引（modal 的 "Used by" 区）
const modelProtocols = {}; // model_id -> [{key, name, branch}]
for (const p of DATA.protocols) {
  for (const m of p.models) {
    (modelProtocols[m.id] = modelProtocols[m.id] || []).push({key: p.key, name: p.name, branch: 'default'});
  }
  for (const m of (p.models_coding_plan || [])) {
    (modelProtocols[m.id] = modelProtocols[m.id] || []).push({key: p.key, name: p.name, branch: 'coding_plan'});
  }
}

// 填 client_type 下拉
(function() {
  const cts = DATA.client_types || [];
  for (const ct of cts) {
    const opt = document.createElement('option');
    opt.value = ct; opt.textContent = ct;
    clientTypeEl.appendChild(opt);
  }
})();

function esc(s) {
  if (s == null) return '';
  return String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}
function fmtNum(n) {
  if (n == null) return '—';
  if (n >= 1_000_000) return (n/1_000_000).toFixed(1) + 'M';
  if (n >= 1000) return (n/1000).toFixed(0) + 'K';
  return String(n);
}
function fmtTokens(n) {
  if (n == null) return '—';
  if (n >= 1_000_000) return (n/1_000_000).toFixed(n % 1_000_000 ? 1 : 0) + 'M tokens';
  if (n >= 1000) return (n/1000).toFixed(0) + 'K tokens';
  return n + ' tokens';
}
function fmtPrice(v) {
  if (v == null) return '—';
  if (v < 0.01) return v.toFixed(4);
  if (v < 1) return v.toFixed(3);
  return v.toFixed(2);
}
function priceTierClass(input_M) {
  if (input_M == null) return '';
  if (input_M >= PRICE_TIERS.p75) return 'high';
  if (input_M < PRICE_TIERS.median) return 'low';
  return 'mid';
}
function priceBarHtml(input_M) {
  if (input_M == null) return '<span class="peak-scope">—</span>';
  const max = PRICE_TIERS.max || 1;
  const pct = Math.max(2, Math.min(100, (input_M / max) * 100));
  const cls = priceTierClass(input_M);
  return '<span class="price-bar-wrap"><span class="price-bar-fill ' + cls + '" style="width:' + pct.toFixed(1) + '%"></span></span>'
    + '<span class="peak-scope">' + fmtPrice(input_M) + '</span>';
}

function logoHtml(p) {
  const name = p.logo_url;
  if (name && /^[a-z0-9-]+$/i.test(name)) {
    return '<img src="https://cdn.simpleicons.org/' + encodeURIComponent(name) + '" alt="">';
  }
  return esc((p.name || '?').slice(0,1).toUpperCase());
}

// peak_hours 窗口格式化（UTC+0，与 Rust gateway::peak_hours 一致，不转本地时区）
function fmtPeakWindow(w) {
  function pad(n) { return String(n).padStart(2, '0'); }
  const sH = w.start_hour ?? 0;
  const eH = w.end_hour ?? 0;
  const sM = w.start_minute ?? 0;
  const eM = w.end_minute ?? 0;
  let r = pad(sH) + ':' + pad(sM) + '-' + pad(eH) + ':' + pad(eM);
  // 跨天窗口（end < start，半开 [start, end)）
  if (eH < sH || (eH === sH && eM < sM)) {
    r += '+1';
  }
  return r + ' UTC+0';
}
function fmtPeakDays(w) {
  const dow = w.days_of_week;
  const dom = w.days_of_month;
  const dowNames = ['日','一','二','三','四','五','六'];
  let s = '每天';
  if (dow && dow.length) s = '周' + dow.map(i => dowNames[i] ?? i).join('/');
  if (dom && dom.length) s += '（' + dom.join(',') + ' 号）';
  return s;
}
function fmtPeakScope(w) {
  if (!w.models || !w.models.length) return '全平台模型';
  return w.models.join(', ');
}
function fmtPeakActive(w) {
  const parts = [];
  if (w.start_at != null) parts.push('起 ' + new Date(w.start_at * 1000).toISOString().slice(0,10));
  if (w.end_at != null) parts.push('止 ' + new Date(w.end_at * 1000).toISOString().slice(0,10));
  return parts.length ? parts.join(' · ') : '';
}

function peakTableHtml(p) {
  if (!p.peak_hours || !p.peak_hours.length) return '';
  const rows = p.peak_hours.map(w => {
    const mult = w.multiplier;
    const cls = mult > 1 ? 'up' : (mult < 1 ? 'down' : '');
    const active = fmtPeakActive(w);
    return '<tr>'
      + '<td><span class="peak-mult ' + cls + '">×' + mult + '</span></td>'
      + '<td>' + esc(fmtPeakWindow(w)) + '</td>'
      + '<td>' + esc(fmtPeakDays(w)) + '</td>'
      + '<td class="peak-scope">' + esc(fmtPeakScope(w)) + (active ? ' · <span class="peak-scope">' + esc(active) + '</span>' : '') + '</td>'
      + '</tr>';
  }).join('');
  return '<div class="subsec">'
    + '<div class="subsec-title peak-title">⚡ peak_hours（UTC+0，first-match wins）</div>'
    + '<table class="compact"><thead><tr>'
    + '<th class="num">倍率</th><th>时段</th><th>适用日</th><th>模型范围</th>'
    + '</tr></thead><tbody>' + rows + '</tbody></table></div>';
}

function slotsTableHtml(slots, title, titleClass) {
  const entries = Object.entries(slots || {});
  if (!entries.length) return '';
  const rows = entries.map(([slot, mid]) => {
    return '<tr>'
      + '<td><span class="slot-name">' + esc(slot) + '</span></td>'
      + '<td><a class="model-link" data-model="' + esc(mid) + '">' + esc(mid) + '</a></td>'
      + '</tr>';
  }).join('');
  return '<div class="subsec">'
    + '<div class="subsec-title ' + (titleClass||'') + '">' + esc(title) + '</div>'
    + '<table class="compact"><thead><tr><th>槽位</th><th>model_id</th></tr></thead><tbody>'
    + rows + '</tbody></table></div>';
}

function modelTableHtml(models, withBar) {
  if (!models || !models.length) return '';
  const rows = models.map(m => {
    const srcCls = m.source === 'override' ? 'src-override' : (m.source === 'top-level' ? 'src-toplevel' : 'src-noprice');
    const srcLabel = m.source === 'override' ? 'override' : (m.source === 'top-level' ? 'top-level' : 'no_price');
    const dpBadge = m.default_platform ? '<span class="badge">' + esc(m.default_platform) + '</span>' : '';
    return '<tr>'
      + '<td><a class="model-link" data-model="' + esc(m.id) + '">' + esc(m.id) + '</a> ' + dpBadge + '</td>'
      + '<td class="num">' + (withBar ? priceBarHtml(m.input_M) : fmtPrice(m.input_M)) + '</td>'
      + '<td class="num">' + fmtPrice(m.output_M) + '</td>'
      + '<td class="num">' + fmtPrice(m.cache_M) + '</td>'
      + '<td class="num">' + fmtNum(m.max_input) + '</td>'
      + '<td class="num">' + fmtNum(m.max_output) + '</td>'
      + '<td class="num">' + fmtNum(m.context_window) + '</td>'
      + '<td><span class="src-tag ' + srcCls + '">' + srcLabel + '</span></td>'
      + '</tr>';
  }).join('');
  return '<table>'
    + '<thead><tr>'
    + '<th>模型</th><th class="num">input $/M</th><th class="num">output $/M</th><th class="num">cache $/M</th>'
    + '<th class="num">max_in</th><th class="num">max_out</th><th class="num">ctx</th><th>来源</th>'
    + '</tr></thead><tbody>' + rows + '</tbody></table>';
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

  const hasPeak = p.peak_hours && p.peak_hours.length > 0;
  const hasCpBranch = (p.slots_coding_plan && Object.keys(p.slots_coding_plan).length)
    || (p.models_coding_plan && p.models_coding_plan.length);

  const bodyParts = [];
  bodyParts.push(p.models.length
    ? modelTableHtml(p.models, true)
    : '<div class="empty">无 model_list.default</div>');

  if (p.slots_default && Object.keys(p.slots_default).length) {
    bodyParts.push(slotsTableHtml(p.slots_default, '槽位映射（models.default）', ''));
  }
  if (hasPeak) bodyParts.push(peakTableHtml(p));

  if (hasCpBranch) {
    const cpParts = ['<div class="cp-section">'];
    cpParts.push('<div class="subsec-title cp-title">Coding Plan 分支</div>');
    if (p.slots_coding_plan && Object.keys(p.slots_coding_plan).length) {
      cpParts.push(slotsTableHtml(p.slots_coding_plan, '槽位映射（models.coding_plan）', 'cp-title'));
    }
    if (p.models_coding_plan && p.models_coding_plan.length) {
      cpParts.push('<div class="subsec"><div class="subsec-title cp-title">cp 分支模型</div>'
        + modelTableHtml(p.models_coding_plan, true) + '</div>');
    }
    cpParts.push('</div>');
    bodyParts.push(cpParts.join(''));
  }

  return '<div class="card" data-key="' + esc(p.key) + '">'
    + '<div class="card-head">'
      + '<div class="logo">' + logoHtml(p) + '</div>'
      + '<div class="head-main">'
        + '<div class="head-row">'
          + '<span class="name">' + esc(p.name) + '</span>'
          + '<span class="proto">' + esc(p.key) + '</span>'
          + (p.client_type ? '<span class="badge">' + esc(p.client_type) + '</span>' : '')
          + (cp ? '<span class="badge cp">coding_plan</span>' : '')
          + (p.is_coding_plan ? '<span class="badge iscp">is_coding_plan</span>' : '')
          + (p.has_override ? '<span class="badge override">override</span>' : '')
          + (hasPeak ? '<span class="badge peak">⚡ peak</span>' : '')
          + '<span class="badge">models: ' + p.model_count + '</span>'
        + '</div>'
        + (p.desc ? '<div class="desc">' + esc(p.desc) + '</div>' : '')
        + (links.length ? '<div class="links">' + links.join('') + '</div>' : '')
        + (eps ? '<div class="endpoints">' + eps + '</div>' : '')
      + '</div>'
      + '<span class="chev">▶</span>'
    + '</div>'
    + '<div class="card-body">' + bodyParts.join('') + '</div>'
    + '</div>';
}

// 顶部聚合统计条
function renderStatsBar() {
  const s = DATA.stats;
  const items = [
    {label: '协议', value: s.total_protocols, cls: 'accent'},
    {label: '去重模型', value: s.total_models, cls: ''},
    {label: '定价覆盖', value: s.pricing_coverage_pct + '%', cls: 'green'},
    {label: 'peak 协议', value: s.protocols_with_peak, cls: 'amber'},
    {label: 'is_coding_plan', value: s.protocols_with_cp, cls: 'purple'},
    {label: 'override 协议', value: s.protocols_with_override, cls: ''},
  ];
  statsBarEl.innerHTML = '<div class="stats-card">'
    + items.map(it => '<div class="stat-item ' + it.cls + '">'
      + '<span class="label">' + esc(it.label) + '</span>'
      + '<span class="value">' + esc(it.value) + '</span></div>').join('')
    + '</div>';
}

// 全局 input $/M 分布条
function renderDistBar() {
  const s = DATA.stats;
  if (s.overall_max_input_M == null) {
    distWrapEl.innerHTML = '';
    return;
  }
  const max = s.overall_max_input_M;
  const min = s.overall_min_input_M ?? 0;
  function pct(v) {
    if (v == null) return null;
    return Math.max(0, Math.min(100, ((v - min) / (max - min || 1)) * 100));
  }
  const ticks = [
    {label: 'min ' + fmtPrice(min), pct: 0, cls: ''},
    {label: 'p25 ' + fmtPrice(s.overall_p25_input_M), pct: pct(s.overall_p25_input_M), cls: 'pct25'},
    {label: 'median ' + fmtPrice(s.overall_median_input_M), pct: pct(s.overall_median_input_M), cls: 'pct50'},
    {label: 'p75 ' + fmtPrice(s.overall_p75_input_M), pct: pct(s.overall_p75_input_M), cls: 'pct75'},
    {label: 'max ' + fmtPrice(max), pct: 100, cls: ''},
  ].filter(t => t.pct != null);
  distWrapEl.innerHTML = '<div class="dist-card">'
    + '<div class="dist-title">全局 input $/M 分布（绿 < median · 黄 [median, p75) · 红 ≥ p75）</div>'
    + '<div class="dist-bar">'
      + ticks.map(t => '<span class="dist-tick ' + t.cls + '" style="left:' + t.pct.toFixed(1) + '%" data-label="' + esc(t.label) + '"></span>').join('')
    + '</div>'
    + '<div class="dist-legend"><span>' + fmtPrice(min) + '</span><span>' + fmtPrice(max) + '</span></div>'
    + '</div>';
}

// 模型详情弹窗（单例）
function findModelEverywhere(mid) {
  // 从 protocols 里找第一条出现的 row（含 cp 分支），其 context_tiers/pricing 来自 models.json
  for (const p of DATA.protocols) {
    for (const m of p.models) {
      if (m.id === mid) return m;
    }
    for (const m of (p.models_coding_plan || [])) {
      if (m.id === mid) return m;
    }
  }
  return null;
}
function showModelDetail(mid) {
  const m = findModelEverywhere(mid);
  if (!m) return;
  const usedBy = modelProtocols[mid] || [];
  const tiers = m.context_tiers || [];
  const pricing = m.pricing || {};

  // context_tiers 表
  let tiersHtml = '';
  if (tiers.length) {
    const rows = tiers.map(t => '<tr>'
      + '<td class="num">' + fmtNum(t.min_tokens) + '</td>'
      + '<td class="num">' + fmtPrice((t.input_cost_per_token ?? 0) * 1_000_000) + '</td>'
      + '<td class="num">' + fmtPrice((t.output_cost_per_token ?? 0) * 1_000_000) + '</td>'
      + '<td class="num">' + fmtPrice((t.cache_read_input_token_cost ?? 0) * 1_000_000) + '</td>'
      + '</tr>').join('');
    tiersHtml = '<div class="modal-section"><h3>分级 context 定价（context_tiers）</h3>'
      + '<table><thead><tr>'
      + '<th class="num">min_tokens</th><th class="num">input $/M</th><th class="num">output $/M</th><th class="num">cache $/M</th>'
      + '</tr></thead><tbody>' + rows + '</tbody></table></div>';
  }

  // pricing override 表（每协议 entry 的 $/M）
  let pricingHtml = '';
  const pricingEntries = Object.entries(pricing).filter(([k]) => pricing[k] && (
    pricing[k].input_cost_per_token != null || pricing[k].output_cost_per_token != null
  ));
  if (pricingEntries.length) {
    const rows = pricingEntries.map(([k, v]) => '<tr>'
      + '<td><code class="k">' + esc(k) + '</code></td>'
      + '<td class="num">' + fmtPrice((v.input_cost_per_token ?? null) === null ? null : v.input_cost_per_token * 1_000_000) + '</td>'
      + '<td class="num">' + fmtPrice((v.output_cost_per_token ?? null) === null ? null : v.output_cost_per_token * 1_000_000) + '</td>'
      + '<td class="num">' + fmtPrice((v.cache_read_input_token_cost ?? null) === null ? null : v.cache_read_input_token_cost * 1_000_000) + '</td>'
      + '</tr>').join('');
    pricingHtml = '<div class="modal-section"><h3>per-protocol 定价 override（pricing）</h3>'
      + '<table><thead><tr>'
      + '<th>protocol</th><th class="num">input $/M</th><th class="num">output $/M</th><th class="num">cache $/M</th>'
      + '</tr></thead><tbody>' + rows + '</tbody></table></div>';
  }

  // used-by badges
  const usedByHtml = usedBy.length
    ? '<div class="modal-section"><h3>出现于 ' + usedBy.length + ' 处协议</h3>'
      + '<div class="used-by">' + usedBy.map(u => '<span class="badge ' + (u.branch === 'coding_plan' ? 'cp' : '') + '">' + esc(u.key) + (u.branch === 'coding_plan' ? ' · cp' : '') + '</span>').join('') + '</div></div>'
    : '';

  modalEl.innerHTML = ''
    + '<div class="modal-head">'
      + '<h2>' + esc(m.id) + '</h2>'
      + (m.default_platform ? '<span class="badge">default: ' + esc(m.default_platform) + '</span>' : '')
      + '<button class="modal-close" aria-label="关闭">×</button>'
    + '</div>'
    + '<div class="kv">'
      + '<span class="k">当前解析 source</span><span class="v">' + esc(m.source) + '</span>'
      + '<span class="k">input $/M</span><span class="v">' + fmtPrice(m.input_M) + '</span>'
      + '<span class="k">output $/M</span><span class="v">' + fmtPrice(m.output_M) + '</span>'
      + '<span class="k">cache $/M</span><span class="v">' + fmtPrice(m.cache_M) + '</span>'
      + '<span class="k">max_input</span><span class="v">' + fmtTokens(m.max_input) + '</span>'
      + '<span class="k">max_output</span><span class="v">' + fmtTokens(m.max_output) + '</span>'
      + '<span class="k">context_window</span><span class="v">' + fmtTokens(m.context_window) + '</span>'
    + '</div>'
    + tiersHtml + pricingHtml + usedByHtml;
  modalBackdrop.classList.add('open');
}
function closeModal() { modalBackdrop.classList.remove('open'); }

// 事件：modal 关闭（Esc / backdrop / close 按钮）
modalBackdrop.addEventListener('click', (e) => {
  if (e.target === modalBackdrop) closeModal();
});
modalEl.addEventListener('click', (e) => {
  if (e.target.classList.contains('modal-close')) closeModal();
});
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && modalBackdrop.classList.contains('open')) closeModal();
});

function applyFilter() {
  const q = searchEl.value.trim().toLowerCase();
  const onlyOv = onlyOverrideEl.checked;
  const fPeak = hasPeakEl.checked;
  const fCp = hasCpEl.checked;
  const fIscp = isCpEl.checked;
  const ct = clientTypeEl.value;
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
    if (sortKey === 'output_M') {
      const amin = Math.min(...a.models.map(m => m.output_M == null ? Infinity : m.output_M), Infinity);
      const bmin = Math.min(...b.models.map(m => m.output_M == null ? Infinity : m.output_M), Infinity);
      return amin - bmin;
    }
    if (sortKey === 'ctx') {
      const amax = Math.max(...a.models.map(m => m.context_window == null ? 0 : m.context_window), 0);
      const bmax = Math.max(...b.models.map(m => m.context_window == null ? 0 : m.context_window), 0);
      return bmax - amax;
    }
    if (sortKey === 'has_peak') {
      const ap = (a.peak_hours && a.peak_hours.length) ? 1 : 0;
      const bp = (b.peak_hours && b.peak_hours.length) ? 1 : 0;
      if (ap !== bp) return bp - ap;
      return (a.name || '').localeCompare(b.name || '');
    }
    return 0;
  });

  let shown = 0;
  grid.innerHTML = '';
  for (const p of proto) {
    if (onlyOv && !p.has_override) continue;
    if (fPeak && !(p.peak_hours && p.peak_hours.length)) continue;
    if (fCp && p.endpoints_coding_plan.length === 0) continue;
    if (fIscp && !p.is_coding_plan) continue;
    if (ct && p.client_type !== ct) continue;
    // 搜索：name / key / 模型 id / 槽位 model_id / peak scope
    const slotIds = Object.values(p.slots_default || {}).concat(Object.values(p.slots_coding_plan || {}));
    const peakScope = (p.peak_hours || []).flatMap(w => w.models || []);
    const modelIds = p.models.map(m => m.id).concat((p.models_coding_plan || []).map(m => m.id));
    const haystack = [
      (p.name || '').toLowerCase(), p.key.toLowerCase(),
      ...modelIds, ...slotIds, ...peakScope,
    ].join(' ');
    const hit = !q || haystack.includes(q);
    if (!hit) continue;
    grid.insertAdjacentHTML('beforeend', cardHtml(p));
    shown++;
  }
  countEl.textContent = shown + ' / ' + DATA.protocols.length + ' 平台';
}

// 事件委托：卡片展开 / 模型详情点击
grid.addEventListener('click', (e) => {
  const link = e.target.closest('.model-link');
  if (link) {
    e.stopPropagation();
    showModelDetail(link.getAttribute('data-model'));
    return;
  }
  const head = e.target.closest('.card-head');
  if (head) head.parentElement.classList.toggle('expanded');
});

searchEl.addEventListener('input', applyFilter);
sortEl.addEventListener('change', applyFilter);
clientTypeEl.addEventListener('change', applyFilter);
onlyOverrideEl.addEventListener('change', applyFilter);
hasPeakEl.addEventListener('change', applyFilter);
hasCpEl.addEventListener('change', applyFilter);
isCpEl.addEventListener('change', applyFilter);

renderStatsBar();
renderDistBar();
applyFilter();
</script>
</body>
</html>
"""


def render(view_data):
    payload = json.dumps(view_data, ensure_ascii=False)
    # <script type="application/json"> 的 textContent 不反转 HTML 实体，
    # html.escape 会把 " 转 &quot; 致浏览器 JSON.parse 炸（sediment 契约）。
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
