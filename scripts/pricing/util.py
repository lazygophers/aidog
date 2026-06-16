"""单位换算 + 常量。所有价格最终落 $/token (est_cost 直接乘 token 数)。"""

# 汇率 (静态近似, 用于 CNY 计价平台转 USD; 真实成本以平台币种为准, 此处仅 est_cost 估算)
CNY_PER_USD = 7.2


def per_m(usd_per_million: float) -> float:
    """$/M tokens → $/token。"""
    return usd_per_million / 1_000_000.0


def cny_per_m_to_usd_token(cny_per_million: float) -> float:
    """¥/M tokens → $/token。"""
    return (cny_per_million / CNY_PER_USD) / 1_000_000.0
