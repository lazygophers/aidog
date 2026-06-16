"""Google Gemini 官方一手价 (https://ai.google.dev/gemini-api/docs/pricing)。

USD/M tokens → $/token。取 ≤200k tier (paid standard)。2026-06-16 核实。
Context window = 1M (2.5) / 1M (2.0-flash)。
"""

from __future__ import annotations

from schema import ModelEntry, PlatformPricing
from util import per_m

CTX_1M = 1024 * 1024


async def fetch() -> dict[str, ModelEntry]:
    def _entry(inp_m: float, out_m: float, cache_m: float | None, ctx: int = CTX_1M) -> ModelEntry:
        pp = PlatformPricing(
            input_cost_per_token=per_m(inp_m),
            output_cost_per_token=per_m(out_m),
            cache_read_input_token_cost=per_m(cache_m) if cache_m else None,
        )
        return ModelEntry(
            input_cost_per_token=per_m(inp_m),
            output_cost_per_token=per_m(out_m),
            cache_read_input_token_cost=per_m(cache_m) if cache_m else 0.0,
            max_input_tokens=ctx,
            context_window=ctx,
            pricing={"gemini": pp},
        )

    return {
        "gemini-2.5-pro": _entry(1.25, 10.00, 0.125),
        "gemini-2.5-flash": _entry(0.30, 2.50, 0.03),
        "gemini-2.5-flash-lite": _entry(0.10, 0.40, 0.01),
        "gemini-2.0-flash": _entry(0.10, 0.40, None),
        "gemini-2.0-flash-lite": _entry(0.075, 0.30, None),
    }
