"""Novita AI 官方一手价。

价格源 (公开 SSR, 无需登录): https://novita.ai/zh/pricing
计费口径 = $/M tokens (USD) → util.per_m。
2026-06-17 核实。收录 Serverless LLM 文本模型 (per-token 计费);
跳过 image/video/audio/embedding/rerank + "Tiered pricing"/"全模态"/"免费"/OCR/roleplay niche。

定位: Novita 是聚合平台 (转售 DeepSeek/Qwen/GLM/Kimi/MiniMax/StepFun/Gemma/Llama/
ERNIE/gpt-oss 等)。REGISTRY 顺序 novita 在 siliconflow + litellm + openrouter 之后 (最后) →
- 重叠模型 (DeepSeek-V4-Pro/Flash / Qwen3.6 / GLM / Kimi / MiniMax-M2.5 / gpt-oss / gemma-4 等):
  key 与 siliconflow 同名 → 合并进已有 entry, 补 pricing["novita"] 作 Novita 平台专用价。
- Novita 独家模型 (ERNIE / Nemotron / Llama-4 / Step-3.7 / Kat-Coder / Ring / Ling / Qwen3.7-Max
  / GLM-4.6 / Kimi-K2.7 / MiniMax-M2.7 等) → 成新 top-level entry (default_platform=novita)。

cache: novita "缓存读取" 列 → cache_read; 无该列模型留空。扁平价, 不用 context_tiers。
max_output 页未给 → 留空。上下文取页标 token 数。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import per_m

PRICING_URL = "https://novita.ai/zh/pricing"


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
        pricing={"novita": pp},
    )


async def fetch() -> dict[str, ModelEntry]:
    return {
        # ===== DeepSeek (重叠模型与 siliconflow 同名 → 补 pricing["novita"]) =====
        "DeepSeek-V4-Pro":        _entry(1.6,  3.2,  ctx=1_048_576, cache=0.135),
        "DeepSeek-V4-Flash":      _entry(0.14, 0.28, ctx=1_048_576, cache=0.028),
        "DeepSeek-V3.2":          _entry(0.269, 0.4, ctx=163_840,   cache=0.1345),
        "DeepSeek-V3.2-Exp":      _entry(0.27, 0.41, ctx=163_840),
        "DeepSeek-V3.1-Terminus": _entry(0.27, 1.0,  ctx=131_072,   cache=0.135),
        # DeepSeek 独家
        "DeepSeek-V3.1":      _entry(0.27, 1.0,  ctx=131_072, cache=0.135),
        "DeepSeek-V3-0324":   _entry(0.27, 1.12, ctx=163_840, cache=0.135),
        "DeepSeek-R1-0528":   _entry(0.7,  2.5,  ctx=163_840, cache=0.35),

        # ===== Qwen (重叠: Qwen3.6-27B / 35B-A3B / 122B / 27B-3.5 与 siliconflow 同名) =====
        "Qwen3.6-27B":       _entry(0.6,  3.6,  ctx=262_144),
        "Qwen3.5-27B":       _entry(0.3,  2.4,  ctx=262_144),
        "Qwen3.5-122B-A10B": _entry(0.4,  3.2,  ctx=262_144),
        "Qwen3.6-35B-A3B":   _entry(0.248, 1.485, ctx=262_144),
        # Qwen 独家
        "Qwen3.7-Max":                    _entry(1.25, 3.75, ctx=1_000_000, cache=0.25),
        "Qwen3.5-35B-A3B":                _entry(0.25, 2.0,  ctx=262_144),
        "Qwen3.5-397B-A17B":              _entry(0.6,  3.6,  ctx=262_144),
        "Qwen3-Coder-Next":               _entry(0.2,  1.5,  ctx=262_144),
        "Qwen3-Coder-480B-A35B-Instruct": _entry(0.38, 1.55, ctx=262_144),
        "Qwen3-Coder-30B-A3B-Instruct":   _entry(0.07, 0.27, ctx=160_000),
        "Qwen3-VL-235B-A22B-Thinking":    _entry(0.98, 3.95, ctx=131_072),
        "Qwen3-VL-235B-A22B-Instruct":    _entry(0.3,  1.5,  ctx=131_072),
        "Qwen3-Next-80B-A3B-Instruct":    _entry(0.15, 1.5,  ctx=131_072),
        "Qwen3-Next-80B-A3B-Thinking":    _entry(0.15, 1.5,  ctx=131_072),
        "Qwen3-235B-A22B-Thinking-2507":  _entry(0.3,  3.0,  ctx=131_072),
        "Qwen3-235B-A22B-Instruct-2507":  _entry(0.09, 0.58, ctx=131_072),
        "Qwen3-235B-A22B":                _entry(0.2,  0.8,  ctx=40_960),
        "Qwen2.5-72B-Instruct":           _entry(0.38, 0.4,  ctx=32_000),
        "Qwen3-VL-8B-Instruct":           _entry(0.08, 0.5,  ctx=131_072),
        "Qwen3-VL-30B-A3B-Instruct":      _entry(0.2,  0.7,  ctx=131_072),
        "Qwen3-VL-30B-A3B-Thinking":      _entry(0.2,  1.0,  ctx=131_072),
        "Qwen-MT-Plus":                   _entry(0.25, 0.75, ctx=16_384),

        # ===== Baidu ERNIE (独家) =====
        "CoBuddy":                _entry(0.28, 1.13, ctx=131_072, cache=0.07),
        "ERNIE-4.5-VL-424B-A47B": _entry(0.42, 1.25, ctx=123_000),
        "ERNIE-4.5-VL-28B-A3B":   _entry(0.14, 0.56, ctx=30_000),
        "ERNIE-4.5-21B-A3B":      _entry(0.07, 0.28, ctx=120_000),

        # ===== Z.ai GLM (重叠: GLM-5.1/5/4.7/4.5-Air 与 siliconflow 同名) =====
        "GLM-5.1":     _entry(1.38, 4.4,  ctx=204_800, cache=0.26),
        "GLM-5":       _entry(1.0,  3.2,  ctx=202_800, cache=0.2),
        "GLM-4.7":     _entry(0.6,  2.2,  ctx=204_800, cache=0.11),
        "GLM-4.5-Air": _entry(0.13, 0.85, ctx=131_072, cache=0.025),
        # GLM 独家
        "GLM-4.7-Flash": _entry(0.07, 0.4,  ctx=200_000, cache=0.01),
        "GLM-4.6":       _entry(0.55, 2.2,  ctx=204_800, cache=0.11),
        "GLM-4.5":       _entry(0.6,  2.2,  ctx=131_072, cache=0.11),
        "GLM-4.6V":      _entry(0.3,  0.9,  ctx=131_072, cache=0.055),
        "GLM-4.5V":      _entry(0.6,  1.8,  ctx=65_536,  cache=0.11),

        # ===== Moonshot Kimi (重叠: K2.5/K2.6 与 siliconflow 同名) =====
        "Kimi-K2.5": _entry(0.6, 3.0, ctx=262_144, cache=0.1),
        "Kimi-K2.6": _entry(0.8, 3.4, ctx=262_144, cache=0.16),
        # Kimi 独家
        "Kimi-K2.7-Code":   _entry(0.95, 4.0, ctx=262_144, cache=0.19),
        "Kimi-K2-Thinking": _entry(0.6,  2.5, ctx=262_144, cache=0.15),
        "Kimi-K2-0905":     _entry(0.6,  2.5, ctx=262_144),
        "Kimi-K2-Instruct": _entry(0.57, 2.3, ctx=131_072),

        # ===== MiniMax (重叠: M2.5 与 siliconflow 同名) =====
        "MiniMax-M2.5": _entry(0.3, 1.2, ctx=204_800, cache=0.03),
        # MiniMax 独家
        "MiniMax-M2.7":            _entry(0.3, 1.2, ctx=204_800, cache=0.06),
        "MiniMax-M2.5-highspeed":  _entry(0.6, 2.4, ctx=204_800, cache=0.03),
        "MiniMax-M2.1":            _entry(0.3, 1.2, ctx=204_800, cache=0.03),
        "MiniMax-M2":              _entry(0.3, 1.2, ctx=204_800, cache=0.03),
        "MiniMax-M1":              _entry(0.55, 2.2, ctx=1_000_000),

        # ===== StepFun (独家; StepFun 自家一手价 scraper 仍占位, 此处先补) =====
        "Step-3.7-Flash": _entry(0.2, 1.15, ctx=262_144, cache=0.04),

        # ===== Nvidia (独家) =====
        "Nemotron-3-Nano-30B-A3B": _entry(0.05, 0.2, ctx=262_144),

        # ===== Gemma (重叠: gemma-4-26B/31B 与 siliconflow 同名; siliconflow key 连字符小写) =====
        "gemma-4-26B-A4B-it": _entry(0.13, 0.4, ctx=262_144),
        "gemma-4-31B-it":     _entry(0.14, 0.4, ctx=262_144),
        # Gemma 独家
        "Gemma-3-27B": _entry(0.119, 0.2, ctx=98_304),

        # ===== OpenAI gpt-oss (重叠: 与 siliconflow 同名) =====
        "gpt-oss-120b": _entry(0.05, 0.25, ctx=131_072),
        "gpt-oss-20b":  _entry(0.04, 0.15, ctx=131_072),

        # ===== KwaiKAT (独家) =====
        "Kat-Coder-Pro": _entry(0.3, 1.2, ctx=256_000, cache=0.06),

        # ===== Llama (独家) =====
        "Llama-3.1-8B-Instruct":    _entry(0.02,  0.05, ctx=16_384),
        "Llama-3.3-70B-Instruct":   _entry(0.135, 0.4,  ctx=131_072),
        "Llama-4-Maverick-Instruct": _entry(0.27, 0.85, ctx=1_048_576),
        "Llama-4-Scout-Instruct":   _entry(0.18,  0.59, ctx=131_072),

        # ===== Mistral (独家) =====
        "Mistral-Nemo": _entry(0.04, 0.17, ctx=60_288),

        # ===== Others (XiaomiMiMo / Ring / Ling / WizardLM 独家) =====
        "MiMo-V2.5":     _entry(0.168, 0.336, ctx=1_048_576, cache=0.0034),
        "MiMo-V2.5-Pro": _entry(0.522, 1.044, ctx=1_048_576, cache=0.0043),
        "Ring-2.6-1T":   _entry(0.3,   2.5,   ctx=262_144, cache=0.06),
        "Ling-2.6-flash": _entry(0.1,  0.3,   ctx=262_144, cache=0.02),
        "Ling-2.6-1T":   _entry(0.3,   2.5,   ctx=262_144, cache=0.06),
        "WizardLM-2-8x22B": _entry(0.62, 0.62, ctx=65_535),
    }
