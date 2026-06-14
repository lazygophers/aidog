#!/usr/bin/env python3
# trellisx 强制收尾 — 一键 commit → merge → archive → 销 worktree。
# 由 workflow.md finish 段强制调用 (apply 注入): check 通过后 AI 跑此脚本,
# 不再停在「提醒用户运行 /finish-work」。finish 与 worktree 删除为必须。
#
# 用法: trellisx-finish.py [--task <tid>] [--message "<commit msg>"] [--dry-run]
#   --task     目标 task id (缺省取 task.py current)
#   --message  worktree 提交消息 (缺省 "chore(task): <tid> 收尾提交")
#   --dry-run  只打印将执行的步骤, 不落地
#
# 退出码: 0 成功; 非 0 = 某步失败 (冲突 / 未合并 / archive 失败), 报告后停, 不静默继续。
import argparse
import json
import os
import subprocess
import sys


def run(cmd, cwd=None, timeout=30):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)


def die(msg, code=1):
    print(f"trellisx-finish: {msg}", file=sys.stderr)
    sys.exit(code)


def find_trellis_scripts():
    # 本脚本运行时定位 .trellis/scripts/task.py (注入后位于目标项目 .trellis/scripts/)
    here = os.path.dirname(os.path.abspath(__file__))
    cur = here
    while cur != os.path.dirname(cur):
        cand = os.path.join(cur, ".trellis", "scripts", "task.py")
        if os.path.isfile(cand):
            return cand
        # 本脚本自身在 .trellis/scripts/ 内时
        if os.path.basename(cur) == "scripts" and os.path.isfile(os.path.join(cur, "task.py")):
            return os.path.join(cur, "task.py")
        cur = os.path.dirname(cur)
    return None


def trellis_root_from(taskpy):
    # taskpy = <root>/.trellis/scripts/task.py → root
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(taskpy))))


def git_top(d):
    # 用 --git-common-dir 定位**主 worktree** 根 (而非 linked worktree):
    # 从 worktree 内运行时 --show-toplevel 会返回该 worktree, 合并目标须是主仓。
    r = run(["git", "-C", d, "rev-parse", "--path-format=absolute", "--git-common-dir"])
    if r.returncode == 0 and r.stdout.strip():
        gitdir = r.stdout.strip()
        if os.path.basename(gitdir.rstrip("/")) == ".git":
            return os.path.dirname(gitdir.rstrip("/"))
    # 兜底: 老 git 无 --path-format → show-toplevel
    r2 = run(["git", "-C", d, "rev-parse", "--show-toplevel"])
    return r2.stdout.strip() if r2.returncode == 0 and r2.stdout.strip() else None


def resolve_repo(troot, pkg):
    g = git_top(troot)
    if g:
        return g, os.path.relpath(troot, g)
    if pkg:
        sub = os.path.join(troot, pkg)
        g = git_top(sub)
        if g:
            return g, "."
    return None, None


def worktree_name(tid, pkg, service):
    if pkg:
        return f"{pkg}-{tid}"
    if service in (".", None):
        return tid
    return f"{service.replace(os.sep, '-')}-{tid}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--task")
    ap.add_argument("--message")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    taskpy = find_trellis_scripts()
    if not taskpy:
        die("未找到 .trellis/scripts/task.py (非 trellis 项目?)")
    troot = trellis_root_from(taskpy)

    # 1. 定位 active task
    tid = args.task
    if not tid:
        r = run(["python3", taskpy, "current"])
        line = (r.stdout or "").strip().splitlines()
        path = line[0].strip() if line else ""
        if not path or "tasks/" not in path:
            die("无 active task (task.py current 为空), 无可收尾对象")
        tid = os.path.basename(path.rstrip("/"))
    print(f"trellisx-finish: 收尾 task = {tid}")

    tj = os.path.join(troot, ".trellis", "tasks", tid, "task.json")
    pkg = ""
    if os.path.isfile(tj):
        try:
            meta = json.load(open(tj, encoding="utf-8"))
            pkg = (meta.get("package") or meta.get("scope") or "").strip()
        except Exception:
            pass

    groot, service = resolve_repo(troot, pkg)
    if not groot:
        die("未能定位 git 仓库")

    # 加固: groot 自带 .trellis (layout 1/2) → 以 groot 为 troot, service=".",
    # 避免误从 worktree 副本运行时 service 解析错位 (注入后脚本在主仓不触发此分支)。
    if os.path.isdir(os.path.join(groot, ".trellis")):
        troot = groot
        service = "."
        cand = os.path.join(groot, ".trellis", "scripts", "task.py")
        if os.path.isfile(cand):
            taskpy = cand
        tj = os.path.join(groot, ".trellis", "tasks", tid, "task.json")
        if os.path.isfile(tj):
            try:
                pkg = (json.load(open(tj, encoding="utf-8")).get("package")
                       or json.load(open(tj, encoding="utf-8")).get("scope") or "").strip()
            except Exception:
                pass

    # 对齐 .claude/hooks/trellisx-worktree.py (worktree 实际创建者):
    #   worktree = <troot>/.trellis/worktrees/<tid>, 分支 = trellisx-<tid>。
    # 旧版用 <groot>/.worktrees/<name> → 与 hook 路径不符, has_wt 恒 False,
    # 误判 inline 跳过提交+合并 → 改动滞留 worktree 丢失。
    name = tid
    wt = os.path.join(troot, ".trellis", "worktrees", tid)
    br = f"trellisx-{tid}"
    has_wt = os.path.isdir(wt)

    msg = args.message or f"chore(task): {tid} 收尾提交"

    if args.dry_run:
        print("trellisx-finish DRY-RUN 计划:")
        print(f"  worktree = {wt} (存在={has_wt})  分支={br}")
        print(f"  ① worktree 有改动 → git add -A + commit -m '{msg}'")
        print(f"  ② git -C {groot} merge --no-ff {br}")
        print(f"  ③ python3 {taskpy} archive {tid}  (hook 销毁已合并 worktree)")
        return

    # 2. worktree 内提交 (有改动才提交)
    if has_wt:
        st = run(["git", "-C", wt, "status", "--porcelain"])
        if st.stdout.strip():
            run(["git", "-C", wt, "add", "-A"])
            c = run(["git", "-C", wt, "commit", "-m", msg])
            if c.returncode != 0:
                die(f"worktree 提交失败:\n{c.stderr or c.stdout}")
            print(f"trellisx-finish: ① 已提交 worktree 改动 ({br})")
        else:
            print(f"trellisx-finish: ① worktree 无改动, 跳过提交")

        # 3. 合并 worktree 分支回主分支
        br_exists = run(["git", "-C", groot, "rev-parse", "--verify", br]).returncode == 0
        if br_exists:
            already = run(["git", "-C", groot, "merge-base", "--is-ancestor", br, "HEAD"]).returncode == 0
            if already:
                print(f"trellisx-finish: ② 分支 {br} 已在主分支, 跳过合并")
            else:
                m = run(["git", "-C", groot, "merge", "--no-ff", br,
                         "-m", f"Merge: {tid} ({br})"])
                if m.returncode != 0:
                    confl = run(["git", "-C", groot, "diff", "--name-only", "--diff-filter=U"])
                    run(["git", "-C", groot, "merge", "--abort"])
                    die("② 合并冲突, 已 abort。冲突文件:\n  "
                        + "\n  ".join(confl.stdout.strip().splitlines())
                        + "\n→ 转手动解决, 禁强解。")
                print(f"trellisx-finish: ② 已合并 {br} → 主分支")
    else:
        print("trellisx-finish: 无 worktree (inline 模式?), 跳过 ①②")

    # 4. archive (触发 after_archive hook → 销毁已合并 worktree + 删分支)
    a = run(["python3", taskpy, "archive", tid], timeout=60)
    if a.returncode != 0:
        die(f"③ archive 失败:\n{a.stderr or a.stdout}")
    print(f"trellisx-finish: ③ 已归档 {tid}")
    print(a.stdout.strip())

    # 5. 收尾校验: worktree 应已销毁
    if os.path.isdir(wt):
        print(f"trellisx-finish: ⚠️ worktree {wt} 仍存在 "
              f"(可能有未合并改动被 hook 保留), 请人工核对。", file=sys.stderr)
    print(f"trellisx-finish: ✓ 收尾完成 — {tid} 已提交/合并/归档, worktree 已清理")


if __name__ == "__main__":
    main()
