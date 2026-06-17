#!/usr/bin/env python3
# trellisx 强制收尾 — 一键 commit → merge → archive → 销 worktree。
# 两种触发 (同一脚本):
#   ① after_finish hook 自动调用 (apply 注入 config.yaml): `task.py finish` 触发,
#      自动收尾, 不靠 AI 记得跑。此时 finish 已清 active 指针, 用 $TASK_JSON_PATH 取 tid。
#   ② AI/人手动调用 (CLI, --task / 取 task.py current): 兜底, 幂等可重入。
# finish 与 worktree 删除为必须。
#
# 用法: trellisx-finish.py [--task <tid>] [--message "<commit msg>"] [--dry-run]
#   --task     目标 task id (缺省: 先 $TASK_JSON_PATH 后 task.py current)
#   --message  worktree 提交消息 (缺省 "chore(task): <tid> 收尾提交")
#   --dry-run  只打印将执行的步骤, 不落地
#
# 退出码: 0 成功; 非 0 = 某步失败 (冲突 / 未合并 / archive 失败), 报告后停, 不静默继续。
import argparse
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import trellisx_wt  # noqa: E402  路径/分支/命名单一真值 (与 trellisx-worktree.py 共用)


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

    # 1. 定位 task: --task > $TASK_JSON_PATH (after_finish hook 模式) > task.py current (手动)
    #    finish 已清 active 指针 → hook 内 current 取不到, 必须靠 $TASK_JSON_PATH。
    tid = args.task
    if not tid:
        tjp_env = os.environ.get("TASK_JSON_PATH", "")
        if tjp_env and os.path.isfile(tjp_env):
            tid = os.path.basename(os.path.dirname(os.path.abspath(tjp_env)))
    if not tid:
        r = run(["python3", taskpy, "current"])
        line = (r.stdout or "").strip().splitlines()
        path = line[0].strip() if line else ""
        if not path or "tasks/" not in path:
            die("无 active task ($TASK_JSON_PATH 与 task.py current 均为空), 无可收尾对象")
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

    groot, service = trellisx_wt.resolve_repo(troot, pkg)
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

    name, wt, br = trellisx_wt.worktree_paths(groot, tid, pkg, service)
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
