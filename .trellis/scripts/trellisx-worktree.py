#!/usr/bin/env python3
# trellisx worktree lifecycle — 由 .trellis/config.yaml hooks 调用
# 用法: trellisx-worktree.py start|archive   (trellis 传 TASK_JSON_PATH env)
import json, os, subprocess, sys

action = sys.argv[1] if len(sys.argv) > 1 else ""
tj = os.environ.get("TASK_JSON_PATH", "")
if not tj or not os.path.isfile(tj):
    sys.exit(0)


# task.json 形如 <troot>/.trellis/tasks/<tid>/task.json → 上溯定位 .trellis 父
def trellis_root(p):
    cur = os.path.dirname(os.path.abspath(p))
    while cur != os.path.dirname(cur):
        if os.path.basename(cur) == ".trellis":
            return os.path.dirname(cur)
        cur = os.path.dirname(cur)
    return None


troot = trellis_root(tj)
if not troot:
    sys.exit(0)

tid = os.path.basename(os.path.dirname(tj))
try:
    meta = json.load(open(tj, encoding="utf-8"))
except Exception:
    sys.exit(0)
pkg = (meta.get("package") or meta.get("scope") or "").strip()


def git_top(d):  # d 所在 git 仓库根, 非 git → None
    r = subprocess.run(["git", "-C", d, "rev-parse", "--show-toplevel"],
                       capture_output=True, text=True, timeout=10)
    return r.stdout.strip() if r.returncode == 0 and r.stdout.strip() else None


def resolve_repo():  # → (git根, service相对路径) | (None, None)
    g = git_top(troot)
    if g:                                    # 布局 1/2: .trellis 在 git 内
        return g, os.path.relpath(troot, g)
    if pkg:                                  # 布局 3: 子仓在 troot/<pkg>
        sub = os.path.join(troot, pkg)
        g = git_top(sub)
        if g:
            return g, "."
    return None, None


groot, service = resolve_repo()
if not groot:
    print(f"trellisx: 未能为 task {tid} 定位 git 仓库 (多子仓布局需先 task.py set-scope <子仓>)。worktree 跳过。", file=sys.stderr)
    sys.exit(0)

name = f"{pkg}-{tid}" if pkg else (tid if service == "." else f"{service.replace(os.sep, '-')}-{tid}")
wt = os.path.join(groot, ".worktrees", name)
br = f"trellisx-{name}"

if action == "start":
    if not os.path.isdir(wt):
        subprocess.run(["git", "-C", groot, "worktree", "add", wt, "-b", br],
                       capture_output=True, timeout=15)
        if service not in (".", None) and not pkg:   # 微服务 → sparse 只检子目录
            subprocess.run(["git", "-C", wt, "sparse-checkout", "set", service],
                           capture_output=True, timeout=10)
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
                print(f"trellisx: worktree 已销毁 {wt} (分支已合并回主分支)", file=sys.stderr)
