"""scraper 注册表 — 每平台一个独立模块。

约定: 每个模块导出 `PLATFORM: str` (platform_type serde 裸名) 和
`async def fetch() -> dict[str, ModelEntry]` (key = 模型名)。

aggregate.py 通过 REGISTRY 自动发现并调用, 新增平台只需:
1. 新建 scrapers/<name>.py
2. 在此 REGISTRY 注册
"""
from . import (
    anthropic,
    bailian,
    deepseek,
    gemini,
    glm,
    kimi,
    litellm,
    minimax,
    novita,
    openai,
    openrouter,
    siliconflow,
    stepfun,
    xiaomi,
)

# (module, platform_type) — platform_type 须与 Rust Protocol serde 裸名一致
# 顺序: first-party 一手价优先 → litellm 骨干兜底 → openrouter 骨干兜底 → 第三方
REGISTRY = [
    (deepseek, "deepseek"),
    (openai, "openai"),
    (anthropic, "anthropic"),
    (gemini, "gemini"),
    (glm, "glm"),
    (kimi, "kimi"),
    (minimax, "minimax"),
    (xiaomi, "xiaomi_mimo"),
    (bailian, "bailian"),
    (siliconflow, "siliconflow"),
    (litellm, "litellm"),
    (openrouter, "openrouter"),
    (novita, "novita"),
    (stepfun, "stepfun"),
]

__all__ = ["REGISTRY"]
