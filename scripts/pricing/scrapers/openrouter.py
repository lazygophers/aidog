"""OpenRouter 骨干源: GET /api/v1/models (JSON, 337+ 模型)。

单次抓取即覆盖 openai/google/anthropic/deepseek/z-ai(glm)/minimax/moonshotai(kimi)/
stepfun 等多平台模型。fill:
- top-level 价 (近似, 标 openrouter 价; 各平台官方一手价由对应 first-party scraper 覆盖)
- pricing["openrouter"]
- max_input/output_tokens + context_window (模型固有, 平台无关)
"""

from __future__ import annotations

import httpx
from schema import ModelEntry, PlatformPricing

# OpenRouter model id 前缀 → aidog platform_type (Rust Protocol serde 裸名)
# 未列出的前缀仍按 openrouter 平台收录 (top-level 价 + max_tokens)
PREFIX_MAP: dict[str, str] = {
    "openai": "openai",
    "google": "gemini",
    "anthropic": "anthropic",
    "~anthropic": "anthropic",
    "deepseek": "deepseek",
    "z-ai": "glm",
    "minimax": "minimax",
    "moonshotai": "kimi",
    "stepfun": "stepfun",
    "qwen": "qwen",
    "mistralai": "mistral",
    "meta-llama": "meta",
}

API = "https://openrouter.ai/api/v1/models"


def _f(v) -> float:
    """OpenRouter 价格字符串 → $/token float; <=0 或 '-' → 0。"""
    try:
        f = float(v)
        return f if f > 0 else 0.0
    except (TypeError, ValueError):
        return 0.0


async def fetch() -> dict[str, ModelEntry]:
    async with httpx.AsyncClient(timeout=httpx.Timeout(30.0), follow_redirects=True) as c:
        r = await c.get(API, headers={"User-Agent": "aidog-pricing-sync/0.1"})
        r.raise_for_status()
        data = r.json()
    out: dict[str, ModelEntry] = {}
    for m in data.get("data", []):
        mid = m.get("id") or ""
        if "/" not in mid:
            continue
        pricing = m.get("pricing") or {}
        prompt = _f(pricing.get("prompt"))
        completion = _f(pricing.get("completion"))
        if prompt == 0 and completion == 0:
            continue  # 无价 (如 fusion 路由模型) 跳过
        cache_read = _f(pricing.get("input_cache_read"))
        cache_write = _f(pricing.get("input_cache_write"))

        top = m.get("top_provider") or {}
        context = m.get("context_length") or top.get("context_length")
        max_completion = top.get("max_completion_tokens")

        # 模型名: 去掉 org 前缀, 取后半 (openai/gpt-4o → gpt-4o); 全小写归一在 app 端做
        name = mid.split("/", 1)[1] if "/" in mid else mid
        # OpenRouter 名常含 org 重复 (deepseek/deepseek-chat → deepseek-chat)
        org = mid.split("/", 1)[0].lower()
        if name.lower().startswith(org + "-") or name.lower().startswith(org + "/"):
            pass  # 保留原名

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
            max_input_tokens=int(context) if context else None,
            max_output_tokens=int(max_completion) if max_completion else None,
            context_window=int(context) if context else None,
            pricing={"openrouter": pp},
        )
        # 标注 default_platform 为该模型的来源 platform_type (若映射命中)
        prefix = org
        entry.default_platform = PREFIX_MAP.get(prefix, "openrouter")
        # 合并同名 (OpenRouter 常有同模型多 variant, 取首个有效)
        if name not in out:
            out[name] = entry
    return out
