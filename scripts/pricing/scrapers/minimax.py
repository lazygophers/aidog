"""MiniMax 官方一手价 (https://platform.minimaxi.com/docs/guides/pricing-paygo)。

CNY/M tokens → $/token (util.CNY_PER_USD)。2026-06-17 核实。
仅收录语言模型 (chat completion); 语音/视频/音乐/图像非 token 计费, 不收录。
MiniMax-M3 有 input 长度阶梯 (≤512K 标准 / >512K 高档, 限时限供应), 用 context_tier 表达;
M3 价格为永久五折后实价。M2.x 含缓存写入价 (cache_creation)。
"""

from __future__ import annotations

from schema import ContextTier, ModelEntry, PlatformPricing
from util import cny_per_m_to_usd_token

PRICING_URL = "https://platform.minimaxi.com/docs/guides/pricing-paygo"

# M3 input 长度阶梯分界 (>512K 触发高档)
TIER_512K = 524_289  # 512*1024 + 1, 严格 ">"


def _pp(inp: float, out: float, cache_r: float, *, cache_w: float | None = None) -> PlatformPricing:
    return PlatformPricing(
        input_cost_per_token=cny_per_m_to_usd_token(inp),
        output_cost_per_token=cny_per_m_to_usd_token(out),
        cache_read_input_token_cost=cny_per_m_to_usd_token(cache_r),
        cache_creation_input_token_cost=(
            cny_per_m_to_usd_token(cache_w) if cache_w is not None else None
        ),
    )


def _entry(
    inp: float, out: float, cache_r: float, *, cache_w: float | None = None,
    tiers: list[ContextTier] | None = None,
) -> ModelEntry:
    pp = _pp(inp, out, cache_r, cache_w=cache_w)
    return ModelEntry(
        input_cost_per_token=pp.input_cost_per_token,
        output_cost_per_token=pp.output_cost_per_token,
        cache_read_input_token_cost=pp.cache_read_input_token_cost,
        cache_creation_input_token_cost=pp.cache_creation_input_token_cost,
        context_tiers=tiers or [],
        pricing={"minimax": pp},
    )


async def fetch() -> dict[str, ModelEntry]:
    out: dict[str, ModelEntry] = {
        # M3 标准档 (永久五折实价) + >512K 高档 context_tier
        "MiniMax-M3": _entry(
            2.10, 8.40, 0.42,
            tiers=[ContextTier(
                min_tokens=TIER_512K,
                input_cost_per_token=cny_per_m_to_usd_token(4.20),
                output_cost_per_token=cny_per_m_to_usd_token(16.80),
                cache_read_input_token_cost=cny_per_m_to_usd_token(0.84),
            )],
        ),
        # M2.7 系列 (含缓存写入 2.625)
        "MiniMax-M2.7": _entry(2.1, 8.4, 0.42, cache_w=2.625),
        "MiniMax-M2.7-highspeed": _entry(4.2, 16.8, 0.42, cache_w=2.625),
    }
    # 历史模型 (M2.5 / M2.1 / M2; cache_read=0.21, cache_write=2.625; highspeed input/output 翻倍)
    for name, inp, o in (
        ("MiniMax-M2.5", 2.1, 8.4), ("MiniMax-M2.5-highspeed", 4.2, 16.8),
        ("MiniMax-M2.1", 2.1, 8.4), ("MiniMax-M2.1-highspeed", 4.2, 16.8),
        ("MiniMax-M2", 2.1, 8.4),
    ):
        out[name] = _entry(inp, o, 0.21, cache_w=2.625)
    return out
