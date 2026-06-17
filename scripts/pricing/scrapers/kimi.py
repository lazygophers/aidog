"""Kimi (月之暗面 moonshot) 官方一手价 (https://platform.kimi.com/docs/pricing)。

CNY/M tokens → $/token (util.CNY_PER_USD)。2026-06-17 核实。
各模型独立定价页 (chat-k25/k26/k27-code/chat-v1), 价格表 SSR 可渲染 → 此处固化一手价。
K2.x 系列支持自动上下文缓存 (缓存命中价); Moonshot V1 无 cache 列 → cache_read 留空。
1M = 1,000,000。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import cny_per_m_to_usd_token

PRICING_URL = "https://platform.kimi.com/docs/pricing/chat"

CTX_256K = 262_144
CTX_128K = 131_072
CTX_32K = 32_768
CTX_8K = 8_192


def _entry(
    inp: float, out: float, *, ctx: int, cache: float | None = None,
) -> ModelEntry:
    pp = PlatformPricing(
        input_cost_per_token=cny_per_m_to_usd_token(inp),
        output_cost_per_token=cny_per_m_to_usd_token(out),
        cache_read_input_token_cost=(
            cny_per_m_to_usd_token(cache) if cache is not None else None
        ),
    )
    return ModelEntry(
        input_cost_per_token=pp.input_cost_per_token,
        output_cost_per_token=pp.output_cost_per_token,
        cache_read_input_token_cost=pp.cache_read_input_token_cost or 0.0,
        max_input_tokens=ctx,
        context_window=ctx,
        pricing={"kimi": pp},
    )


async def fetch() -> dict[str, ModelEntry]:
    return {
        # K2.x 多模态旗舰 (缓存命中 / 缓存未命中 / 输出; 256K 上下文)
        "kimi-k2.5": _entry(4.00, 21.00, ctx=CTX_256K, cache=0.70),
        "kimi-k2.6": _entry(6.50, 27.00, ctx=CTX_256K, cache=1.10),
        "kimi-k2.7-code": _entry(6.50, 27.00, ctx=CTX_256K, cache=1.30),
        # Moonshot V1 (无 prompt cache)
        "moonshot-v1-8k": _entry(2.00, 10.00, ctx=CTX_8K),
        "moonshot-v1-32k": _entry(5.00, 20.00, ctx=CTX_32K),
        "moonshot-v1-128k": _entry(10.00, 30.00, ctx=CTX_128K),
        # Moonshot V1 视觉预览 (同价)
        "moonshot-v1-8k-vision-preview": _entry(2.00, 10.00, ctx=CTX_8K),
        "moonshot-v1-32k-vision-preview": _entry(5.00, 20.00, ctx=CTX_32K),
        "moonshot-v1-128k-vision-preview": _entry(10.00, 30.00, ctx=CTX_128K),
    }
