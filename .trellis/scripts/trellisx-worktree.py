#!/usr/bin/env python3
# trellisx worktree lifecycle — 由 .trellis/config.yaml hooks 调用
# 用法: trellisx-worktree.py start|archive   (trellis 传 TASK_JSON_PATH env)
# 路径/分支/命名约定见公共模块 trellisx_wt.py (单一真值, 与 trellisx-finish.py 共用)。
import json, os, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import trellisx_wt  # noqa: E402

action = sys.argv[1] if len(sys.argv) > 1 else ""
tj = os.environ.get("TASK_JSON_PATH", "")
if not tj or not os.path.isfile(tj):
    sys.exit(0)

troot = trellisx_wt.trellis_root(tj)
if not troot:
    sys.exit(0)

tid = os.path.basename(os.path.dirname(tj))
try:
    meta = json.load(open(tj, encoding="utf-8"))
except Exception:
    sys.exit(0)
pkg = (meta.get("package") or meta.get("scope") or "").strip()

groot, service = trellisx_wt.resolve_repo(troot, pkg)
if not groot:
    print(f"trellisx: 未能为 task {tid} 定位 git 仓库 (多子仓布局需先 task.py set-scope <子仓>)。worktree 跳过。", file=sys.stderr)
    sys.exit(0)

name, wt, br = trellisx_wt.worktree_paths(groot, tid, pkg, service)

_taskmd = os.path.join(os.path.dirname(os.path.abspath(__file__)), "trellisx-taskmd.py")


def _map(*args):
    """调同目录 taskmd 维护 worktree↔task 映射 (cwd=troot 确保定位; 缺脚本/异常静默)。"""
    if os.path.isfile(_taskmd):
        try:
            subprocess.run(["python3", _taskmd, *args], cwd=troot,
                           capture_output=True, timeout=10)
        except Exception:
            pass


if action == "start":
    if not os.path.isdir(wt):
        subprocess.run(["git", "-C", groot, "worktree", "add", wt, "-b", br],
                       capture_output=True, timeout=15)
        if service not in (".", None) and not pkg:   # 微服务 → sparse 只检子目录
            subprocess.run(["git", "-C", wt, "sparse-checkout", "set", service],
                           capture_output=True, timeout=10)
    _map("map-add", wt, tid, "trellisx-start")  # 经 task 建, 已知 tid, 不被 hook 提醒
    print(f"trellisx: worktree → {wt} (源码改动写此 worktree 内)", file=sys.stderr)

elif action == "archive":
    if os.path.isdir(wt):
        st = subprocess.run(["git", "-C", wt, "status", "--porcelain"],
                            capture_output=True, text=True, timeout=10)
        if st.stdout.strip():                 # 工作树脏 → 保留, 不丢未提交改动
            print(f"trellisx: worktree {wt} 有未提交改动, 保留 (先提交/合并再归档)。", file=sys.stderr)
        else:
            # 工作树干净, 但分支 br 可能有未合并回主分支的提交 → 检查后再销毁, 防 branch -D 丢提交。
            # merge-base --is-ancestor br HEAD: br 全部提交可达自 groot HEAD (= 已合并) → 0; 否则非 0。
            merged = subprocess.run(["git", "-C", groot, "merge-base", "--is-ancestor", br, "HEAD"],
                                    capture_output=True, timeout=10)
            if merged.returncode != 0:        # 有提交未合并 → 保留 worktree+分支, 禁丢
                print(f"trellisx: 分支 {br} 有未合并回主分支的提交, 保留 worktree+分支 "
                      f"(先 `git -C {groot} merge --no-ff {br}` 再归档)。", file=sys.stderr)
            else:
                subprocess.run(["git", "-C", groot, "worktree", "remove", wt, "--force"],
                               capture_output=True, timeout=15)
                subprocess.run(["git", "-C", groot, "branch", "-d", br],   # -d 安全删, 已确认合并
                               capture_output=True, timeout=10)
                _map("map-remove", wt)  # 销毁后清映射
                print(f"trellisx: worktree 已销毁 {wt} (分支已合并回主分支)", file=sys.stderr)
