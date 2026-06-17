"""阿里云百炼 (DashScope/通义千问) 官方一手价。

价格源 (公开, 无需登录): https://help.aliyun.com/zh/model-studio/getting-started/models
计费口径 = ¥/千 Token (与 glm/kimi 的 ¥/M 不同) → util.cny_per_1k_to_usd_token。
2026-06-17 核实。取中国大陆(北京)地域价; 国际(新加坡)价高约 2.75×, 不取。

收录范围 (aidog 代理用文本/代码/视觉模型):
- 旗舰商业版 (qwen3-max / qwen-plus / qwen-flash / qwen-turbo / qwen-long / qwq-plus)
- Coder (qwen3-coder-plus / flash / qwen-coder-plus / turbo / 开源 480b / 30b)
- 开源主力 (qwen3-235b / 32b / 30b-a3b / 14b / 8b; qwen2.5-72b/32b/14b/7b)
- 视觉 (qwen-vl-max / vl-plus / qwen3-vl-plus / flash)
跳过: omni/asr/tts/文生图/视频 (非 per-text-token 计费)、快照/历史版 (-2024-xx, 由 litellm 兜底)。

阶梯: 商业版旗舰按输入 Token 数分档 (如 qwen3-max ≤32K/≤128K/≤252K), 用 context_tiers 表达;
base = 最低档。开源版多为单档。
cache_read = 显式缓存命中价 = base 档输入 × 10% (百炼固定比例, 仅支持缓存的旗舰+coder+vl);
cache_creation 按相对比例随档变, 留空避免与 tier 冲突。
思考模式 vs 非思考: 取非思考模式价 (思考仅 output 更贵; aidog 代理默认非思考)。
模型名用官方 id (如 qwen-plus), 与 litellm/openrouter 同名合并覆盖。
"""

from __future__ import annotations

from schema import ContextTier, ModelEntry, PlatformPricing
from util import cny_per_1k_to_usd_token

PRICING_URL = "https://help.aliyun.com/zh/model-studio/getting-started/models"

# 上下文常量
CTX_1M = 1_000_000
CTX_262K = 262_144
CTX_131K = 131_072
CTX_32K = 32_768
CTX_10M = 10_000_000
MAX_OUT_8K = 8_192
MAX_OUT_32K = 32_768
MAX_OUT_65K = 65_536

# 阶梯分界 (严格 ">", 即 >N 触发下一档; min_tokens = N+1)
T32K = 32_768 + 1      # >32K
T128K = 131_072 + 1    # >128K
T200K = 204_800 + 1    # >200K (开源 coder)
T256K = 262_144 + 1    # >256K

CACHE_HIT_RATIO = 0.10  # 百炼显式缓存命中 = 输入价 ×10%


def _pp(inp: float, out: float, *, cache: float | None = None) -> PlatformPricing:
    return PlatformPricing(
        input_cost_per_token=cny_per_1k_to_usd_token(inp),
        output_cost_per_token=cny_per_1k_to_usd_token(out),
        cache_read_input_token_cost=(
            cny_per_1k_to_usd_token(cache) if cache is not None else None
        ),
    )


def _entry(
    inp: float, out: float, *,
    ctx: int, max_out: int | None = None, cache: bool = False,
    tiers: list[ContextTier] | None = None,
) -> ModelEntry:
    cache_cny = inp * CACHE_HIT_RATIO if cache else None
    pp = _pp(inp, out, cache=cache_cny)
    return ModelEntry(
        input_cost_per_token=pp.input_cost_per_token,
        output_cost_per_token=pp.output_cost_per_token,
        cache_read_input_token_cost=pp.cache_read_input_token_cost or 0.0,
        max_input_tokens=ctx,
        max_output_tokens=max_out,
        context_window=ctx,
        context_tiers=tiers or [],
        pricing={"bailian": pp},
    )


def _tier(min_t: int, inp: float, out: float, *, cache: bool = False) -> ContextTier:
    return ContextTier(
        min_tokens=min_t,
        input_cost_per_token=cny_per_1k_to_usd_token(inp),
        output_cost_per_token=cny_per_1k_to_usd_token(out),
        cache_read_input_token_cost=(
            cny_per_1k_to_usd_token(inp * CACHE_HIT_RATIO) if cache else None
        ),
    )


async def fetch() -> dict[str, ModelEntry]:
    return {
        # ===== 旗舰商业版 =====
        # qwen3-max: ≤32K/≤128K/≤252K 三档, ctx 262144, max_out 65536, 支持缓存
        "qwen3-max": _entry(
            0.0032, 0.0128, ctx=CTX_262K, max_out=MAX_OUT_65K, cache=True,
            tiers=[
                _tier(T32K, 0.0064, 0.0256, cache=True),
                _tier(T128K, 0.0096, 0.0384, cache=True),
            ],
        ),
        # qwen-plus: ≤128K/≤256K/≤1M 三档 (非思考), ctx 1M, max_out 32768
        "qwen-plus": _entry(
            0.0008, 0.002, ctx=CTX_1M, max_out=MAX_OUT_32K,
            tiers=[
                _tier(T128K, 0.0024, 0.02),
                _tier(T256K, 0.0048, 0.048),
            ],
        ),
        # qwen-flash: ≤128K/≤256K/≤1M 三档, ctx 1M, max_out 32768, 支持缓存
        "qwen-flash": _entry(
            0.00015, 0.0015, ctx=CTX_1M, max_out=MAX_OUT_32K, cache=True,
            tiers=[
                _tier(T128K, 0.0006, 0.006, cache=True),
                _tier(T256K, 0.0012, 0.012, cache=True),
            ],
        ),
        # qwen-turbo: 单档 (非思考), ctx 1M, max_out 16384
        "qwen-turbo": _entry(0.0003, 0.0006, ctx=CTX_1M, max_out=16384),
        # qwen-long: 长文档, ctx 10M, max_out 32768
        "qwen-long": _entry(0.0005, 0.002, ctx=CTX_10M, max_out=MAX_OUT_32K),
        # qwq-plus: 推理, ctx 131072, max_out 8192
        "qwq-plus": _entry(0.0016, 0.004, ctx=CTX_131K, max_out=MAX_OUT_8K),

        # ===== Coder 商业版 =====
        # qwen3-coder-plus: ≤32K/≤128K/≤256K/≤1M 四档, ctx 1M, max_out 65536, 支持缓存
        "qwen3-coder-plus": _entry(
            0.004, 0.016, ctx=CTX_1M, max_out=MAX_OUT_65K, cache=True,
            tiers=[
                _tier(T32K, 0.006, 0.024, cache=True),
                _tier(T128K, 0.01, 0.04, cache=True),
                _tier(T256K, 0.02, 0.2, cache=True),
            ],
        ),
        # qwen3-coder-flash: 四档, ctx 1M, max_out 65536, 支持缓存
        "qwen3-coder-flash": _entry(
            0.001, 0.004, ctx=CTX_1M, max_out=MAX_OUT_65K, cache=True,
            tiers=[
                _tier(T32K, 0.0015, 0.006, cache=True),
                _tier(T128K, 0.0025, 0.01, cache=True),
                _tier(T256K, 0.005, 0.025, cache=True),
            ],
        ),
        # 旧 coder 商业版
        "qwen-coder-plus": _entry(0.0035, 0.007, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen-coder-turbo": _entry(0.002, 0.006, ctx=CTX_131K, max_out=MAX_OUT_8K),

        # ===== Coder 开源版 (阶梯, ctx 262144, max_out 65536) =====
        "qwen3-coder-480b-a35b-instruct": _entry(
            0.006, 0.024, ctx=CTX_262K, max_out=MAX_OUT_65K,
            tiers=[
                _tier(T32K, 0.009, 0.036),
                _tier(T128K, 0.015, 0.06),
            ],
        ),
        "qwen3-coder-30b-a3b-instruct": _entry(
            0.0015, 0.006, ctx=CTX_262K, max_out=MAX_OUT_65K,
            tiers=[
                _tier(T32K, 0.00225, 0.009),
                _tier(T128K, 0.00375, 0.015),
            ],
        ),

        # ===== Qwen3 开源版 (非思考模式价, 单档, ctx 131072) =====
        "qwen3-235b-a22b": _entry(0.002, 0.008, ctx=CTX_131K, max_out=16384),
        "qwen3-32b":       _entry(0.002, 0.008, ctx=CTX_131K, max_out=16384),
        "qwen3-30b-a3b":   _entry(0.00075, 0.003, ctx=CTX_131K, max_out=16384),
        "qwen3-14b":       _entry(0.001, 0.004, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen3-8b":        _entry(0.0005, 0.002, ctx=CTX_131K),

        # ===== Qwen2.5 开源版 =====
        "qwen2.5-72b-instruct":     _entry(0.004, 0.012, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen2.5-32b-instruct":     _entry(0.002, 0.006, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen2.5-14b-instruct":     _entry(0.001, 0.003, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen2.5-7b-instruct":      _entry(0.0005, 0.001, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen2.5-coder-32b-instruct": _entry(0.002, 0.006, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen2.5-coder-7b-instruct":  _entry(0.001, 0.002, ctx=CTX_131K, max_out=MAX_OUT_8K),

        # ===== 视觉 (VL) — 按输入+输出总 Token 计费 =====
        # qwen-vl-max / vl-plus: 单档, ctx 131072
        "qwen-vl-max":  _entry(0.0016, 0.004, ctx=CTX_131K, max_out=MAX_OUT_8K),
        "qwen-vl-plus": _entry(0.0008, 0.002, ctx=CTX_131K, max_out=MAX_OUT_8K),
        # qwen3-vl-plus / flash: ≤32K/≤128K/≤256K 阶梯, ctx 262144, max_out 32768
        "qwen3-vl-plus": _entry(
            0.001, 0.01, ctx=CTX_262K, max_out=MAX_OUT_32K,
            tiers=[
                _tier(T32K, 0.0015, 0.015),
                _tier(T128K, 0.003, 0.03),
            ],
        ),
        "qwen3-vl-flash": _entry(
            0.00015, 0.0015, ctx=CTX_262K, max_out=MAX_OUT_32K,
            tiers=[
                _tier(T32K, 0.0003, 0.003),
                _tier(T128K, 0.0006, 0.006),
            ],
        ),
    }
