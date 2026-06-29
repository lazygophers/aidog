#!/usr/bin/env python3
# trellisx task.md 看板 — 唯一读写入口 (AI 与 hook 都经此脚本, 不直接编辑 task.md)
#
# 用法:
#   trellisx-taskmd.py sync <create|start|archive>   # hook 用: 读 $TASK_JSON_PATH 同步确定性列
#   trellisx-taskmd.py update <tid> [--status S] [--worktree W]
#   trellisx-taskmd.py show [tid]                     # 打印看板 (或某任务行)
#   trellisx-taskmd.py cleanup [--days 7]             # 清理超 N 天的已完成行
#   trellisx-taskmd.py map-add <wt> <tid> [创建源]    # upsert worktree↔task 映射 (一对多)
#   trellisx-taskmd.py map-remove <wt>                # 删一条映射
#   trellisx-taskmd.py map-get <wt>                   # 命中→stdout tid 退0; 否则退1
#   trellisx-taskmd.py map-list                       # 打印全部映射
#   trellisx-taskmd.py lint                           # 合规退0; 否则退1+stderr
#   trellisx-taskmd.py fix                            # 机械修复 (列对齐/状态归一/去重)
#
# 列分工 (5 列): hook(sync) 维护 ID/名称/描述/状态; AI(update) 细化状态 (阶段) + worktree。
# 状态列承载生命周期阶段: 规划中/实施中/检查中/收尾/已完成/已归档。
# 冲突规则: hook sync 写基础态 (规划中/实施中/已完成/已归档); 若 AI 已写细分 (实施中/检查中/收尾),
#           hook sync 不覆 AI 细分 (如 AI 写"检查中", hook 不会强覆为"实施中")。
import json, os, re, sys
from datetime import date

# 状态列取值 (合并原"状态"+"阶段"): 基础态由 hook sync 写, 细分态由 AI update 写
STATUS_BASE = {"planning": "规划中", "in_progress": "实施中", "completed": "已完成", "archived": "已归档"}
# AI 细分态 (hook sync 不覆这些)
STATUS_REFINED = {"实施中", "检查中", "收尾"}
# 全部合法状态值 (lint 用)
STATUS_ALL = set(STATUS_BASE.values()) | STATUS_REFINED
HEADER = (
    "# Trellis 任务看板\n\n"
    "> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。\n\n"
    "| ID | 名称 | 描述 | 状态 | worktree |\n"
    "| --- | --- | --- | --- | --- |\n"
)

# 独立的 worktree ↔ task 映射区 (单表设计的明确例外: 主表一行一 task, 但 worktree
# 可能由 subagent isolation / 手动 git worktree add 建, 无对应 task 行; 此区显式登记
# 每个活跃 worktree 映射到哪个 task, 无映射的由 WorktreeCreate hook 提醒补登)。
MAP_MARK = "## Worktree ↔ Task 映射"
MAP_HEADER = (
    "\n" + MAP_MARK + "\n\n"
    "> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);\n"
    "> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。\n\n"
    "| worktree | task | 创建源 |\n"
    "| --- | --- | --- |\n"
)


def trellis_root_from(path):
    cur = os.path.dirname(os.path.abspath(path))
    while cur != os.path.dirname(cur):
        if os.path.basename(cur) == ".trellis":
            return os.path.dirname(cur)
        cur = os.path.dirname(cur)
    return None


def find_trellis_root():
    cur = os.path.abspath(os.getcwd())
    while cur != os.path.dirname(cur):
        if os.path.isdir(os.path.join(cur, ".trellis")):
            return cur
        cur = os.path.dirname(cur)
    return None


def taskmd_path(troot):
    return os.path.join(troot, ".trellis", "task.md")


def load_md(troot):
    p = taskmd_path(troot)
    md = open(p, encoding="utf-8").read() if os.path.exists(p) else HEADER
    if "| ID | 名称 |" not in md:
        md = HEADER + md
    return md


def save_md(troot, md):
    open(taskmd_path(troot), "w", encoding="utf-8").write(md)


def row_cells(line):
    return [c.strip() for c in line.strip().strip("|").split("|")]


def find_row(md, tid):
    m = re.search(rf"(?m)^\| {re.escape(tid)} \|.*$", md)
    return m


def write_row(md, tid, cells):
    row = f"| {tid} | " + " | ".join(cells) + " |"
    m = find_row(md, tid)
    return re.sub(rf"(?m)^\| {re.escape(tid)} \|.*$", row, md) if m else (md.rstrip() + "\n" + row + "\n")


def completed_at(troot, tid):
    tj = os.path.join(troot, ".trellis", "tasks", tid, "task.json")
    if os.path.isfile(tj):
        try:
            return json.load(open(tj, encoding="utf-8")).get("completedAt")
        except Exception:
            return None
    return None


def cleanup(md, troot, days):
    today = date.today()
    out = []
    for ln in md.splitlines():
        if ln.startswith("| ") and not ln.startswith("| ID |") and not ln.startswith("| --- |"):
            c = row_cells(ln)
            if len(c) >= 4 and c[3] == "已完成":
                ca = completed_at(troot, c[0])
                if ca:
                    try:
                        y, mo, d = map(int, ca[:10].split("-"))
                        if (today - date(y, mo, d)).days > days:
                            continue  # 移除
                    except Exception:
                        pass
        out.append(ln)
    return "\n".join(out) + "\n"


# ---- worktree ↔ task 映射区 ----
def _norm_wt(p):
    # realpath 解 symlink (macOS /var→/private/var; git worktree 与 hook 传路径可能一侧已解析)
    return os.path.realpath(os.path.expanduser((p or "").strip()))


def ensure_map_section(md):
    """task.md 无映射区 → 追加空映射区。"""
    return md if MAP_MARK in md else (md.rstrip() + "\n" + MAP_HEADER)


def map_rows(md):
    """解析映射区行 → [(worktree显示, tid, 备注, 原始行)]。"""
    if MAP_MARK not in md:
        return []
    seg = md.split(MAP_MARK, 1)[1]
    out = []
    for ln in seg.splitlines():
        if (ln.startswith("| ") and not ln.startswith("| worktree |")
                and not ln.startswith("| --- |")):
            c = row_cells(ln)
            if len(c) >= 2 and c[0]:
                out.append((c[0], c[1], c[2] if len(c) > 2 else "", ln))
    return out


def map_find(md, wt):
    """按 worktree 路径 (规范化匹配) 找映射行 → (显示, tid, 备注, 原始行) 或 None。"""
    n = _norm_wt(wt)
    for disp, tid, note, ln in map_rows(md):
        if _norm_wt(disp) == n:
            return (disp, tid, note, ln)
    return None


def map_remove_by_tid(md, tid):
    """移除映射区中 tid 列 == 给定 tid 的所有行 (archive 时清)。"""
    in_map, keep = False, []
    for ln in md.splitlines():
        if MAP_MARK in ln:
            in_map = True
        if (in_map and ln.startswith("| ") and not ln.startswith("| worktree |")
                and not ln.startswith("| --- |")):
            c = row_cells(ln)
            if len(c) >= 2 and c[1] == tid:
                continue  # 丢弃该 task 的映射行
        keep.append(ln)
    return "\n".join(keep) + "\n"


# ---- 命令分发 ----
def cmd_sync(action):
    tj = os.environ.get("TASK_JSON_PATH", "")
    if not tj or not os.path.isfile(tj):
        sys.exit(0)
    troot = trellis_root_from(tj)
    if not troot:
        sys.exit(0)
    try:
        meta = json.load(open(tj, encoding="utf-8"))
    except Exception:
        sys.exit(0)
    tid = meta.get("id") or os.path.basename(os.path.dirname(tj))
    title = (meta.get("title") or tid).replace("|", "/")
    desc = ((meta.get("description") or "").replace("|", "/").strip()) or "—"
    status = meta.get("status") or "planning"

    md = load_md(troot)
    m = find_row(md, tid)
    # 保留 AI 列 (worktree); 状态列按合并规则处理
    if m:
        c = row_cells(m.group(0))
        # 兼容旧 7 列数据 (迁移期): 取前 3 列 + 状态 + worktree (旧 worktree 在 c[6])
        cur_status = c[3] if len(c) > 3 and c[3] else STATUS_BASE.get(status, "规划中")
        # 旧 7 列 → 新 5 列: worktree 原在 c[6], 现在最后一列
        if len(c) >= 7:
            wt = c[6] if c[6] else "—"
        else:
            wt = c[4] if len(c) > 4 and c[4] else "—"
    else:
        cur_status, wt = STATUS_BASE.get(status, "规划中"), "—"

    # 状态合并规则: hook sync 写基础态; 若 AI 已写细分 (实施中/检查中/收尾), 不覆
    base_status = STATUS_BASE.get(status, status)
    if action == "create":
        new_status = "规划中"
    elif action == "archive":
        new_status = "已完成"
    else:  # start / 一般 sync
        # 不覆 AI 细分: 若当前是细分态且 task 仍 in_progress, 保留 AI 写的细分
        if cur_status in STATUS_REFINED and status == "in_progress":
            new_status = cur_status
        else:
            new_status = base_status

    md = write_row(md, tid, [title, desc, new_status, wt])
    if action == "archive":
        md = cleanup(md, troot, 7)
        md = map_remove_by_tid(md, tid)  # 归档顺带清该 task 的 worktree 映射
    save_md(troot, md)
    print(f"trellisx: task.md 看板已同步 ({tid} → {new_status})", file=sys.stderr)


def cmd_update(argv):
    if not argv:
        print("用法: update <tid> [--status S] [--worktree W]", file=sys.stderr)
        sys.exit(1)
    tid = argv[0]
    opts = {}
    i = 1
    while i < len(argv) - 1:
        if argv[i] in ("--status", "--worktree"):
            opts[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            i += 1
    troot = find_trellis_root()
    if not troot:
        print("trellisx: 未找到 .trellis", file=sys.stderr)
        sys.exit(1)
    md = load_md(troot)
    m = find_row(md, tid)
    if not m:
        print(f"trellisx: task.md 无 {tid} 行 (先 sync create)", file=sys.stderr)
        sys.exit(1)
    c = row_cells(m.group(0))      # [id,名称,描述,状态,worktree]
    # 兼容旧 7 列 → 取前 3 列 + 状态 + worktree(旧 c[6])
    if len(c) >= 7:
        title, desc, status, wt = c[1], c[2], c[3], c[6]
    else:
        while len(c) < 5:
            c.append("—")
        title, desc, status, wt = c[1], c[2], c[3], c[4]
    if "status" in opts: status = opts["status"]
    if "worktree" in opts: wt = opts["worktree"]
    md = write_row(md, tid, [title, desc, status, wt])
    save_md(troot, md)
    print(f"trellisx: {tid} 已更新 {opts}", file=sys.stderr)


def cmd_show(argv):
    troot = find_trellis_root()
    if not troot:
        print("trellisx: 未找到 .trellis", file=sys.stderr)
        sys.exit(1)
    md = load_md(troot)
    if argv:
        m = find_row(md, argv[0])
        print(m.group(0) if m else f"(无 {argv[0]})")
    else:
        print(md)


def cmd_cleanup(argv):
    days = 7
    if "--days" in argv:
        try:
            days = int(argv[argv.index("--days") + 1])
        except Exception:
            pass
    troot = find_trellis_root()
    if not troot:
        sys.exit(1)
    md = cleanup(load_md(troot), troot, days)
    save_md(troot, md)
    print(f"trellisx: 已清理超 {days} 天的已完成行", file=sys.stderr)


def cmd_map_add(argv):
    """map-add <worktree> <tid> [创建源] — 按 worktree 规范化 abspath upsert 一条映射。
    同 tid 可对多 worktree (各占一行, 一对多); 创建源默认 -。"""
    if len(argv) < 2:
        print("用法: map-add <worktree> <tid> [创建源]", file=sys.stderr)
        sys.exit(1)
    wt, tid = _norm_wt(argv[0]), argv[1]
    source = (" ".join(argv[2:]).replace("|", "/").strip()) or "-"
    troot = find_trellis_root()
    if not troot:
        print("trellisx: 未找到 .trellis", file=sys.stderr)
        sys.exit(1)
    md = ensure_map_section(load_md(troot))
    row = f"| {wt} | {tid} | {source} |"
    found = map_find(md, wt)
    md = md.replace(found[3], row) if found else (md.rstrip() + "\n" + row + "\n")
    save_md(troot, md)
    print(f"trellisx: worktree 映射登记 {wt} → {tid}", file=sys.stderr)


def cmd_map_remove(argv):
    """map-remove <worktree> — 删除一条映射 (worktree 销毁时调)。"""
    if not argv:
        print("用法: map-remove <worktree>", file=sys.stderr)
        sys.exit(1)
    troot = find_trellis_root()
    if not troot:
        sys.exit(0)
    md = load_md(troot)
    found = map_find(md, argv[0])
    if found:
        md = "\n".join(l for l in md.splitlines() if l != found[3]) + "\n"
        save_md(troot, md)
        print(f"trellisx: 移除 worktree 映射 {argv[0]}", file=sys.stderr)


def cmd_map_get(argv):
    """map-get <worktree> — 命中打印 tid 到 stdout 退 0; 否则退 1。"""
    if not argv:
        sys.exit(2)
    troot = find_trellis_root()
    if not troot:
        sys.exit(1)  # 无 .trellis → 无映射
    found = map_find(load_md(troot), argv[0])
    if found:
        print(found[1])
        sys.exit(0)
    sys.exit(1)  # 无映射


def cmd_map_list(_argv):
    """map-list — 打印全部 worktree↔task 映射。"""
    troot = find_trellis_root()
    if not troot:
        print("trellisx: 未找到 .trellis", file=sys.stderr)
        sys.exit(1)
    rows = map_rows(load_md(troot))
    if not rows:
        print("(无 worktree 映射)")
    for disp, tid, src, _ in rows:
        print(f"{disp} → {tid}  ({src})")


def cmd_lint(_argv):
    """lint — 合规退 0; 否则退 1 + stderr 列问题。
    规则: 主表数据行 5 列 / 映射区数据行 3 列 / 状态 ∈ 合法集 /
          主表 ID 不重复。"""
    troot = find_trellis_root()
    if not troot:
        print("trellisx: 未找到 .trellis", file=sys.stderr)
        sys.exit(1)
    md = load_md(troot)
    errs = []
    seg_main, seg_map = md, ""
    if MAP_MARK in md:
        seg_main, seg_map = md.split(MAP_MARK, 1)
    # 主表数据行
    seen = set()
    for ln in seg_main.splitlines():
        if (ln.startswith("| ") and not ln.startswith("| ID |")
                and not ln.startswith("| --- |")):
            c = row_cells(ln)
            if len(c) != 5:
                errs.append(f"主表数据行列数 {len(c)}≠5: {ln.strip()}")
                continue
            if c[3] not in STATUS_ALL:
                errs.append(f"主表状态非法 '{c[3]}': {ln.strip()}")
            if c[0] in seen:
                errs.append(f"主表 ID 重复 '{c[0]}'")
            seen.add(c[0])
    # 映射区数据行
    for ln in seg_map.splitlines():
        if (ln.startswith("| ") and not ln.startswith("| worktree |")
                and not ln.startswith("| --- |")):
            c = row_cells(ln)
            if len(c) != 3:
                errs.append(f"映射区数据行列数 {len(c)}≠3: {ln.strip()}")
    if errs:
        for e in errs:
            print("trellisx lint: " + e, file=sys.stderr)
        sys.exit(1)
    sys.exit(0)


PARK_MARK = "## ⚠ 待人工修正 (无法自动归类的行)"
PARK_HEADER = (
    "\n" + PARK_MARK + "\n\n"
    "> 下列行 lint 不合规且无法机械归类 (列数异常且非主表/映射行形态), 已停泊于此防丢失;\n"
    "> 请人工核对后改回主表或映射区, 或删除。修正后本块应清空。\n\n"
)


def _is_main_data_row(c):
    """主表数据行: 5 列且第 4 列是合法状态 (中文或英文键)。"""
    if len(c) != 5:
        return False
    return c[3] in STATUS_ALL or c[3] in STATUS_BASE


def _looks_like_map_row(c):
    """映射数据行: 恰 3 列, 或首列像 worktree 路径 (含 / 或 ~ 或 .worktree)。"""
    if not c or not c[0]:
        return False
    if len(c) == 3:
        return True
    first = c[0]
    return ("/" in first) or first.startswith("~") or (".worktree" in first)


def _norm_main_status(c):
    """英文状态键归一为中文展示值 (planning→规划中 等); 旧"进行中"→"实施中"。"""
    if c[3] in STATUS_BASE:
        c[3] = STATUS_BASE[c[3]]
    elif c[3] == "进行中":
        c[3] = "实施中"
    return c


def _migrate_old_row(c):
    """旧 7 列行 (ID/名称/描述/状态/阶段/进度/worktree) → 新 5 列 (ID/名称/描述/状态/worktree)。
    状态合并: 旧基础态 (规划中/进行中→实施中/已完成) + 旧阶段 (规划/实施/检查/收尾) → 新状态。"""
    if len(c) < 7:
        return c  # 非旧格式, 原样返回
    old_status, old_phase = c[3], c[4]
    wt = c[6] if len(c) > 6 and c[6] else "—"
    # 旧基础态归一
    if old_status == "进行中":
        base = "实施中"
    elif old_status in STATUS_BASE.values():
        base = old_status
    elif old_status in STATUS_BASE:
        base = STATUS_BASE[old_status]
    else:
        base = old_status
    # 旧阶段细化 (若旧基础态非已完成/已归档, 阶段可细化)
    if old_phase == "检查" and base not in ("已完成", "已归档"):
        new_status = "检查中"
    elif old_phase == "收尾" and base not in ("已完成", "已归档"):
        new_status = "收尾"
    elif old_phase == "实施" and base not in ("已完成", "已归档"):
        new_status = "实施中"
    else:
        new_status = base
    return [c[0], c[1], c[2], new_status, wt]


def cmd_fix(_argv):
    """fix — 机械修复 task.md: 按行形态重新归类到正确表 (主表/映射区), 英文状态归一,
    主表 ID 去重 (保留首见)。无法归类的行停泊到「待人工修正」块, 不丢数据。
    仅当内容有变化才写盘 (幂等), 写前备份 task.md.bak。
    全部可修复 → 退 0; 有残留不可修复行 → 退 1 + stderr 列出。"""
    troot = find_trellis_root()
    if not troot:
        print("trellisx: 未找到 .trellis", file=sys.stderr)
        sys.exit(1)
    orig = load_md(troot)

    def is_data_line(ln):
        return (ln.startswith("| ")
                and not ln.startswith("| ID |")
                and not ln.startswith("| 名称 |")
                and not ln.startswith("| worktree |")
                and not ln.startswith("| --- |"))

    main_rows, map_seen_lines, map_rows_out, parked = [], set(), [], []
    seen_ids = set()
    for ln in orig.splitlines():
        if not is_data_line(ln):
            continue
        c = row_cells(ln)
        # 旧 7 列行先迁移到新 5 列
        if len(c) >= 7:
            c = _migrate_old_row(c)
        if _is_main_data_row(c):
            c = _norm_main_status(c)
            if c[0] in seen_ids:
                continue  # 去重: 保留首见 ID
            seen_ids.add(c[0])
            main_rows.append("| " + " | ".join(c) + " |")
        elif _looks_like_map_row(c):
            wt = c[0]
            tid = c[1] if len(c) > 1 else ""
            src = c[2] if len(c) > 2 else "-"   # >3 列: 取前 3, 多余丢弃
            row = f"| {wt} | {tid} | {src or '-'} |"
            if row not in map_seen_lines:        # 去重
                map_seen_lines.add(row)
                map_rows_out.append(row)
        else:
            s = ln.strip()
            if s not in parked:
                parked.append(s)

    rebuilt = HEADER + ("".join(r + "\n" for r in main_rows))
    rebuilt += MAP_HEADER + ("".join(r + "\n" for r in map_rows_out))
    if parked:
        rebuilt += PARK_HEADER + ("".join(r + "\n" for r in parked))

    if rebuilt != orig:
        try:
            open(taskmd_path(troot) + ".bak", "w", encoding="utf-8").write(orig)
        except Exception:
            pass
        save_md(troot, rebuilt)
        print("trellisx: task.md 已自动修正 (旧列迁移/错置行归位/状态归一/去重)", file=sys.stderr)
    else:
        print("trellisx: task.md 无需修正", file=sys.stderr)

    if parked:
        print(f"trellisx fix: {len(parked)} 行无法机械归类, 已停泊「待人工修正」块, 需人工核对:",
              file=sys.stderr)
        for s in parked:
            print("  - " + s, file=sys.stderr)
        sys.exit(1)
    sys.exit(0)


def main():
    if len(sys.argv) < 2:
        print("用法: trellisx-taskmd.py "
              "<sync|update|show|cleanup|map-add|map-remove|map-get|map-list|lint|fix> ...",
              file=sys.stderr)
        sys.exit(1)
    cmd = sys.argv[1]
    if cmd == "sync":
        cmd_sync(sys.argv[2] if len(sys.argv) > 2 else "")
    elif cmd == "update":
        cmd_update(sys.argv[2:])
    elif cmd == "show":
        cmd_show(sys.argv[2:])
    elif cmd == "cleanup":
        cmd_cleanup(sys.argv[2:])
    elif cmd == "map-add":
        cmd_map_add(sys.argv[2:])
    elif cmd == "map-remove":
        cmd_map_remove(sys.argv[2:])
    elif cmd == "map-get":
        cmd_map_get(sys.argv[2:])
    elif cmd == "map-list":
        cmd_map_list(sys.argv[2:])
    elif cmd == "lint":
        cmd_lint(sys.argv[2:])
    elif cmd == "fix":
        cmd_fix(sys.argv[2:])
    else:
        print(f"trellisx: 未知命令 {cmd}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
