"""GLM (智谱 z-ai) 官方一手价 (https://open.bigmodel.cn/pricing)。

CNY/M tokens → $/token (util.CNY_PER_USD)。2026-06-17 核实。
阶梯定价 (input 长度 [0,32K) 短档 / [32K+) 长档) 用 context_tiers 表达;
GLM-4.7 另有 output 长度阶梯 ([0,0.2M)/[0.2M+)), 三档组合过于复杂, 取首档代表价。
GLM-4.7-Flash / GLM-4.6V-Flash 免费 → 不收录 (0 价无估算意义)。
GLM-4.6 文本已从旗舰表移除 (2026-07 下架), 不臆测。
缓存存储官方"限时免费", cache_creation 留空。
"""

from __future__ import annotations

from schema import ContextTier, ModelEntry, PlatformPricing
from util import cny_per_m_to_usd_token

PRICING_URL = "https://open.bigmodel.cn/pricing"

# 上下文 (tokens)
CTX_1M = 1024 * 1024
CTX_200K = 200 * 1024
CTX_128K = 128 * 1024
CTX_64K = 64 * 1024
# input 长度阶梯分界 (官方 [0,32) / [32+) 千 tokens)
TIER_32K = 32 * 1024


def _pp(inp: float, out: float, cache: float) -> PlatformPricing:
    return PlatformPricing(
        input_cost_per_token=cny_per_m_to_usd_token(inp),
        output_cost_per_token=cny_per_m_to_usd_token(out),
        cache_read_input_token_cost=cny_per_m_to_usd_token(cache),
    )


def _tier(min_t: int, inp: float, out: float, cache: float) -> ContextTier:
    p = _pp(inp, out, cache)
    return ContextTier(
        min_tokens=min_t,
        input_cost_per_token=p.input_cost_per_token,
        output_cost_per_token=p.output_cost_per_token,
        cache_read_input_token_cost=p.cache_read_input_token_cost,
    )


def _entry(
    inp: float, out: float, cache: float, *, ctx: int | None = None,
    max_out: int | None = None, tiers: list[ContextTier] | None = None,
) -> ModelEntry:
    base = _pp(inp, out, cache)
    return ModelEntry(
        input_cost_per_token=base.input_cost_per_token,
        output_cost_per_token=base.output_cost_per_token,
        cache_read_input_token_cost=base.cache_read_input_token_cost,
        max_input_tokens=ctx,
        max_output_tokens=max_out,
        context_window=ctx,
        context_tiers=tiers or [],
        pricing={"glm": base},
    )


def _tiered(
    base: tuple[float, float, float], long: tuple[float, float, float], *, ctx: int | None = None,
) -> ModelEntry:
    """短档 base + 长档 (≥32K input) context_tier。"""
    bi, bo, bc = base
    li, lo, lc = long
    return _entry(bi, bo, bc, ctx=ctx, tiers=[_tier(TIER_32K, li, lo, lc)])


async def fetch() -> dict[str, ModelEntry]:
    out: dict[str, ModelEntry] = {
        # 单档模型
        # 旗舰 1M 上下文
        "glm-5.2": _entry(8, 28, 2, ctx=CTX_1M),
        # 200K 上下文; GLM-4.7 另有 output[0.2M+) 阶档 (3/14/0.6), 取首档代表
        "glm-4.7": _entry(2, 8, 0.4, ctx=CTX_200K),
        "glm-4.7-flashx": _entry(0.5, 3, 0.1, ctx=CTX_200K),
        # 128K, max_out 96K (官方文档 docs.bigmodel.cn glm-4.5)
        "glm-4.5": _entry(0.8, 2, 0.16, ctx=CTX_128K, max_out=96 * 1024),
    }

    # input 长度阶梯模型 ([0,32K) 短档 base / [32K+) 长档)
    out.update({
        "glm-5.1": _tiered((6, 24, 1.3), (8, 28, 2), ctx=CTX_1M),
        "glm-5-turbo": _tiered((5, 22, 1.2), (7, 26, 1.8)),
        "glm-5": _tiered((4, 18, 1), (6, 22, 1.5)),
        "glm-4.5-air": _tiered((0.8, 2, 0.16), (1.2, 8, 0.24), ctx=CTX_128K),
        # 视觉模型
        "glm-5v-turbo": _tiered((5, 22, 1.2), (7, 26, 1.8)),
        "glm-4.6v": _tiered((1, 3, 0.2), (2, 6, 0.4), ctx=CTX_128K),
        "glm-4.6v-flashx": _tiered((0.15, 1.5, 0.03), (0.3, 3, 0.03), ctx=CTX_128K),
        "glm-4.5v": _tiered((2, 6, 0.4), (4, 12, 0.8), ctx=CTX_64K),
    })
    return out
