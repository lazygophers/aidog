
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
    # 非 aidog 数据：来源标签「CC官方」由 group-route 段显示（见下），此处不加前缀。
    return _seg_group_coding_aidog(inp, o, _stdin_coding_gi(inp))


_seg_group_route_aidog = seg_group_route


def seg_group_route_with_fallback(inp, o, gi):
    # coding 段正在用 stdin 回退（无 aidog tier 且 stdin 有 rate_limits）→ 路由段改显「CC官方」。
    if not (_group_applicable(gi) and gi.get("coding_plan")) and _stdin_coding_gi(inp)["coding_plan"]:
        return "CC官方"
    return _seg_group_route_aidog(inp, o, gi)


RENDERERS["group-coding"] = seg_group_coding_with_fallback
RENDERERS["group-route"] = seg_group_route_with_fallback
