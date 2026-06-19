// AUTO-GENERATED from scripts/statusline-golden/engine.py — DO NOT EDIT.
// Regenerate: node scripts/build-statusline-runtime.mjs
// The statusline runtime engine, embedded verbatim into generated .py scripts.
/* eslint-disable */
export const ENGINE_PY: string = `# Statusline rendering engine (Python port of the former bash generators).
# Byte-for-byte compatible with the jq/printf/awk/sed bash implementation.
# stdlib only. This module is embedded verbatim into the generated scripts.
import sys, os, json, math, re, subprocess

# ── jq-equivalent numeric helpers ──

def jround(x):
    # jq \`round\`: round half away from zero.
    if x is None:
        return 0
    return math.floor(x + 0.5) if x >= 0 else math.ceil(x - 0.5)

def jfloor(x):
    return math.floor(x)

def jts(x):
    # jq \`tostring\`: numbers → shortest decimal (ints have no fractional part);
    # strings pass through unchanged; booleans → "true"/"false"; null → "null".
    if x is None:
        return "null"
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, str):
        return x
    if isinstance(x, float) and x.is_integer():
        x = int(x)
    if isinstance(x, int):
        return str(x)
    return repr(x)

def get(obj, *path, default=None):
    cur = obj
    for k in path:
        if isinstance(cur, dict) and k in cur and cur[k] is not None:
            cur = cur[k]
        else:
            return default
    return cur

def km_abbrev(n):
    # \`if . >= 1000000 then ((./100000|round)/10)+"M" elif .>=1000 then ((./100|round)/10)+"K" else tostring\`
    if n >= 1000000:
        return jts(jround(n / 100000) / 10) + "M"
    if n >= 1000:
        return jts(jround(n / 100) / 10) + "K"
    return jts(n)

def km_abbrev_max(n):
    # context-max abbrev: K branch uses (./1000|round) (no /10).
    if n >= 1000000:
        return jts(jround(n / 100000) / 10) + "M"
    if n >= 1000:
        return jts(jround(n / 1000)) + "K"
    return jts(n)

def dur_human(ms):
    s = jfloor(ms / 1000)
    pad2 = lambda v: (lambda t: "0" + t if len(t) < 2 else t)(str(v))
    if s >= 3600:
        return jts(jfloor(s / 3600)) + "h" + pad2(jfloor((s % 3600) / 60)) + "m"
    if s >= 60:
        return jts(jfloor(s / 60)) + "m" + pad2(s % 60) + "s"
    return jts(s) + "s"

# ── ANSI helpers ──

RESET = "\\033[0m"

def fg(rgb, text):
    return "\\033[38;2;%sm%s%s" % (rgb, text, RESET)

# Catppuccin / semantic truecolor triples (kept verbatim from bash).
ANSI_RED = "255;69;58"
ANSI_AMBER = "255;214;10"
ANSI_GREEN = "52;199;89"
CAT_MAUVE = "203;166;247"
CAT_BLUE = "137;180;250"
CAT_YELLOW = "249;226;175"
CAT_GREEN = "166;227;161"
CAT_RED = "243;139;168"
CAT_SUBTLE = "108;112;134"

# ── group-info cache (fetched once, shared by group-* segments) ──

def fetch_group_info():
    base = os.environ.get("ANTHROPIC_BASE_URL", "")
    if not base:
        return None
    # scheme://host[:port] — strip any path suffix (/v1, /api/paas/v4, …).
    try:
        scheme, rest = base.split("://", 1)
        host = rest.split("/", 1)[0]
        root = scheme + "://" + host
    except ValueError:
        return None
    token = os.environ.get("ANTHROPIC_AUTH_TOKEN", "")
    try:
        import urllib.request
        req = urllib.request.Request(
            root + "/api/group-info", method="POST",
            headers={"Authorization": "Bearer " + token},
        )
        with urllib.request.urlopen(req, timeout=1) as r:
            return json.loads(r.read().decode("utf-8"))
    except Exception:
        return None


# ─── Segment renderers (return the segment body text, or None to drop) ───

def seg_model(inp, o):
    if o.get("format") == "full":
        return get(inp, "model", "id", default="claude")
    return get(inp, "model", "display_name", default="Claude")

def seg_context_bar(inp, o):
    w = o.get("width") or 10
    pct = jround(get(inp, "context_window", "used_percentage", default=0))
    filled = (pct * w) // 100
    empty = w - filled
    bar = (o.get("filled") or "▓") * filled + (o.get("empty") or "░") * empty
    return bar + " " + str(pct) + "%"

def seg_context_pct(inp, o):
    if o.get("degradeZero"):
        pct = jround(get(inp, "context_window", "used_percentage", default=0))
        return (str(pct) + "%") if pct > 0 else None
    pct = jround(get(inp, "context_window", "used_percentage", default=0))
    return str(pct) + "%"

def seg_git(inp, o):
    if o.get("showRepo"):
        owner = get(inp, "workspace", "repo", "owner")
        name = get(inp, "workspace", "repo", "name")
        # jq: \`[owner, name] | join("/") // "detached"\`. jq \`join\` converts nulls
        # to "" and joins ALL elements (so [null,null] → "/"). \`//\` only falls
        # back on null/false, never on the (truthy) "" / "/" result.
        joined = "/".join("" if p is None else str(p) for p in (owner, name))
        return joined
    return get(inp, "workspace", "repo", "name", default="detached")

def seg_cost(inp, o):
    usd = get(inp, "cost", "total_cost_usd", default=0)
    out = "$%.2f" % (jround(usd * 100) / 100)
    if o.get("showDuration"):
        dur = get(inp, "cost", "total_duration_ms", default=0)
        out += " · " + str(jround(dur / 1000)) + "s"
    return out

def seg_rate_limits(inp, o):
    w = o.get("windows")
    def pct(win):
        v = get(inp, "rate_limits", win, "used_percentage")
        return "?" if v is None else jts(v)
    if w == "5h":
        return "5h:" + pct("five_hour") + "%"
    if w == "7d":
        return "7d:" + pct("seven_day") + "%"
    return "5h:" + pct("five_hour") + "% 7d:" + pct("seven_day") + "%"

def seg_effort(inp, o):
    return get(inp, "effort", "level", default="") or None

def seg_vim(inp, o):
    return get(inp, "vim", "mode", default="") or None

def seg_separator(inp, o):
    c = o.get("char")
    return c if isinstance(c, str) else "·"

# ── atomic cost/exec ──

def seg_cost_usd(inp, o):
    usd = get(inp, "cost", "total_cost_usd")
    if usd is None:
        return None
    return o.get("prefix", "$") + jts(jround(usd * 100) / 100)

def seg_session_duration(inp, o):
    ms = get(inp, "cost", "total_duration_ms")
    if ms is None:
        return None
    return (jts(ms) + "ms") if o.get("format") == "ms" else dur_human(ms)

def seg_api_duration(inp, o):
    ms = get(inp, "cost", "total_api_duration_ms")
    if ms is None:
        return None
    return (jts(ms) + "ms") if o.get("format") == "ms" else dur_human(ms)

def seg_lines_changed(inp, o):
    a = get(inp, "cost", "total_lines_added", default=0)
    r = get(inp, "cost", "total_lines_removed", default=0)
    return "+" + jts(a) + " -" + jts(r)

# ── context window ──

def seg_context_tokens(inp, o):
    abbrev = o.get("abbrev")
    fmt = (lambda n: km_abbrev(n)) if abbrev else (lambda n: jts(n))
    if o.get("mode") == "sum":
        cw = get(inp, "context_window")
        if cw is None:
            return None
        tot = (get(inp, "context_window", "total_input_tokens", default=0)
               + get(inp, "context_window", "total_output_tokens", default=0))
        return fmt(tot) if tot > 0 else None
    i = get(inp, "context_window", "total_input_tokens")
    if i is None:
        return None
    out = get(inp, "context_window", "total_output_tokens", default=0)
    return fmt(i) + "/" + fmt(out)

def seg_context_max(inp, o):
    v = get(inp, "context_window", "context_window_size")
    if v is None:
        return None
    return km_abbrev_max(v) if o.get("abbrev") else jts(v)

def seg_context_remaining(inp, o):
    v = get(inp, "context_window", "remaining_percentage")
    if v is None:
        return None
    return jts(jround(v)) + "%"

def seg_context_cache(inp, o):
    if o.get("mode") == "hitrate":
        cu = get(inp, "context_window", "current_usage")
        if cu is None:
            rate = 0
        else:
            r = get(inp, "context_window", "current_usage", "cache_read_input_tokens", default=0)
            i = get(inp, "context_window", "current_usage", "input_tokens", default=0)
            rate = (r / (i + r) * 100) if (i + r) > 0 else 0
        # Mirror bash \`printf '%.4f' | sed 's/0*$//; s/\\\\\\.$//'\`: the trailing-dot
        # strip is a no-op in the original (the over-escaped sequence \`\\\\\\.\`
        # matches a literal backslash + dot, never present), so only the trailing
        # zeros are removed — leaving a bare \`.\` (e.g. 100.0000 → "100."). Replicate.
        s = "%.4f" % rate
        s = re.sub(r"0*$", "", s)
        return (o.get("prefix", "缓存 ")) + s + "%"
    abbrev = o.get("abbrev")
    fmt = (lambda n: km_abbrev(n)) if abbrev else (lambda n: jts(n))
    cu = get(inp, "context_window", "current_usage")
    if cu is None:
        return None
    w = get(inp, "context_window", "current_usage", "cache_creation_input_tokens", default=0)
    r = get(inp, "context_window", "current_usage", "cache_read_input_tokens", default=0)
    return "w" + fmt(w) + " r" + fmt(r)

# ── rate limits (per window) ──

def _rl(inp, win, label, hours_div, show):
    v = get(inp, "rate_limits", win, "used_percentage")
    if v is None:
        return None
    out = label + ":" + jts(jround(v)) + "%"
    # showReset uses jq \`now\` (real time); fixtures omit resets_at so the branch
    # is skipped. resets_at present → " (Xm)"/" (Xh)" using current time.
    if show:
        ra = get(inp, "rate_limits", win, "resets_at")
        if ra is not None:
            import time
            delta = ra - time.time()
            out += " (" + jts(jfloor(delta / hours_div)) + ("m)" if hours_div == 60 else "h)")
    return out

def seg_rate_limit_5h(inp, o):
    return _rl(inp, "five_hour", "5h", 60, o.get("showReset"))

def seg_rate_limit_7d(inp, o):
    return _rl(inp, "seven_day", "7d", 3600, o.get("showReset"))

# ── git / dir / session ──

def seg_git_branch(inp, o):
    cwd = get(inp, "workspace", "current_dir") or get(inp, "cwd") or "."
    try:
        b = subprocess.run(["git", "-C", cwd, "branch", "--show-current"],
                           capture_output=True, text=True).stdout.strip()
    except Exception:
        b = ""
    return b or None

def _atom(inp, *path):
    return get(inp, *path) or None

def seg_git_host(inp, o): return _atom(inp, "workspace", "repo", "host")
def seg_git_owner(inp, o): return _atom(inp, "workspace", "repo", "owner")
def seg_git_repo(inp, o): return _atom(inp, "workspace", "repo", "name")

def seg_git_repo_full(inp, o):
    name = get(inp, "workspace", "repo", "name")
    if name is None:
        return None
    owner = get(inp, "workspace", "repo", "owner", default="")
    return owner + "/" + name

def seg_git_worktree(inp, o): return _atom(inp, "workspace", "git_worktree")

def _basename(p):
    return p.split("/")[-1]

def seg_cwd(inp, o):
    v = get(inp, "workspace", "current_dir")
    if v is None:
        return None
    return v if o.get("format") == "full" else _basename(v)

def seg_project_dir(inp, o):
    v = get(inp, "workspace", "project_dir")
    if v is None:
        return None
    return v if o.get("format") == "full" else _basename(v)

def seg_added_dirs(inp, o):
    arr = get(inp, "workspace", "added_dirs", default=[])
    if not arr:
        return None
    return ",".join(_basename(p) for p in arr)

def seg_session_id(inp, o):
    v = get(inp, "session_id")
    if v is None:
        return None
    return v[0:8] if o.get("truncate") else v

def seg_session_name(inp, o): return _atom(inp, "session_name")

def seg_transcript_path(inp, o):
    v = get(inp, "transcript_path")
    if v is None:
        return None
    return v if o.get("format") == "full" else _basename(v)

def seg_worktree_name(inp, o): return _atom(inp, "worktree", "name")
def seg_worktree_branch(inp, o): return _atom(inp, "worktree", "branch")
def seg_worktree_original_branch(inp, o): return _atom(inp, "worktree", "original_branch")

def seg_pr_number(inp, o):
    v = get(inp, "pr", "number")
    if v is None:
        return None
    return o.get("prefix", "#") + jts(v)

def seg_pr_url(inp, o): return _atom(inp, "pr", "url")
def seg_pr_state(inp, o): return _atom(inp, "pr", "review_state")

def seg_version(inp, o):
    v = get(inp, "version")
    if v is None:
        return None
    return o.get("prefix", "v") + v
def seg_output_style(inp, o): return _atom(inp, "output_style", "name")

def seg_thinking(inp, o):
    return (o.get("label", "thinking")) if get(inp, "thinking", "enabled") is True else None

def seg_token_warn(inp, o):
    return (o.get("label", "⚠200k")) if get(inp, "exceeds_200k_tokens") is True else None

def seg_agent(inp, o): return _atom(inp, "agent", "name")

def seg_agent_badge(inp, o):
    typ = get(inp, "type", default="")
    if not typ:
        return None
    tl = "Agent" if str(typ).lower() == "local_agent" else typ
    status = (get(inp, "status", default="") or "").lower()
    if status in ("running", "in_progress"):
        sym, sc = "●", CAT_YELLOW
    elif status in ("pending", "queued"):
        sym, sc = "○", CAT_SUBTLE
    elif status in ("completed", "succeeded", "success"):
        sym, sc = "●", CAT_GREEN
    elif status in ("failed", "error"):
        sym, sc = "●", CAT_RED
    elif status in ("cancelled", "canceled"):
        sym, sc = "◌", CAT_SUBTLE
    else:
        sym, sc = "◆", CAT_BLUE
    model = _model_label(inp)
    out = "\\033[38;2;%sm[%s%s" % (CAT_MAUVE, tl, RESET)
    out += "\\033[1m\\033[38;2;%sm·%s%s" % (sc, sym, RESET)
    if model:
        out += "\\033[38;2;%sm·%s%s" % (CAT_BLUE, model, RESET)
    out += "\\033[1m\\033[38;2;%sm]%s" % (CAT_MAUVE, RESET)
    return out

def _model_label(inp):
    m = inp.get("model")
    if isinstance(m, dict):
        v = m.get("display_name")
        if v is None:
            v = m.get("id")
        if v is not None:
            return v
    elif isinstance(m, str):
        return m
    return ""

def seg_custom(inp, o):
    # Only \`.model.display_name\`-style simple paths appear in defaults; emulate
    # the jq \`-r\` path lookup with \`//\` fallbacks for the known expressions.
    expr = o.get("expr", ".model.display_name")
    return _eval_jq_path(inp, expr)

def _eval_jq_path(inp, expr):
    # Supports \`a.b // c.d // "lit"\` chains of dotted paths / string literals.
    for alt in expr.split("//"):
        alt = alt.strip()
        if not alt:
            continue
        if alt.startswith('"') and alt.endswith('"'):
            return alt[1:-1]
        path = [p for p in alt.lstrip(".").split(".") if p]
        v = get(inp, *path)
        if v is not None:
            return v if isinstance(v, str) else jts(v)
    return None

# ── group segments ──

def _group_applicable(gi):
    return isinstance(gi, dict) and gi.get("applicable") is True

def seg_group_balance(inp, o, gi):
    if not _group_applicable(gi):
        return None
    bal = jround(get(gi, "balance", default=0) * 100) / 100
    if o.get("dynamicColor"):
        # dynamic path: printf '%g' then level color; this returns colored text.
        bal_g = _pct_g(bal)
        if bal_g == "0" or bal_g == "0.0":
            return None
        txt = (o.get("prefix", "余额 ")) + bal_g
        lvl = get(gi, "balance_level", default="neutral")
        c = ANSI_RED if lvl == "red" else ANSI_AMBER if lvl == "yellow" else ANSI_GREEN
        return ("DYN", fg(c, txt))
    val = "%.2f" % bal
    return (o.get("prefix", "余额 ")) + val

def _pct_g(x):
    # printf '%g' — shortest, trims trailing zeros.
    s = "%g" % x
    return s

def seg_group_spent(inp, o, gi):
    if not _group_applicable(gi):
        return None
    val = "%.2f" % (jround(get(gi, "spent", default=0) * 100) / 100)
    return (o.get("prefix", "$")) + val

def _coding_text(gi):
    plans = get(gi, "coding_plan", default=[])
    parts = []
    for p in plans:
        name = p.get("name")
        if name == "five_hour":
            nm = "5h"
        elif name in ("seven_day", "weekly_limit"):
            nm = "7d"
        else:
            nm = name
        # coding plan 配额统一显「剩余%」(= 100 - utilization), 与平台卡片口径一致;
        # 末尾「剩」字明确标注为剩余而非已用, 避免歧义。utilization 可能为 None → 当 0。
        util = jround(p.get("utilization", 0) or 0)
        remain = util
        if remain < 0:
            remain = 0
        elif remain > 100:
            remain = 100
        remain = 100 - remain
        parts.append(nm + " " + jts(remain) + "%剩")
    return "·".join(parts)

def seg_group_coding(inp, o, gi):
    if not _group_applicable(gi):
        return None
    plans = get(gi, "coding_plan", default=[])
    if not plans:
        return None
    txt = _coding_text(gi)
    if not txt:
        return None
    if o.get("dynamicColor"):
        levels = [p.get("level") for p in plans]
        if "red" in levels:
            c = ANSI_RED
            resets = [p.get("reset_at") for p in plans if p.get("level") == "red" and p.get("reset_at") is not None]
            if resets:
                rs = min(resets)
                now = _now_epoch()
                d = rs - now
                if d > 0:
                    h = d // 3600
                    m = (d % 3600) // 60
                    txt = txt + " (reset " + str(h) + "h" + str(m) + "m)"
        elif "yellow" in levels:
            c = ANSI_AMBER
        else:
            c = ANSI_GREEN
        return ("DYN", fg(c, txt))
    return txt

def seg_group_requests(inp, o, gi):
    if not _group_applicable(gi):
        return None
    req = get(gi, "requests", default=0)
    sr = jround(get(gi, "success_rate", default=0))
    return jts(req) + "·" + jts(sr) + "%"

def seg_group_cache(inp, o, gi):
    if not _group_applicable(gi):
        return None
    return (o.get("prefix", "缓存 ")) + jts(jround(get(gi, "cache_rate", default=0))) + "%"

def seg_group_tokens(inp, o, gi):
    if not _group_applicable(gi):
        return None
    t = get(gi, "total_tokens", default=0)
    return (o.get("prefix", "")) + km_abbrev(t)


def _now_epoch():
    # Mirror bash \`date +%s\` (honours the test date shim via subprocess).
    try:
        return int(subprocess.run(["date", "+%s"], capture_output=True, text=True).stdout.strip())
    except Exception:
        import time
        return int(time.time())


RENDERERS = {
    "model": seg_model, "context-bar": seg_context_bar, "context-pct": seg_context_pct,
    "git": seg_git, "cost": seg_cost, "rate-limits": seg_rate_limits, "effort": seg_effort,
    "vim": seg_vim, "separator": seg_separator, "cost-usd": seg_cost_usd,
    "session-duration": seg_session_duration, "api-duration": seg_api_duration,
    "lines-changed": seg_lines_changed, "context-tokens": seg_context_tokens,
    "context-max": seg_context_max, "context-remaining": seg_context_remaining,
    "context-cache": seg_context_cache, "rate-limit-5h": seg_rate_limit_5h,
    "rate-limit-7d": seg_rate_limit_7d, "git-branch": seg_git_branch,
    "git-host": seg_git_host, "git-owner": seg_git_owner, "git-repo": seg_git_repo,
    "git-repo-full": seg_git_repo_full, "git-worktree": seg_git_worktree, "cwd": seg_cwd,
    "project-dir": seg_project_dir, "added-dirs": seg_added_dirs, "session-id": seg_session_id,
    "session-name": seg_session_name, "transcript-path": seg_transcript_path,
    "worktree-name": seg_worktree_name, "worktree-branch": seg_worktree_branch,
    "worktree-original-branch": seg_worktree_original_branch, "pr-number": seg_pr_number,
    "pr-url": seg_pr_url, "pr-state": seg_pr_state, "version": seg_version,
    "output-style": seg_output_style, "thinking": seg_thinking, "token-warn": seg_token_warn,
    "agent": seg_agent, "agent-badge": seg_agent_badge, "custom": seg_custom,
    "group-balance": seg_group_balance, "group-spent": seg_group_spent,
    "group-coding": seg_group_coding, "group-requests": seg_group_requests,
    "group-cache": seg_group_cache, "group-tokens": seg_group_tokens,
}

GROUP_TYPES = {"group-balance", "group-spent", "group-coding", "group-requests",
               "group-cache", "group-tokens"}
VALUE_COLORABLE = {"context-pct", "context-bar", "cost", "rate-limits", "cost-usd",
                   "context-remaining", "rate-limit-5h", "rate-limit-7d",
                   "session-duration", "api-duration"}

# ── auto-color thresholds (mirror bash autoColorBash) ──

def auto_color(seg_type, inp, text):
    if seg_type in ("context-pct", "context-bar"):
        m = jround(get(inp, "context_window", "used_percentage", default=0))
        c = _pct_thresh(m)
    elif seg_type == "context-remaining":
        m = jround(get(inp, "context_window", "remaining_percentage", default=100))
        c = ANSI_RED if m < 20 else "255;159;10" if m < 40 else ANSI_GREEN
    elif seg_type in ("cost", "cost-usd"):
        m = jround(get(inp, "cost", "total_cost_usd", default=0) * 100)
        c = ANSI_RED if m > 1000 else "255;159;10" if m > 100 else ANSI_GREEN
    elif seg_type == "rate-limit-5h":
        m = jround(get(inp, "rate_limits", "five_hour", "used_percentage", default=0))
        c = _pct_thresh(m)
    elif seg_type == "rate-limit-7d":
        m = jround(get(inp, "rate_limits", "seven_day", "used_percentage", default=0))
        c = _pct_thresh(m)
    elif seg_type in ("session-duration", "api-duration"):
        field = "total_api_duration_ms" if seg_type == "api-duration" else "total_duration_ms"
        m = jround(get(inp, "cost", field, default=0) / 1000)
        c = ANSI_RED if m > 300 else "255;159;10" if m > 60 else ANSI_GREEN
    else:  # rate-limits — higher of 5h/7d
        a = get(inp, "rate_limits", "five_hour", "used_percentage", default=0)
        b = get(inp, "rate_limits", "seven_day", "used_percentage", default=0)
        m = jround(max(a, b))
        c = _pct_thresh(m)
    return fg(c, text)

def _pct_thresh(m):
    return ANSI_RED if m > 80 else "255;159;10" if m > 60 else ANSI_GREEN


def render_segment(seg, inp, gi):
    """Render one segment → final (possibly ANSI-wrapped) string, or '' when empty.

    Mirrors the bash pipeline exactly:
      body → wrapAffix(body, pre, suf) → [fixed/auto color] → __seg
    where:
      • wrapAffix: when an affix is set, an empty body drops the whole unit
        (affix included). When no affix, the body passes through verbatim.
      • a fixed/auto color wrapper ALWAYS emits its ANSI codes — even around an
        empty captured body (bash \`printf '\\\\033[..m%s\\\\033[0m' ""\` → bare codes).
        So a colored segment with no value still emits empty color codes.
      • an uncolored segment with empty body emits nothing.
      • dynamic-color segments (group*/agent-badge) carry their own ANSI and are
        treated as a plain body for wrapAffix; they self-color (no extra wrap).
    """
    fn = RENDERERS.get(seg["type"])
    if fn is None:
        return ""
    o = seg.get("opts", {})
    if seg["type"] in GROUP_TYPES:
        res = fn(inp, o, gi)
    else:
        res = fn(inp, o)
    # Dynamic-color segments return ("DYN", coloredText) and self-color.
    dyn = isinstance(res, tuple) and len(res) == 2 and res[0] == "DYN"
    body = res[1] if dyn else res
    empty = body is None or body == ""

    affix_pre = o.get("affixPre", "")
    affix_suf = o.get("affixSuf", "")
    has_affix = bool(affix_pre) or bool(affix_suf)

    # wrapAffix result (the "captured body" the color wrapper sees).
    if has_affix:
        wrapped = "" if empty else (affix_pre + body + affix_suf)
    else:
        wrapped = "" if empty else body

    if dyn:
        # Dynamic segment already carries ANSI; affix wraps outside, no extra color.
        return wrapped
    if seg.get("autoColor") and seg["type"] in VALUE_COLORABLE:
        return auto_color(seg["type"], inp, wrapped)
    rgb = seg.get("rgb")
    if rgb:
        return fg(rgb, wrapped)
    return wrapped


def strip_ansi(s):
    return re.sub(r"\\x1b\\[[0-9;]*m", "", s)


def normalize_task(task, top, now):
    """Mirror the jq task-normalization in the subagent generator.

    jq re-derives each task from the immutable \`$top\` JSON, so a per-task
    mutation (token synthesis) must never leak into the shared top object. We
    deep-copy the fallback context_window before mutating it.
    """
    import copy
    t = dict(task)
    m = t.get("model") if t.get("model") is not None else top.get("model")
    cw = t.get("context_window") if t.get("context_window") is not None else top.get("context_window")
    t["model"] = m
    t["context_window"] = cw
    tc = t.get("tokenCount") or 0
    if cw is None and tc > 0:
        t["context_window"] = {"total_input_tokens": t["tokenCount"], "total_output_tokens": 0}
    elif cw is not None and (get(t, "context_window", "total_input_tokens", default=0) == 0) and tc > 0:
        t["context_window"] = copy.deepcopy(cw)
        t["context_window"]["total_input_tokens"] = t["tokenCount"]
    st_raw = t.get("startTime")
    if st_raw is not None:
        st = None
        if isinstance(st_raw, (int, float)) and not isinstance(st_raw, bool):
            st = st_raw / 1000 if st_raw > 10000000000 else st_raw
        elif isinstance(st_raw, str):
            try:
                import datetime
                st = datetime.datetime.fromisoformat(st_raw.replace("Z", "+00:00")).timestamp()
            except Exception:
                st = None
        if st is not None and (now - st) > 0:
            cost = dict(t.get("cost") or {})
            cost["total_duration_ms"] = (now - st) * 1000
            t["cost"] = cost
    name = get(t, "agent", "name") or t.get("label") or t.get("name") or jts(t.get("id"))
    agent = dict(t.get("agent") or {})
    agent["name"] = name
    t["agent"] = agent
    t["session_name"] = t.get("session_name") or t.get("label") or t.get("name") or jts(t.get("id"))
    return t


def render_subagent(payload, segs, now):
    """Render subagent JSONL: one {"id","content"} line per task."""
    top = {"model": payload.get("model"), "context_window": payload.get("context_window")}
    lines = []
    for task in (payload.get("tasks") or []):
        if not isinstance(task, dict) or task.get("id") is None:
            continue
        tid = jts(task.get("id"))
        if tid == "":
            continue
        inp = normalize_task(task, top, now)
        content = ""
        for seg in segs:
            content += render_segment(seg, inp, None)
        lines.append(json.dumps({"id": tid, "content": content}, ensure_ascii=False, separators=(",", ":")))
    return lines


def render(payload, rows, gi):
    """Render the main statusline rows → list of output lines."""
    cols = int(get(payload, "terminal", "width", default=0) or 0)
    out_lines = []
    for row in rows:
        align = row["align"]
        line = ""
        for seg in row["segs"]:
            line += render_segment(seg, payload, gi)
        if align in ("center", "right"):
            vis = strip_ansi(line)
            w = len(vis)
            if cols > 0 and cols > w:
                pad = (cols - w) // 2 if align == "center" else (cols - w)
                line = (" " * pad) + line
        out_lines.append(line)
    return out_lines
`;
