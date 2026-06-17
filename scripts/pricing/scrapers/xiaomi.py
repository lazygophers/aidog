"""Xiaomi MiMo 官方一手价 (https://mimo.mi.com/docs/zh-CN/price/pay-as-you-go)。

CNY/M tokens (国内定价) → $/token (util.CNY_PER_USD)。2026-06-17 核实。
仅收录文本生成模型; ASR/TTS 非标准 token 计费 (按时长/限时免费), 不收录。
海外 USD 定价与国内 CNY÷7.2 基本一致, 统一取国内口径。
缓存写入限时免费 → cache_creation 留空。
mimo-v2 系列 (v2-pro/v2-omni/v2-flash) 2026.6.1–6.30 陆续转发至 v2.5 并下线, 保留作兼容别名。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import cny_per_m_to_usd_token

PRICING_URL = "https://mimo.mi.com/docs/zh-CN/price/pay-as-you-go"

CTX_1M = 1024 * 1024
CTX_256K = 256 * 1024
OUT_128K = 128 * 1024
OUT_64K = 64 * 1024


def _entry(
    inp: float, out: float, cache: float, *,
    ctx: int, max_out: int | None = None,
) -> ModelEntry:
    pp = PlatformPricing(
        input_cost_per_token=cny_per_m_to_usd_token(inp),
        output_cost_per_token=cny_per_m_to_usd_token(out),
        cache_read_input_token_cost=cny_per_m_to_usd_token(cache),
    )
    return ModelEntry(
        input_cost_per_token=pp.input_cost_per_token,
        output_cost_per_token=pp.output_cost_per_token,
        cache_read_input_token_cost=pp.cache_read_input_token_cost,
        max_input_tokens=ctx,
        max_output_tokens=max_out,
        context_window=ctx,
        pricing={"xiaomi_mimo": pp},
    )


async def fetch() -> dict[str, ModelEntry]:
    return {
        # MiMo-V2.5 旗舰 (2026.05.27 降价生效)
        "mimo-v2.5-pro": _entry(3.00, 6.00, 0.025, ctx=CTX_1M, max_out=OUT_128K),
        "mimo-v2.5": _entry(1.00, 2.00, 0.02, ctx=CTX_1M, max_out=OUT_128K),
        # MiMo-V2 (转发至 v2.5 中, 兼容别名)
        "mimo-v2-pro": _entry(3.00, 6.00, 0.025, ctx=CTX_1M),
        "mimo-v2-omni": _entry(1.00, 2.00, 0.02, ctx=CTX_256K),
        "mimo-v2-flash": _entry(0.70, 2.10, 0.07, ctx=CTX_256K, max_out=OUT_64K),
    }
