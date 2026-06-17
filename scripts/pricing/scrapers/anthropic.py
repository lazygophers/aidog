"""Anthropic 官方一手价 (https://platform.claude.com/docs/en/about-claude/pricing)。

官方定价页 JS 渲染, 表格拆碎不可程序化解析; 低价变动低频, 硬编码表 (跟 deepseek.py 同策略)。
价格来源: 上述文档页 2026-06-17 核实, $/MTok → $/token (除 1_000_000)。

cache_creation 用 5m cache write 价 (1.25x base input; prompt caching 默认 write 操作);
cache_read = cache hit (0.1x base input)。
context_window: 4.6+ 代 1M tokens; pre-4.6 代 200K tokens。
max_output_tokens: 1M 代 128K; 200K 代 64K (Opus 4.1/4 为 32K, 早期遗留值)。

模型名用 dateless 主名 (如 claude-opus-4-8), 跟 litellm 别名表对齐确保合并覆盖;
带日期/别名后缀的变体由 litellm 兜底 (47 个 claude-* 别名)。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import per_m

# (model_id, input $/MTok, cache_write_5m $/MTok, cache_read $/MTok, output $/MTok,
#  context_window, max_output_tokens)
_PRICES: list[tuple[str, float, float, float, float, int, int]] = [
    # --- Frontier (1M context, 128K max output) ---
    ("claude-fable-5",       10, 12.50, 1,    50, 1_000_000, 128 * 1024),
    ("claude-opus-4-8",       5,  6.25, 0.50, 25, 1_000_000, 128 * 1024),
    ("claude-opus-4-7",       5,  6.25, 0.50, 25, 1_000_000, 128 * 1024),
    ("claude-opus-4-6",       5,  6.25, 0.50, 25, 1_000_000, 128 * 1024),
    ("claude-sonnet-4-6",     3,  3.75, 0.30, 15, 1_000_000, 128 * 1024),
    # --- 200K context (64K max output, except Opus 4.1/4 = 32K) ---
    ("claude-opus-4-5",       5,  6.25, 0.50, 25, 200_000,  64 * 1024),
    ("claude-opus-4-1",      15, 18.75, 1.50, 75, 200_000,  32 * 1024),
    ("claude-opus-4",        15, 18.75, 1.50, 75, 200_000,  32 * 1024),
    ("claude-sonnet-4-5",     3,  3.75, 0.30, 15, 200_000,  64 * 1024),
    ("claude-sonnet-4",       3,  3.75, 0.30, 15, 200_000,  64 * 1024),
    ("claude-haiku-4-5",      1,  1.25, 0.10,  5, 200_000,  64 * 1024),
    ("claude-haiku-3-5",   0.80,  1.00, 0.08,  4, 200_000,  64 * 1024),
]


async def fetch() -> dict[str, ModelEntry]:
    out: dict[str, ModelEntry] = {}
    for mid, inp_m, cw_m, cr_m, out_m, ctx, max_out in _PRICES:
        in_t = per_m(inp_m)
        cw_t = per_m(cw_m)
        cr_t = per_m(cr_m)
        out_t = per_m(out_m)
        pp = PlatformPricing(
            input_cost_per_token=in_t,
            output_cost_per_token=out_t,
            cache_read_input_token_cost=cr_t,
            cache_creation_input_token_cost=cw_t,
        )
        out[mid] = ModelEntry(
            default_platform="anthropic",
            input_cost_per_token=in_t,
            output_cost_per_token=out_t,
            cache_read_input_token_cost=cr_t,
            cache_creation_input_token_cost=cw_t,
            max_input_tokens=ctx,
            max_output_tokens=max_out,
            context_window=ctx,
            pricing={"anthropic": pp},
        )
    return out

