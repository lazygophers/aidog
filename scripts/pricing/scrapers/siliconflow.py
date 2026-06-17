"""SiliconFlow (硅基流动) 官方一手价。

价格源 (公开 SSR, 无需登录): https://www.siliconflow.com/pricing
计费口径 = $/M tokens (USD, 已是美元) → util.per_m。
2026-06-17 核实。收录 Serverless LLM 文本模型 (per-token 计费);
跳过 image/video/audio/embedding/rerank + "Load More" 折叠的长尾模型。

定位: SiliconFlow 是聚合平台 (转售 DeepSeek/Qwen/GLM/Kimi/MiniMax/OpenAI/gemma 等)。
REGISTRY 顺序 siliconflow 在各 first-party 之后 → top-level 价已被 deepseek/glm/kimi/minimax/
bailian 等上游官方 scraper 占; 本 scraper 仅填 pricing["siliconflow"] 作 SF 平台专用价
(用户经 SF 平台调用时 resolve_price 查 pricing["siliconflow"] 命中)。
SF 独家收录 (gpt-oss / gemma-4 / Hunyuan / Hy3 / Nex-N1 / Qwen3.6 等) 则成新 top-level。

无 cache 列的模型 (DeepSeek-V3.2-Exp / V3.1-Terminus / Qwen3.6 / gpt-oss / gemma / Hunyuan /
GLM-4.5-Air) → cache_read 留空。SF 定价无输入长度阶梯 (扁平价), 不用 context_tiers。
max_output SF 页未给 → 留空。上下文长度取页标 K (二进制, 131K=131072 等)。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import per_m

PRICING_URL = "https://www.siliconflow.com/pricing"


def _pp(inp: float, out: float, cache: float | None = None) -> PlatformPricing:
    return PlatformPricing(
        input_cost_per_token=per_m(inp),
        output_cost_per_token=per_m(out),
        cache_read_input_token_cost=(per_m(cache) if cache is not None else None),
    )


def _entry(inp: float, out: float, *, ctx: int, cache: float | None = None) -> ModelEntry:
    pp = _pp(inp, out, cache=cache)
    return ModelEntry(
        input_cost_per_token=pp.input_cost_per_token,
        output_cost_per_token=pp.output_cost_per_token,
        cache_read_input_token_cost=pp.cache_read_input_token_cost or 0.0,
        max_input_tokens=ctx,
        context_window=ctx,
        pricing={"siliconflow": pp},
    )


async def fetch() -> dict[str, ModelEntry]:
    return {
        # ===== DeepSeek =====
        "DeepSeek-V4-Pro":        _entry(1.6,  3.48, ctx=1_048_576, cache=0.145),
        "DeepSeek-V4-Flash":      _entry(0.13, 0.28, ctx=1_048_576, cache=0.028),
        "DeepSeek-V3.2":          _entry(0.27, 0.42, ctx=131_072,   cache=0.135),
        "DeepSeek-V3.2-Exp":      _entry(0.27, 0.41, ctx=131_072),
        "DeepSeek-V3.1-Terminus": _entry(0.27, 1.0,  ctx=131_072),
        # ===== Qwen =====
        "Qwen3.6-27B":       _entry(0.3,   3.2,  ctx=262_144),
        "Qwen3.6-35B-A3B":   _entry(0.2,   1.6,  ctx=262_144),
        "Qwen3.5-9B":        _entry(0.1,   0.15, ctx=262_144),
        "Qwen3.5-122B-A10B": _entry(0.26,  2.08, ctx=262_144),
        "Qwen3.5-27B":       _entry(0.25,  2.0,  ctx=262_144),
        # ===== Z.ai (GLM) =====
        "GLM-5.1":      _entry(1.4,  4.4,  ctx=204_800, cache=0.26),
        "GLM-5":        _entry(0.95, 2.55, ctx=204_800, cache=0.2),
        "GLM-4.7":      _entry(0.42, 2.2,  ctx=204_800, cache=0.11),
        "GLM-4.5-Air":  _entry(0.14, 0.86, ctx=131_072),
        "GLM-5V-Turbo": _entry(1.2,  4.0,  ctx=204_800, cache=0.24),
        # ===== Moonshot (Kimi) =====
        "Kimi-K2.5": _entry(0.45, 2.25, ctx=262_144, cache=0.07),
        "Kimi-K2.6": _entry(0.77, 4.0,  ctx=262_144, cache=0.2),
        # ===== MiniMax =====
        "MiniMax-M2.5": _entry(0.3, 1.2, ctx=196_608,   cache=0.03),
        "MiniMax-M3":   _entry(0.3, 1.2, ctx=1_048_576, cache=0.06),
        # ===== OpenAI (gpt-oss 开源) =====
        "gpt-oss-120b": _entry(0.05, 0.45, ctx=131_072),
        "gpt-oss-20b":  _entry(0.04, 0.18, ctx=131_072),
        # ===== Others =====
        "DeepSeek-V3.1-Nex-N1":  _entry(0.27, 1.0,  ctx=131_072),
        "gemma-4-26B-A4B-it":    _entry(0.12, 0.4,  ctx=262_144),
        "gemma-4-31B-it":        _entry(0.13, 0.4,  ctx=262_144),
        "Hunyuan-A13B-Instruct": _entry(0.14, 0.57, ctx=131_072),
        "Hy3-preview":           _entry(0.066, 0.26, ctx=262_144, cache=0.029),
    }
