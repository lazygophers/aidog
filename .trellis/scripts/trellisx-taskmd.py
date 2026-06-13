#!/usr/bin/env python3
# trellisx task.md 看板 — 唯一读写入口 (AI 与 hook 都经此脚本, 不直接编辑 task.md)
#
# 用法:
#   trellisx-taskmd.py sync <create|start|archive>   # hook 用: 读 $TASK_JSON_PATH 同步确定性列
#   trellisx-taskmd.py update <tid> [--status S] [--phase P] [--progress N] [--worktree W]
#   trellisx-taskmd.py show [tid]                     # 打印看板 (或某任务行)
#   trellisx-taskmd.py cleanup [--days 7]             # 清理超 N 天的已完成行
#
# 列分工: hook(sync) 维护 ID/名称/描述/状态; AI(update) 维护 阶段/进度/worktree。互不覆盖。
import json, os, re, sys
from datetime import date

STATUS_CN = {"planning": "规划中", "in_progress": "进行中", "completed": "已完成"}
PHASE_DEFAULT = {"planning": "规划", "in_progress": "实施", "completed": "收尾"}
PROG_DEFAULT = {"planning": "0%", "in_progress": "—", "completed": "100%"}
HEADER = (
    "# Trellis 任务看板\n\n"
    "> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。\n\n"
    "| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |\n"
    "| --- | --- | --- | --- | --- | --- | --- |\n"
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
    status_cn = STATUS_CN.get(status, status)

    md = load_md(troot)
    m = find_row(md, tid)
    if m:  # 保留 AI 列 (阶段/进度/worktree)
        c = row_cells(m.group(0))
        phase = c[4] if len(c) > 4 and c[4] else PHASE_DEFAULT.get(status, "规划")
        prog = c[5] if len(c) > 5 and c[5] else PROG_DEFAULT.get(status, "—")
        wt = c[6] if len(c) > 6 and c[6] else "—"
    else:
        phase, prog, wt = PHASE_DEFAULT.get(status, "规划"), PROG_DEFAULT.get(status, "—"), "—"

    if action == "create":
        phase, prog = "规划", "0%"
    elif action == "archive":
        status_cn, phase, prog = "已完成", "收尾", "100%"

    md = write_row(md, tid, [title, desc, status_cn, phase, prog, wt])
    if action == "archive":
        md = cleanup(md, troot, 7)
    save_md(troot, md)
    print(f"trellisx: task.md 看板已同步 ({tid} → {status_cn})", file=sys.stderr)


def cmd_update(argv):
    if not argv:
        print("用法: update <tid> [--status S] [--phase P] [--progress N] [--worktree W]", file=sys.stderr)
        sys.exit(1)
    tid = argv[0]
    opts = {}
    i = 1
    while i < len(argv) - 1:
        if argv[i] in ("--status", "--phase", "--progress", "--worktree"):
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
    c = row_cells(m.group(0))      # [id,名称,描述,状态,阶段,进度,worktree]
    while len(c) < 7:
        c.append("—")
    if "status" in opts: c[3] = opts["status"]
    if "phase" in opts: c[4] = opts["phase"]
    if "progress" in opts: c[5] = opts["progress"]
    if "worktree" in opts: c[6] = opts["worktree"]
    md = write_row(md, tid, c[1:7])
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


def main():
    if len(sys.argv) < 2:
        print("用法: trellisx-taskmd.py <sync|update|show|cleanup> ...", file=sys.stderr)
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
    else:
        print(f"trellisx: 未知命令 {cmd}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
