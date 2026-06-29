#!/usr/bin/env python3
# trellisx 强制收尾 — 一键 commit → merge → archive → 销 worktree。
# 两种触发 (同一脚本):
#   ① after_finish hook 自动调用 (apply 注入 config.yaml): `task.py finish` 触发,
#      自动收尾, 不靠 AI 记得跑。此时 finish 已清 active 指针, 用 $TASK_JSON_PATH 取 tid。
#   ② AI/人手动调用 (CLI, --task / 取 task.py current): 兜底, 幂等可重入。
# finish 与 worktree 删除为必须。
# 一对多: 1 task : 1 workflow : N worktree —— 一个 task exec 编排成 1 个 Claude Code Workflow,
# workflow 内 agent 各 worktree 隔离 (默认 1; 少数冲突型并行 subtask 各开子 worktree → N)。
# 收尾时经 task↔worktree 映射 (trellisx-taskmd.py map-list) 查出该 task 名下**全部**
# worktree, 子 subtask 分支先合、task 主分支后合, 再走 archive, 杜绝漏合丢提交。
# 单 worktree (绝大多数) 行为完全不变。
#
# 边界 (关键, 对齐 design C4 "收尾两层"): 收尾分两层, 责任不同。
#   ① git 层 (本脚本, 确定性): commit → merge --no-ff → 销 worktree → archive, 都是 git 操作。
#   ⓪ AI 层 (脚本做不到, 必须 AI 主动): 关闭本 task 名下悬挂的 Claude Code Workflow / 后台 Task
#      是 AI 行为 (TaskStop), 脚本做不到、也不假装能做。
# 顺序: 先 AI 层清悬挂 (⓪ TaskList 查 → 逐个 TaskStop 关) → 再 git 层 finish (① 本脚本) ——
# worktree 仍有进程在写时销毁 = 流程错误。本脚本仅在摘要末尾输出一行提醒, 不触碰 workflow/task 生命周期。
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


def taskmd_script_for(taskpy):
    # trellisx-taskmd.py 与 task.py 同处 .trellis/scripts/ (注入后); 不存在则 None。
    cand = os.path.join(os.path.dirname(os.path.abspath(taskpy)), "trellisx-taskmd.py")
    return cand if os.path.isfile(cand) else None


def collect_worktrees(troot, taskpy, tid, main_wt, main_br):
    """汇总该 task 名下**全部**待合并 worktree → [(wt路径, 分支名), ...]。

    经 task↔worktree 映射 (trellisx-taskmd.py map-list, cwd=troot 以命中 find_trellis_root)
    查出 tid 对应的全部 worktree; 每个取其实际 HEAD 分支 (子 worktree 分支名由创建者定,
    靠映射+实际分支而非猜命名)。**合并顺序: 子 subtask worktree 先, task 主 worktree 后** ——
    保证最后落主分支的是 task 主分支, 主分支提交历史以主分支收口。
    主 worktree (映射中可能含/不含) 与映射结果按 realpath 去重。目录不存在的 worktree 跳过 (已销/幂等)。
    """
    def head_branch(wt):
        r = run(["git", "-C", wt, "rev-parse", "--abbrev-ref", "HEAD"])
        return r.stdout.strip() if r.returncode == 0 else ""

    mains_real = os.path.realpath(main_wt)
    subs = []  # 子 worktree (非主)
    taskmd = taskmd_script_for(taskpy)
    if taskmd:
        r = run(["python3", taskmd, "map-list"], cwd=troot)
        if r.returncode == 0:
            for wt_disp in trellisx_wt.parse_map_list(r.stdout, tid):
                if os.path.realpath(wt_disp) == mains_real:
                    continue  # 主 worktree 单独在末尾处理
                if not os.path.isdir(wt_disp):
                    continue  # 已销毁 → 跳过
                br = head_branch(wt_disp)
                if br and br != "HEAD":  # 跳过 detached HEAD
                    subs.append((wt_disp, br))

    merge_list = list(subs)  # 子分支先
    if os.path.isdir(main_wt):  # task 主 worktree 后 (绝大多数仅此一项)
        merge_list.append((main_wt, main_br))
    return merge_list


def merge_one(groot, wt, br, msg, tid, dry_run):
    """单个 worktree: commit (有改动) → merge --no-ff 回主分支。

    冲突 → abort + 报冲突文件 + die(非 0)。已合并 → no-op (幂等)。
    返回 True 表示已合并/已是祖先; 冲突时 die 不返回。
    """
    if dry_run:
        return True
    if os.path.isdir(wt):
        st = run(["git", "-C", wt, "status", "--porcelain"])
        if st.stdout.strip():
            run(["git", "-C", wt, "add", "-A"])
            c = run(["git", "-C", wt, "commit", "-m", msg])
            if c.returncode != 0:
                die(f"worktree 提交失败 ({br}):\n{c.stderr or c.stdout}")
            print(f"trellisx-finish: ① 已提交 worktree 改动 ({br})")
        else:
            print(f"trellisx-finish: ① worktree 无改动, 跳过提交 ({br})")

    br_exists = run(["git", "-C", groot, "rev-parse", "--verify", br]).returncode == 0
    if not br_exists:
        print(f"trellisx-finish: ② 分支 {br} 不存在, 跳过合并")
        return True
    already = run(["git", "-C", groot, "merge-base", "--is-ancestor", br, "HEAD"]).returncode == 0
    if already:
        print(f"trellisx-finish: ② 分支 {br} 已在主分支, 跳过合并")
        return True
    m = run(["git", "-C", groot, "merge", "--no-ff", br, "-m", f"Merge: {tid} ({br})"])
    if m.returncode != 0:
        confl = run(["git", "-C", groot, "diff", "--name-only", "--diff-filter=U"])
        run(["git", "-C", groot, "merge", "--abort"])
        die("② 合并冲突, 已 abort。冲突分支: " + br + "\n冲突文件:\n  "
            + "\n  ".join(confl.stdout.strip().splitlines())
            + "\n→ 转手动解决, 禁强解。")
    print(f"trellisx-finish: ② 已合并 {br} → 主分支")
    return True


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

    msg = args.message or f"chore(task): {tid} 收尾提交"

    # task↔worktree 一对多: 经映射汇总该 task 名下全部待合并 worktree (子分支先, 主分支后)。
    # 绝大多数场景仅 task 主 worktree 一项, 行为与旧版一致。
    merge_list = collect_worktrees(troot, taskpy, tid, wt, br)

    if args.dry_run:
        print("trellisx-finish DRY-RUN 计划:")
        if merge_list:
            print(f"  待合并 worktree/分支 (顺序: 子 subtask 先, task 主分支后), 共 {len(merge_list)} 个:")
            for i, (mwt, mbr) in enumerate(merge_list, 1):
                tag = "主" if os.path.realpath(mwt) == os.path.realpath(wt) else "子"
                print(f"    {i}. [{tag}] {mwt}  分支={mbr}")
            print(f"  ① 各 worktree 有改动 → git add -A + commit -m '{msg}'")
            print(f"  ② 依次 git -C {groot} merge --no-ff <分支>")
        else:
            print("  无 worktree (inline 模式?), 跳过 ①②")
        print(f"  ③ python3 {taskpy} archive {tid}  (hook 销毁已合并 worktree)")
        return

    # 2+3. 逐个 worktree: commit (有改动) → merge --no-ff 回主分支 (任一冲突 → die, 不进 archive)
    if merge_list:
        for mwt, mbr in merge_list:
            merge_one(groot, mwt, mbr, msg, tid, args.dry_run)
    else:
        print("trellisx-finish: 无 worktree (inline 模式?), 跳过 ①②")

    # 4. archive (触发 after_archive hook → 销毁已合并 worktree + 删分支)
    a = run(["python3", taskpy, "archive", tid], timeout=60)
    if a.returncode != 0:
        die(f"③ archive 失败:\n{a.stderr or a.stdout}")
    print(f"trellisx-finish: ③ 已归档 {tid}")
    print(a.stdout.strip())

    # 5. 收尾校验: 全部已合并 worktree 应已销毁 (after_archive hook 仅销主 worktree;
    #    子 worktree 由其创建者/isolation 机制管理, 此处仅提示残留供人工核对)。
    for mwt, _mbr in merge_list:
        if os.path.isdir(mwt):
            print(f"trellisx-finish: ⚠️ worktree {mwt} 仍存在 "
                  f"(可能有未合并改动被 hook 保留, 或子 worktree 待其创建者清理), 请人工核对。",
                  file=sys.stderr)
    print(f"trellisx-finish: ✓ 收尾完成 — {tid} 已提交/合并/归档, worktree 已清理")
    print("trellisx-finish: reminder: 本脚本只销 worktree (git); 关闭 Workflow/Task 是 AI 层 "
          "(TaskStop), 脚本做不到 —— AI 须自查 TaskList 无悬挂 Workflow/后台 agent 任务。")


if __name__ == "__main__":
    main()
