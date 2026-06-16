"""Novita 官方一手价 (https://novita.ai/pricing)。

官方定价页 JS 渲染, 暂无稳定程序化抓取路径。
Novita 上游模型多与 OpenRouter 重叠。
待官方可解析源接入后此处填充一手价; 当前返空。
"""

from __future__ import annotations

from schema import ModelEntry


async def fetch() -> dict[str, ModelEntry]:
    return {}
