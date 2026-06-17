"""Kimi (月之暗面 moonshot) 官方一手价 (https://platform.moonshot.cn/docs/pricing)。

官方定价页 JS 渲染, 暂无稳定程序化抓取路径。
OpenRouter 骨干已覆盖 moonshotai 模型价 (kimi 平台 + top-level)。
待官方 API 或可解析源接入后此处填充一手价; 当前返空。
"""

from __future__ import annotations

from schema import ModelEntry


async def fetch() -> dict[str, ModelEntry]:
    return {}
