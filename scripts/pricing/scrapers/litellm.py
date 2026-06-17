"""LiteLLM 骨干兜底源: model_prices_and_context_window.json (BerriAI/litellm main)。

100+ provider 聚合表, 作为 first-party scraper 未覆盖模型的兜底价来源。
fill:
- top-level 价 (近似; 各平台官方一手价由对应 first-party scraper 覆盖)
- pricing[<platform_type>] (按 prefix 映射) + pricing["litellm"]
- max_input/output_tokens + context_window (模型固有, 平台无关)

LiteLLM key 格式 `<provider>/<model>` (如 `zai/glm-4.6`); 模型名取 `/` 后半。
仅处理 PREFIX_MAP 命中的前缀; 其余 (litellm 自有 / 未知 org) 跳过避免污染。
例外: anthropic 模型在 LiteLLM 中以裸名 `claude-*` 列出 (无 `anthropic/` 前缀),
单独按 `claude-` 前缀识别归 anthropic。
"""

from __future__ import annotations

import asyncio

import fetchutil
from schema import ModelEntry, PlatformPricing

URL = (
    "https://raw.githubusercontent.com/BerriAI/litellm/main/"
    "model_prices_and_context_window.json"
)

# LiteLLM key 前缀 → aidog platform_type (Rust Protocol serde 裸名)
# 与 openrouter.py PREFIX_MAP 语义对齐 (openrouter 用 openrouter 内部 org slug,
# litellm 用上游 provider slug, 两者覆盖不同但 platform_type 落点一致)
PREFIX_MAP: dict[str, str] = {
    "openai": "openai",
    "anthropic": "anthropic",
    "gemini": "gemini",
    "vertex_ai": "gemini",
    "deepseek": "deepseek",
    "zai": "glm",
    "moonshot": "kimi",
    "minimax": "minimax",
}

# 裸名 (无 `/` 前缀) 模型名前缀 → platform_type。
# LiteLLM 对部分上游 provider 不加 org 前缀 (anthropic 即如此), 单独识别。
BARE_PREFIX_MAP: dict[str, str] = {
    "claude-": "anthropic",
}

RETRIES = 1  # 首次失败后重试次数


def _f(v) -> float:
    """LiteLLM 价格字段 → $/token float; 缺失/<=0 → 0。"""
    try:
        f = float(v)
        return f if f > 0 else 0.0
    except (TypeError, ValueError):
        return 0.0


async def _get() -> dict:
    """带一次重试的 JSON 抓取。"""
    last: Exception | None = None
    for _ in range(RETRIES + 1):
        try:
            return await fetchutil.get_json(URL)  # type: ignore[arg-type]
        except Exception as e:  # noqa: BLE001
            last = e
    raise last  # type: ignore[misc]


def _resolve_platform(key: str) -> tuple[str, str] | None:
    """LiteLLM key → (platform_type, model_name); 非目标返回 None。

    优先匹配 `<prefix>/<model>`; 否则按裸名前缀 (claude-*) 识别。
    """
    if "/" in key:
        prefix, _, name = key.partition("/")
        ptype = PREFIX_MAP.get(prefix)
        if ptype and name:
            return ptype, name
        return None
    for bp, ptype in BARE_PREFIX_MAP.items():
        if key.startswith(bp):
            return ptype, key
    return None


async def fetch() -> dict[str, ModelEntry]:
    data = await _get()
    out: dict[str, ModelEntry] = {}
    # LiteLLM JSON 顶层是 { "<model_key>": {fields}, ... }; 部分顶层 meta key
    # (如 "sample_spec") 无价格字段, 天然被 price==0 过滤
    for key, m in data.items():
        if not isinstance(m, dict):
            continue
        resolved = _resolve_platform(key)
        if resolved is None:
            continue
        ptype, name = resolved

        prompt = _f(m.get("input_cost_per_token"))
        completion = _f(m.get("output_cost_per_token"))
        if prompt == 0 and completion == 0:
            continue  # 无价 (LiteLLM 把无价模型也列进来, 此处过滤)

        cache_read = _f(m.get("cache_read_input_token_cost"))
        cache_write = _f(m.get("cache_creation_input_token_cost"))

        max_in = m.get("max_input_tokens")
        max_out = m.get("max_output_tokens")
        # 老字段 max_tokens ≈ context_window (LiteLLM 部分模型仅填此项)
        ctx = m.get("max_tokens") or max_in

        pp = PlatformPricing(
            input_cost_per_token=prompt or None,
            output_cost_per_token=completion or None,
            cache_read_input_token_cost=cache_read or None,
            cache_creation_input_token_cost=cache_write or None,
        )
        entry = ModelEntry(
            input_cost_per_token=prompt,
            output_cost_per_token=completion,
            cache_read_input_token_cost=cache_read,
            cache_creation_input_token_cost=cache_write or None,
            max_input_tokens=int(max_in) if max_in else None,
            max_output_tokens=int(max_out) if max_out else None,
            context_window=int(ctx) if ctx else None,
            pricing={ptype: pp, "litellm": pp.model_copy()},
            default_platform=ptype,
        )
        # 合并同名: 首次命中保留 (litellm 为兜底, first-party 已覆盖的模型
        # 由 REGISTRY 顺序在 aggregate 端择优, 此处不去重跨源)
        if name not in out:
            out[name] = entry
    return out


if __name__ == "__main__":  # pragma: no cover
    r = asyncio.run(fetch())
    print(f"models: {len(r)}")
    for k, v in list(r.items())[:5]:
        print(k, list(v.pricing.keys()))
