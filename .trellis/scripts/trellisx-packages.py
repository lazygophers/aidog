#!/usr/bin/env python3
# trellisx packages 自动发现 — apply 时一次扫描, 把 monorepo 包写进 .trellis/config.yaml 的 packages:
# 触发: 仅 apply 一次 (非 hook); 用户后续仓结构变化需重跑。
# 安全: 仅当项目当前无实值 packages: (单仓) 才自动写; 已配置 → 只报告不覆盖 (尊重用户配置)。
#
# 用法:
#   trellisx-packages.py discover [--repo R]   只打印发现的包 (JSON), 不写盘 (plan-hook 用)
#   trellisx-packages.py apply    [--repo R]   写入 config.yaml packages: (write-hook 审批后用)
#
# 发现信号 (4 类, 强→弱): git submodule (.gitmodules) > 嵌套独立 .git 子仓 >
#   workspace 清单 (package.json workspaces / pnpm-workspace.yaml / go.work) > 约定目录 (packages|apps|services|libs/*)
import argparse
import glob
import json
import os
import re
import sys

_PRUNE = {"node_modules", ".git", ".worktrees", ".trellis", "dist", "build",
          "vendor", "target", ".next", "__pycache__", ".venv", "venv"}
_MANIFESTS = ("package.json", "go.mod", "Cargo.toml", "pyproject.toml")


def _name(path):
    return os.path.basename(path.rstrip("/")) or path.replace("/", "-")


def discover(repo):
    """→ {path: {name, type, git}} (path 相对 repo, 去重, 强信号覆盖弱)。"""
    found = {}  # path -> dict

    def add(rel, type_="local", git=False, require_manifest=False):
        rel = rel.strip("/").replace(os.sep, "/")
        abs_ = os.path.join(repo, rel)
        if not rel or rel == "." or not os.path.isdir(abs_):
            return
        if require_manifest and not any(os.path.isfile(os.path.join(abs_, mf)) for mf in _MANIFESTS):
            return                            # workspace/约定目录成员须含项目 manifest, 降误报
        cur = found.get(rel)
        # 强信号覆盖: submodule > git > local
        rank = {"submodule": 3, "git-sub": 2, "local": 1}
        new_r = 3 if type_ == "submodule" else (2 if git else 1)
        if cur and rank.get(cur.get("_r", "local"), 1) >= new_r:
            return
        d = {"name": _name(rel), "path": rel}
        if type_ == "submodule":
            d["type"] = "submodule"; d["_r"] = "submodule"
        elif git:
            d["git"] = "true"; d["_r"] = "git-sub"
        else:
            d["_r"] = "local"
        found[rel] = d

    # 信号 1: git submodules
    gm = os.path.join(repo, ".gitmodules")
    if os.path.isfile(gm):
        try:
            for m in re.finditer(r"(?m)^\s*path\s*=\s*(.+?)\s*$", open(gm, encoding="utf-8").read()):
                add(m.group(1), "submodule")
        except Exception:
            pass

    # 信号 2: 嵌套独立 .git 子仓 (深度 ≤4, 剪枝)
    for dp, dns, _ in os.walk(repo):
        depth = dp[len(repo):].count(os.sep)
        if depth >= 4:
            dns[:] = []
            continue
        dns[:] = [d for d in dns if d not in _PRUNE]
        if dp != repo and (os.path.exists(os.path.join(dp, ".git"))):
            add(os.path.relpath(dp, repo), git=True)

    # 信号 3a: package.json workspaces
    pj = os.path.join(repo, "package.json")
    if os.path.isfile(pj):
        try:
            ws = json.load(open(pj, encoding="utf-8")).get("workspaces")
            pats = ws.get("packages", []) if isinstance(ws, dict) else (ws or [])
            for pat in pats:
                for p in glob.glob(os.path.join(repo, pat)):
                    if os.path.isdir(p):
                        add(os.path.relpath(p, repo), require_manifest=True)
        except Exception:
            pass
    # 信号 3b: pnpm-workspace.yaml
    pw = os.path.join(repo, "pnpm-workspace.yaml")
    if os.path.isfile(pw):
        try:
            for m in re.finditer(r"(?m)^\s*-\s*['\"]?([^'\"\n]+?)['\"]?\s*$", open(pw, encoding="utf-8").read()):
                for p in glob.glob(os.path.join(repo, m.group(1))):
                    if os.path.isdir(p):
                        add(os.path.relpath(p, repo), require_manifest=True)
        except Exception:
            pass
    # 信号 3c: go.work
    gw = os.path.join(repo, "go.work")
    if os.path.isfile(gw):
        try:
            for m in re.finditer(r"(?m)^\s*(?:use\s+)?\.?/?([\w./-]+)\s*$", open(gw, encoding="utf-8").read()):
                cand = m.group(1)
                if cand not in ("use", "(", ")") and os.path.isdir(os.path.join(repo, cand)):
                    add(cand, require_manifest=True)
        except Exception:
            pass

    # 信号 4: 约定目录 (含项目 manifest 才算, 降误报)
    for base in ("packages", "apps", "services", "libs", "modules"):
        bd = os.path.join(repo, base)
        if not os.path.isdir(bd):
            continue
        for sub in sorted(os.listdir(bd)):
            sp = os.path.join(bd, sub)
            if os.path.isdir(sp) and any(os.path.isfile(os.path.join(sp, mf)) for mf in _MANIFESTS):
                add(os.path.join(base, sub))

    # 去内部 _r 字段, 处理 name 冲突
    out, names = {}, {}
    for rel, d in sorted(found.items()):
        d.pop("_r", None)
        nm = d["name"]
        if nm in names:                       # 同名 → 用路径前缀消歧
            nm = rel.replace("/", "-")
            d["name"] = nm
        names[nm] = rel
        out[rel] = d
    return out


def _has_real_packages(cfg_text):
    """config 是否已有实值 (非注释) packages: 块。"""
    return bool(re.search(r"(?m)^packages:\s*$", cfg_text))


def _render(pkgs):
    lines = ["", "# [trellisx] 自动发现的 monorepo 包 (apply 一次扫描; 仓结构变化重跑 trellisx-packages.py apply)", "packages:"]
    for d in pkgs.values():
        lines.append(f"  {d['name']}:")
        lines.append(f"    path: {d['path']}")
        if d.get("type"):
            lines.append(f"    type: {d['type']}")
        if d.get("git"):
            lines.append(f"    git: {d['git']}")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["discover", "apply"])
    ap.add_argument("--repo", default=".")
    args = ap.parse_args()
    repo = os.path.abspath(args.repo)
    pkgs = discover(repo)

    if args.mode == "discover":
        print(json.dumps({"packages": list(pkgs.values()), "count": len(pkgs)}, ensure_ascii=False, indent=2))
        return

    # apply
    cfg = os.path.join(repo, ".trellis", "config.yaml")
    if not os.path.isfile(cfg):
        print("trellisx-packages: 无 .trellis/config.yaml, 跳过", file=sys.stderr); sys.exit(0)
    s = open(cfg, encoding="utf-8").read()
    if not pkgs:
        print("trellisx-packages: 未发现 monorepo 包 → 保持单仓 (不写 packages:)", file=sys.stderr); return
    if _has_real_packages(s):
        print(f"trellisx-packages: config 已有实值 packages: → 不覆盖。发现 {len(pkgs)} 包供人工核对:", file=sys.stderr)
        for d in pkgs.values():
            kind = d.get("type") or ("git" if d.get("git") else "local")
            print(f"  - {d['name']}: {d['path']} ({kind})", file=sys.stderr)
        return
    open(cfg, "w", encoding="utf-8").write(s.rstrip() + "\n" + _render(pkgs))
    print(f"trellisx-packages: 写入 {len(pkgs)} 包到 config.yaml packages:", file=sys.stderr)


if __name__ == "__main__":
    main()
