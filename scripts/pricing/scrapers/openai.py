"""OpenAI 官方一手价 (https://developers.openai.com/api/docs/pricing)。

Astro v6 SSR, HTML 静态可抓: 页内嵌多个 `"rows":[...]` JSON island, 每个 = 一个定价表。
每行 `[model名, input, cached_input, output]` (单位 $/1M tokens)。

解析范围 (Standard tier, per-text-token 计费模型):
- flagship 全表 (island 首现, 42 模型): gpt-5.x + o-series + legacy
- ChatGPT specialized (chat-latest)
- Codex (gpt-5.3-codex 等)
跳 Batch/Flex/Priority (SegmentedControl 同表多 tab, 取 Standard 首个)、跳
image/video/realtime/transcription (单位非 per-text-token, schema 不适用)。

上下文阶梯: OpenAI 旗舰模型分 short (<272K) / long (≥272K) 两档。island JSON 只存
short 档; long 档是 OpenAI 固定倍率 (input×2 / output×1.5 / cache_read×2, 2026-06
跨 gpt-5.5/5.5-pro/5.4/5.4-pro 四模型核实一致), 客户端按倍率渲染。仅 island 模型名含
"(<NNNK context length)" 标记者有 long 档, 据此建 context_tiers。

top-level 价 = short 档 (基线, 向后兼容); pricing["openai"] = short per-platform 价;
context_tiers = [{min_tokens, long 档价}]。失败抛异常由 aggregate 跳过, 禁静默返空。
max_tokens 留 None (模型固有, openrouter 骨干合并补; 官方 models 页 div 结构 parse 不可靠, 不硬编)。
"""

from __future__ import annotations

import html as _html
import json
import re
from typing import Any

from fetchutil import get_text
from schema import ContextTier, ModelEntry, PlatformPricing

PRICING_URL = "https://developers.openai.com/api/docs/pricing"

# OpenAI 长上下文 (>272K) 定价倍率 (相对 short 档, 2026-06 核实)
_LONG_INPUT_MULT = 2.0
_LONG_OUTPUT_MULT = 1.5
_LONG_CACHE_MULT = 2.0
# "<272K context length" 标记里提取的阈值 (272K = 272000 tokens)
_CTX_LABEL_RE = re.compile(r"\s*\(<\s*(\d+(?:\.\d+)?)\s*K?\s*context length\)\s*$")


def _extract_rows_islands(h: str) -> list[list[list[Any]]]:
    """提取所有 `"rows":[...]` island 并解码为 cell 列表。

    返回 [[ [model名, input, cached, output], ... ], ...] 每个 island 一组。
    """
    h = _html.unescape(h)
    out: list[list[list[Any]]] = []
    for m in re.finditer(r'"rows":\[', h):
        i = m.start() + len('"rows":')
        start = h.index("[", i)
        j = start
        depth = 0
        instr = False
        esc = False
        while j < len(h):
            c = h[j]
            if instr:
                if esc:
                    esc = False
                elif c == "\\":
                    esc = True
                elif c == '"':
                    instr = False
            else:
                if c == '"':
                    instr = True
                elif c == "[":
                    depth += 1
                elif c == "]":
                    depth -= 1
                    if depth == 0:
                        break
            j += 1
        try:
            data = json.loads(h[start : j + 1])
        except json.JSONDecodeError:
            continue
        rb = data[1] if isinstance(data, list) and len(data) >= 2 and isinstance(data[1], list) else None
        if rb is None:
            continue
        cells: list[list[Any]] = []
        for row in rb:
            inner = row[1] if isinstance(row, list) and len(row) >= 2 and isinstance(row[1], list) else row
            decoded = [(c[1] if isinstance(c, list) and len(c) == 2 else c) for c in inner]
            cells.append(decoded)
        out.append(cells)
    return out


def _num(v: Any) -> float | None:
    """island 单元格 → $/1M float; '' / '-' / None → None (无该档价)。"""
    if v is None or v == "" or v == "-":
        return None
    try:
        f = float(v)
        return f if f > 0 else None
    except (TypeError, ValueError):
        return None


def _per_m_to_token(per_m: float | None) -> float:
    return (per_m or 0.0) / 1_000_000.0


def _build_entry(name: str, in_m: float | None, cache_m: float | None, out_m: float | None) -> ModelEntry:
    """short 档价 → ModelEntry (top-level + pricing['openai']); long 档由调用方按倍率附 tier。"""
    in_t = _per_m_to_token(in_m)
    out_t = _per_m_to_token(out_m)
    cache_t = _per_m_to_token(cache_m)
    return ModelEntry(
        default_platform="openai",
        input_cost_per_token=in_t,
        output_cost_per_token=out_t,
        cache_read_input_token_cost=cache_t,
        pricing={"openai": PlatformPricing(
            input_cost_per_token=in_t or None,
            output_cost_per_token=out_t or None,
            cache_read_input_token_cost=cache_t or None,
        )},
    )


def _parse_threshold(name: str) -> tuple[str, int | None]:
    """剥模型名尾部 '(<NNNK context length)' 标记 → (干净名, 阈值 tokens)。

    有标记 = 该模型存在 long 档; None = 单一价。
    """
    mm = _CTX_LABEL_RE.search(name)
    if not mm:
        return name.strip(), None
    val = float(mm.group(1))
    return name[: mm.start()].strip(), int(val * 1000)


async def fetch() -> dict[str, ModelEntry]:
    html_text = await get_text(PRICING_URL)
    islands = _extract_rows_islands(html_text)
    if not islands:
        raise RuntimeError("OpenAI pricing 页无 rows island (结构改版?)")

    # island#0 = Standard flagship 全表 (42 模型, 页内首渲染)。
    # 另需 ChatGPT specialized (chat-latest) 与 Codex 两 island。
    flagship = islands[0]
    chat = _find_island(islands, "chat-latest")
    codex = _find_island(islands, "gpt-5.3-codex")

    out: dict[str, ModelEntry] = {}
    for block in (flagship, chat, codex):
        if not block:
            continue
        for cells in block:
            if not cells or not isinstance(cells[0], str):
                continue
            raw_name = cells[0]
            if len(cells) < 4:
                continue
            name, threshold = _parse_threshold(raw_name)
            if not name:
                continue
            in_m = _num(cells[1])
            cache_m = _num(cells[2])
            out_m = _num(cells[3])
            if in_m is None and out_m is None:
                continue  # 无价 (如 gpt-5.4-cyber 占位) 跳过
            entry = _build_entry(name, in_m, cache_m, out_m)
            # 有 "(<NNNK context length)" 标记 → 该模型有 long 档, 按倍率建 tier
            if threshold is not None:
                entry.context_tiers.append(ContextTier(
                    min_tokens=threshold,
                    input_cost_per_token=_per_m_to_token(in_m * _LONG_INPUT_MULT if in_m else None) or None,
                    output_cost_per_token=_per_m_to_token(out_m * _LONG_OUTPUT_MULT if out_m else None) or None,
                    cache_read_input_token_cost=_per_m_to_token(cache_m * _LONG_CACHE_MULT if cache_m else None) or None,
                ))
            # 同名 (多 island / 多 variant) 取首个有效
            if name not in out:
                out[name] = entry
    if not out:
        raise RuntimeError("OpenAI pricing 解析得 0 模型 (结构改版?)")
    return out


def _find_island(islands: list[list[list[Any]]], needle: str) -> list[list[Any]] | None:
    """在 islands 中找含指定模型名的 island (取首个命中, 即 Standard tier)。"""
    for block in islands[1:]:  # 跳过 flagship #0
        for cells in block:
            if cells and isinstance(cells[0], str) and cells[0] == needle:
                return block
    return None
