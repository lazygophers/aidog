
# ── 官方 stdin 回退 ──
# aidog 组不可识别（无 ANTHROPIC_BASE_URL）、group-info 查询失败、或 group-info 无 coding tier 时，
# group-coding 改用 Claude Code stdin 的 rate_limits（5h / 7d），口径与 coding tier 一致。
# 组识别且有 tier 时原样走 aidog 数据。

_seg_group_coding_aidog = seg_group_coding


def _stdin_coding_gi(inp):
    plans = []
    for name in ("five_hour", "seven_day"):
        w = get(inp, "rate_limits", name)
        if isinstance(w, dict) and isinstance(w.get("used_percentage"), (int, float)):
            plans.append({"name": name, "utilization": w["used_percentage"], "reset_at": w.get("resets_at")})
    return {"applicable": True, "coding_plan": plans}


def seg_group_coding_with_fallback(inp, o, gi):
    if _group_applicable(gi) and gi.get("coding_plan"):
        return _seg_group_coding_aidog(inp, o, gi)
    # 非 aidog 数据：加来源前缀，避免与 aidog tier 混淆（中文前缀沿用 engine.py 的「缓存 」「余额 」惯例）。
    res = _seg_group_coding_aidog(inp, o, _stdin_coding_gi(inp))
    if isinstance(res, tuple):
        return (res[0], "CC官方 " + res[1])
    return None if res is None else "CC官方 " + res


RENDERERS["group-coding"] = seg_group_coding_with_fallback
