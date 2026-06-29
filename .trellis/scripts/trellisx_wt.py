#!/usr/bin/env python3
"""trellisx worktree 路径/分支/命名 — 单一真值模块。

`trellisx-worktree.py` (生命周期 hook, 建/销) 与 `trellisx-finish.py` (强制收尾)
共用本模块, 杜绝两边各写一份导致路径/分支/命名漂移。

约定 (不要在别处重写, 改约定只改这里):
- worktree 路径 = `<git根>/.worktrees/<name>`
- 分支         = `trellisx-<name>`
- name         = pkg-tid / service-tid / tid  (见 worktree_name)
"""
import os
import subprocess


def _run(cmd, timeout=15):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def trellis_root(p):
    """task.json 形如 <troot>/.trellis/tasks/<tid>/task.json → 上溯定位 .trellis 父 (troot)。"""
    cur = os.path.dirname(os.path.abspath(p))
    while cur != os.path.dirname(cur):
        if os.path.basename(cur) == ".trellis":
            return os.path.dirname(cur)
        cur = os.path.dirname(cur)
    return None


def git_top(d):
    """d 所在 git 仓库的**主 worktree** 根 (非 linked worktree), 非 git → None。

    用 `--git-common-dir` 而非 `--show-toplevel`: 从 linked worktree 内调用时
    show-toplevel 会返回该 worktree, 而合并/销毁的基准必须是主仓。
    """
    r = _run(["git", "-C", d, "rev-parse", "--path-format=absolute", "--git-common-dir"])
    if r.returncode == 0 and r.stdout.strip():
        gd = r.stdout.strip().rstrip("/")
        if os.path.basename(gd) == ".git":
            return os.path.dirname(gd)
    r2 = _run(["git", "-C", d, "rev-parse", "--show-toplevel"])  # 老 git 兜底
    return r2.stdout.strip() if r2.returncode == 0 and r2.stdout.strip() else None


def resolve_repo(troot, pkg):
    """→ (git根 groot, service 相对路径) | (None, None)。3 布局自适应。"""
    g = git_top(troot)
    if g:                                    # 布局 1/2: .trellis 在 git 内
        return g, os.path.relpath(troot, g)
    if pkg:                                  # 布局 3: 子仓在 troot/<pkg>
        sub = os.path.join(troot, pkg)
        g = git_top(sub)
        if g:
            return g, "."
    return None, None


def worktree_name(tid, pkg="", service="."):
    """worktree / 分支命名: 有 pkg → pkg-tid; 微服务子目录 → service-tid; 否则 tid。"""
    if pkg:
        return f"{pkg}-{tid}"
    if service in (".", None):
        return tid
    return f"{service.replace(os.sep, '-')}-{tid}"


def worktree_paths(groot, tid, pkg="", service="."):
    """单一真值: → (name, worktree 绝对路径, 分支名)。"""
    name = worktree_name(tid, pkg, service)
    wt = os.path.join(groot, ".worktrees", name)
    br = f"trellisx-{name}"
    return name, wt, br


def parse_map_list(text, tid):
    """解析 `trellisx-taskmd.py map-list` stdout, 抽出映射到 `tid` 的全部 worktree 路径。

    每行格式 (见 trellisx-taskmd cmd_map_list): `<worktree路径> → <tid>  (<创建源>)`。
    返回去重 (按 realpath) 后保持出现顺序的 worktree 显示路径列表; 无匹配 → []。
    纯函数, 不触盘 (realpath 仅做 symlink 规范化用于去重比较)。
    """
    out, seen = [], set()
    for ln in (text or "").splitlines():
        ln = ln.strip()
        if " → " not in ln:
            continue
        left, right = ln.split(" → ", 1)
        wt_disp = left.strip()
        # right 形如 "<tid>  (<源>)" → tid 是首个空白前 token
        row_tid = right.strip().split()[0] if right.strip().split() else ""
        if row_tid != tid or not wt_disp:
            continue
        key = os.path.realpath(os.path.expanduser(wt_disp))
        if key in seen:
            continue
        seen.add(key)
        out.append(wt_disp)
    return out
