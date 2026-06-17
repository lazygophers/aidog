"""SiliconFlow 官方一手价 (https://siliconflow.cn/pricing)。

/v1/models 需鉴权返回非标准结构, 官方定价页 JS 渲染。
OpenRouter 骨干已覆盖 SiliconFlow 上游模型价。
待官方可解析源接入后此处填充一手价; 当前返空。
"""

from __future__ import annotations

from schema import ModelEntry


async def fetch() -> dict[str, ModelEntry]:
    return {}
