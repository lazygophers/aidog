"""StepFun (阶跃星辰) 官方一手价 (https://platform.stepfun.com/docs/pricing)。

官方定价页 JS 渲染, 暂无稳定程序化抓取路径。
OpenRouter 骨干已覆盖 stepfun 模型价 (stepfun 平台 + top-level)。
待官方可解析源接入后此处填充一手价; 当前返空。
"""

from __future__ import annotations

from schema import ModelEntry


async def fetch() -> dict[str, ModelEntry]:
    return {}
