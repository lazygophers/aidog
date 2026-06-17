"""DeepSeek 官方一手价 (https://api-docs.deepseek.com/zh-cn/quick_start/pricing)。

CNY/M tokens → $/token (util.CNY_PER_USD)。2026-06-16 核实。
v4-flash/v4-pro; deepseek-chat/deepseek-reasoner 2026/07/24 弃用, 映射到 v4-flash。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import cny_per_m_to_usd_token

# 1M 上下文, 最大输出 384K
MAX_OUT = 384 * 1024
CTX = 1024 * 1024


async def fetch() -> dict[str, ModelEntry]:
    def _entry(inp_miss: float, inp_hit: float, out: float) -> ModelEntry:
        return ModelEntry(
            input_cost_per_token=cny_per_m_to_usd_token(inp_miss),
            output_cost_per_token=cny_per_m_to_usd_token(out),
            cache_read_input_token_cost=cny_per_m_to_usd_token(inp_hit),
            max_input_tokens=CTX,
            max_output_tokens=MAX_OUT,
            context_window=CTX,
            pricing={"deepseek": PlatformPricing(
                input_cost_per_token=cny_per_m_to_usd_token(inp_miss),
                output_cost_per_token=cny_per_m_to_usd_token(out),
                cache_read_input_token_cost=cny_per_m_to_usd_token(inp_hit),
            )},
        )

    return {
        "deepseek-v4-flash": _entry(1, 0.02, 2),
        "deepseek-v4-pro": _entry(3, 0.025, 6),
        # 兼容别名 (弃用前仍可用)
        "deepseek-chat": _entry(1, 0.02, 2),
        "deepseek-reasoner": _entry(3, 0.025, 6),
    }
