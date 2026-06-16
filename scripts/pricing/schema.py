"""data/models.json 的 pydantic schema — 单一事实源。

Rust 端 ModelPrice / ModelPriceSummary 字段必须与此对齐 (见 design.md)。
所有价格单位 = $/token (与 LiteLLM 惯例一致, est_cost 直接乘 token 数)。
"""

from __future__ import annotations

from datetime import datetime, timezone
from typing import Optional

from pydantic import BaseModel, Field


class PlatformPricing(BaseModel):
    """单个平台对该模型的定价覆盖 (resolve_price 回退链 pricing[platform_type])。"""

    input_cost_per_token: Optional[float] = None
    output_cost_per_token: Optional[float] = None
    cache_read_input_token_cost: Optional[float] = None
    cache_creation_input_token_cost: Optional[float] = None


class ModelEntry(BaseModel):
    """单个模型的完整信息 (price + max_tokens + context)。"""

    default_platform: Optional[str] = None
    input_cost_per_token: float = 0.0
    output_cost_per_token: float = 0.0
    cache_read_input_token_cost: float = 0.0
    cache_creation_input_token_cost: Optional[float] = None

    # 上限 (Q3: 出站仅当客户端 max_tokens 超过 max_output_tokens 时裁剪; 模型无值不裁)
    max_input_tokens: Optional[int] = None
    max_output_tokens: Optional[int] = None
    context_window: Optional[int] = None

    # per-platform 价格覆盖 (key = platform_type serde 裸名, 如 "deepseek"/"openrouter")
    pricing: dict[str, PlatformPricing] = Field(default_factory=dict)


class ModelsFile(BaseModel):
    """data/models.json 顶层结构 — GitHub 仓库唯一信源。"""

    version: int = 1
    generated_at: str = Field(
        default_factory=lambda: datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    )
    models: dict[str, ModelEntry] = Field(default_factory=dict)
