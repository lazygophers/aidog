#!/usr/bin/env python3
# trellisx 批量收尾 — 一次清理/归档/收尾**全部**已完成 task (completed ∪ merged)。
# 定位: 单 task 收尾用 trellisx-finish.py; 本脚本是其**批量编排层** —— 枚举 + 逐个委托,
#        不重写 git 逻辑 (merge/archive/销 worktree 全交 finish.py + worktree 安全判据)。
#
# 三需求 (用户原话):
#   ① 清理所有已完成的 worktree   → 已合并+干净的 worktree 销毁 (沿用 worktree.py 安全判据)
#   ② 归档所有已完成的任务        → 逐个 task.py archive (经 finish.py 全链, 先合并后归档)
#   ③ 清理所有已完成任务的 task.md → taskmd cleanup --days N 删看板"已完成"行
#
# 完成判定 = completed ∪ merged (并集):
#   - completed: .trellis/tasks/<tid>/task.json status == "completed" (未归档)
#   - merged   : worktree 干净 且 分支已 merge 回主分支 (merge-base --is-ancestor)
#   - 当前 active task **永不**纳入 (防销正在用的)。
#
# 用法: trellisx-cleanup.py [--apply] [--days N] [--message "<commit msg>"]
#   (无 --apply)  DRY-RUN: 只枚举+打印将做什么, 不落地 (默认, 安全)
#   --apply       执行: 逐个 finish (commit→merge→archive→销 worktree) + 孤儿 worktree 清扫 + 看板清理
#   --days N       看板"已完成"行清理阈值, 删除完成超 N 天的行 (默认 0 = 全删)
#   --message MSG  worktree 提交消息 (透传 finish.py)
#
# 退出码: 0 全部成功; 非 0 = 至少一个 task 收尾失败 (合并冲突等), 报告后停, 不静默吞错。
import argparse
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import trellisx_wt  # noqa: E402  worktree 路径/分支/命名单一真值


def run(cmd, cwd=None, timeout=30):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)


def find_trellis_scripts():
    """定位 .trellis/scripts/task.py (注入后本脚本与它同处该目录)。"""
    here = os.path.dirname(os.path.abspath(__file__))
    cur = here
    while cur != os.path.dirname(cur):
        cand = os.path.join(cur, ".trellis", "scripts", "task.py")
        if os.path.isfile(cand):
            return cand
        if os.path.basename(cur) == "scripts" and os.path.isfile(os.path.join(cur, "task.py")):
            return os.path.join(cur, "task.py")
        cur = os.path.dirname(cur)
    return None


def trellis_root_from(taskpy):
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(taskpy))))


def sibling(taskpy, name):
    cand = os.path.join(os.path.dirname(os.path.abspath(taskpy)), name)
    return cand if os.path.isfile(cand) else None


def active_tid(taskpy):
    """当前 active task id (无 → None)。批量收尾**永不**纳入它。"""
    r = run(["python3", taskpy, "current"])
    for ln in (r.stdout or "").splitlines():
        ln = ln.strip()
        if "tasks/" in ln and ".trellis" in ln:
            return os.path.basename(ln.rstrip("/"))
    return None


def scan_completed(troot, exclude):
    """扫 .trellis/tasks/<tid>/task.json status==completed (跳 archive/ 与 exclude)。"""
    tasks_dir = os.path.join(troot, ".trellis", "tasks")
    out = []
    if not os.path.isdir(tasks_dir):
        return out
    for tid in sorted(os.listdir(tasks_dir)):
        if tid == "archive" or tid in exclude:
            continue
        tj = os.path.join(tasks_dir, tid, "task.json")
        if not os.path.isfile(tj):
            continue
        try:
            if json.load(open(tj, encoding="utf-8")).get("status") == "completed":
                out.append(tid)
        except Exception:
            pass
    return out


def parse_all_maps(text):
    """解析 map-list 全部行 → [(worktree显示路径, tid), ...]。"""
    out = []
    for ln in (text or "").splitlines():
        ln = ln.strip()
        if " → " not in ln:
            continue
        left, right = ln.split(" → ", 1)
        wt = left.strip()
        toks = right.strip().split()
        if wt and toks:
            out.append((wt, toks[0]))
    return out


def wt_state(wt):
    """孤儿 worktree 安全判定 (镜像 trellisx-worktree.py archive):
    → ("gone"|"dirty"|"unmerged"|"merged", groot, br)。
    只有 "merged" 才可安全销毁 (干净 且 分支已合并回主分支)。"""
    if not os.path.isdir(wt):
        return "gone", None, None
    groot = trellisx_wt.git_top(wt)
    br = ""
    r = run(["git", "-C", wt, "rev-parse", "--abbrev-ref", "HEAD"])
    if r.returncode == 0:
        br = r.stdout.strip()
    st = run(["git", "-C", wt, "status", "--porcelain"])
    if st.stdout.strip():
        return "dirty", groot, br
    if not groot or not br or br == "HEAD":
        return "unmerged", groot, br  # 无法判定 → 保守保留
    merged = run(["git", "-C", groot, "merge-base", "--is-ancestor", br, "HEAD"])
    if merged.returncode != 0:
        return "unmerged", groot, br
    return "merged", groot, br


def destroy_worktree(groot, wt, br, taskmd, troot):
    """安全销毁已合并 worktree (前置: wt_state == merged)。"""
    run(["git", "-C", groot, "worktree", "remove", wt, "--force"], timeout=20)
    if br and br != "HEAD":
        run(["git", "-C", groot, "branch", "-d", br], timeout=15)
    if taskmd:
        run(["python3", taskmd, "map-remove", wt], cwd=troot, timeout=15)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true", help="执行 (缺省为 dry-run 只报告)")
    ap.add_argument("--days", type=int, default=0, help="看板已完成行清理阈值天数 (默认 0=全删)")
    ap.add_argument("--message", help="worktree 提交消息 (透传 finish.py)")
    args = ap.parse_args()

    taskpy = find_trellis_scripts()
    if not taskpy:
        print("trellisx-cleanup: 未找到 .trellis/scripts/task.py (非 trellis 项目?)", file=sys.stderr)
        sys.exit(1)
    troot = trellis_root_from(taskpy)
    finishpy = sibling(taskpy, "trellisx-finish.py")
    taskmd = sibling(taskpy, "trellisx-taskmd.py")

    active = active_tid(taskpy)
    exclude = {active} if active else set()

    # 枚举 ①completed 任务 ②map 中孤儿/已合并 worktree
    completed = scan_completed(troot, exclude)
    maps = []
    if taskmd:
        r = run(["python3", taskmd, "map-list"], cwd=troot)
        if r.returncode == 0:
            maps = parse_all_maps(r.stdout)
    completed_set = set(completed)
    orphans = []  # (wt, tid, state, groot, br) — 不属任何待收尾 completed 的 worktree
    for wt, tid in maps:
        if tid in completed_set:
            continue  # 属待收尾 task, 由 finish.py 处理
        if active and tid == active:
            continue  # active task 的 worktree, 跳过
        state, groot, br = wt_state(wt)
        orphans.append((wt, tid, state, groot, br))

    # ── 报告 (dry-run 与 apply 都先打印) ──
    print("trellisx-cleanup 计划:")
    if active:
        print(f"  · 跳过当前 active task: {active}")
    print(f"  ① 待收尾 completed 任务 ({len(completed)}): "
          + (", ".join(completed) if completed else "无"))
    merged_orphans = [o for o in orphans if o[2] == "merged"]
    kept_orphans = [o for o in orphans if o[2] in ("dirty", "unmerged")]
    gone_orphans = [o for o in orphans if o[2] == "gone"]
    print(f"  ② 待销毁孤儿 worktree (已合并+干净, {len(merged_orphans)}):")
    for wt, tid, _s, _g, br in merged_orphans:
        print(f"      - {wt}  (tid={tid}, 分支={br})")
    if kept_orphans:
        print(f"  · 保留孤儿 worktree (脏/未合并, 不丢提交, {len(kept_orphans)}):")
        for wt, tid, s, _g, br in kept_orphans:
            print(f"      - {wt}  (tid={tid}, 状态={s})")
    if gone_orphans:
        print(f"  · 映射残留 (worktree 已不存在, 清映射, {len(gone_orphans)})")
    print(f"  ③ 看板清理: taskmd cleanup --days {args.days} "
          f"(删除完成超 {args.days} 天的'已完成'行)")

    if not args.apply:
        print("\ntrellisx-cleanup: DRY-RUN (未落地)。确认后加 --apply 执行。")
        return

    # ── 执行 ──
    print("\ntrellisx-cleanup: 开始执行 (--apply)")
    failures = []

    # ① 逐个 task 全链收尾 (finish.py: commit→merge→archive→销 worktree; 逐个提交; 幂等)
    if not finishpy:
        print("trellisx-cleanup: 缺 trellisx-finish.py, 跳过任务收尾 (先 /trellisx-apply 复制脚本)",
              file=sys.stderr)
    for tid in completed:
        cmd = ["python3", finishpy, "--task", tid]
        if args.message:
            cmd += ["--message", args.message]
        r = run(cmd, timeout=120)
        sys.stdout.write(r.stdout)
        if r.returncode != 0:
            sys.stderr.write(r.stderr)
            failures.append(tid)
            print(f"trellisx-cleanup: ✗ {tid} 收尾失败 (见上, 多为合并冲突 → 转手动)", file=sys.stderr)
        else:
            print(f"trellisx-cleanup: ✓ {tid} 已收尾")

    # ② 孤儿 worktree 清扫 (只销已合并+干净的; 脏/未合并保留)
    for wt, tid, state, groot, br in orphans:
        if state == "merged" and groot:
            destroy_worktree(groot, wt, br, taskmd, troot)
            print(f"trellisx-cleanup: ✓ 已销毁孤儿 worktree {wt} (tid={tid}, 已合并)")
        elif state == "gone" and taskmd:
            run(["python3", taskmd, "map-remove", wt], cwd=troot, timeout=15)
            print(f"trellisx-cleanup: · 已清残留映射 {wt}")
        elif state in ("dirty", "unmerged"):
            print(f"trellisx-cleanup: ⚠ 保留 worktree {wt} (tid={tid}, {state}) — "
                  f"先提交/合并再清理, 禁丢提交", file=sys.stderr)

    # ③ 看板清理 (删已完成行) + 规范校验
    if taskmd:
        run(["python3", taskmd, "cleanup", "--days", str(args.days)], cwd=troot, timeout=15)
        print(f"trellisx-cleanup: ✓ 看板已清理 (--days {args.days})")
        lint = run(["python3", taskmd, "lint"], cwd=troot, timeout=15)
        if lint.returncode != 0:
            print(f"trellisx-cleanup: ⚠ 看板 lint 未通过:\n{lint.stderr}", file=sys.stderr)
        else:
            print("trellisx-cleanup: ✓ 看板格式合规")

    # 收尾摘要
    if failures:
        print(f"\ntrellisx-cleanup: ✗ {len(failures)} 个 task 收尾失败: {', '.join(failures)} "
              f"(转手动解决冲突后重跑)", file=sys.stderr)
        sys.exit(1)
    print(f"\ntrellisx-cleanup: ✓ 批量收尾完成 — {len(completed)} task 归档, "
          f"{len(merged_orphans)} 孤儿 worktree 销毁, 看板已清理")
    print("trellisx-cleanup: reminder: 本脚本只做 git/看板 (确定性); 关闭悬挂 Workflow/后台 Task "
          "是 AI 层 (TaskList 查 → TaskStop 关), 脚本做不到 —— AI 须自查。")


if __name__ == "__main__":
    main()
