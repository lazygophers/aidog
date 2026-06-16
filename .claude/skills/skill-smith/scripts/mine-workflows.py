#!/usr/bin/env python3
"""mine-workflows.py — 从本项目的 Claude Code 会话历史 + git log 挖掘高频重复工作流，
产出「值得沉淀成 skill」的候选清单，供 skill-smith 的 CREATE 模式勾选。

自包含：仅用 Python 3 stdlib，无第三方依赖，可直接 `python3 mine-workflows.py` 运行。

数据源（只读，不改任何文件）：
- ~/.claude/projects/<cwd 编码>/*.jsonl   —— 本项目历次会话记录
- `git log`（当前仓库）                    —— 提交类型/scope 频次

用法：
  python3 mine-workflows.py [--repo <path>] [--top N] [--days D]
    --repo  仓库根（默认当前工作目录）
    --top   每类候选输出条数（默认 12）
    --days  只看最近 D 天的会话（默认全部）
"""
import argparse
import json
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path


def encode_project(repo: Path) -> str:
    """Claude Code 把 cwd 编码成 projects 子目录名：/ 替换为 -。"""
    return str(repo.resolve()).replace("/", "-")


def session_files(repo: Path):
    base = Path.home() / ".claude" / "projects" / encode_project(repo)
    if not base.is_dir():
        return []
    return sorted(base.glob("*.jsonl"))


def iter_events(files):
    for f in files:
        try:
            with f.open(encoding="utf-8") as fh:
                for line in fh:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        yield json.loads(line)
                    except json.JSONDecodeError:
                        continue
        except OSError:
            continue


def mine_sessions(files):
    """统计：Bash 首 token、Skill 调用、用户请求高频动词短语。"""
    bash = Counter()
    skills = Counter()
    user_verbs = Counter()
    # 中文动作词 + 英文动词，粗粒度信号
    verb_pat = re.compile(
        r"(优化|修复|检查|生成|新增|添加|重构|查|排查|同步|导出|导入|测试|分析|"
        r"实现|对齐|清理|补全|review|fix|add|optimize|refactor|debug|check|generate)"
    )
    for ev in iter_events(files):
        # tool_use 事件
        content = ev.get("message", {}).get("content") if isinstance(ev.get("message"), dict) else None
        if isinstance(content, list):
            for block in content:
                if not isinstance(block, dict):
                    continue
                if block.get("type") == "tool_use":
                    name = block.get("name", "")
                    inp = block.get("input", {}) or {}
                    if name == "Bash":
                        cmd = (inp.get("command") or "").strip()
                        tok = cmd.split()[0] if cmd else ""
                        # 跳过 cd/echo 等噪声
                        if tok and tok not in {"cd", "echo", "ls", "cat", "pwd"}:
                            bash[tok] += 1
                    elif name == "Skill":
                        s = inp.get("skill") or inp.get("command") or ""
                        if s:
                            skills[s] += 1
        # 用户消息
        if ev.get("type") == "user":
            msg = ev.get("message", {})
            txt = ""
            if isinstance(msg, dict):
                c = msg.get("content")
                if isinstance(c, str):
                    txt = c
                elif isinstance(c, list):
                    txt = " ".join(b.get("text", "") for b in c if isinstance(b, dict))
            for m in verb_pat.findall(txt):
                user_verbs[m] += 1
    return bash, skills, user_verbs


def mine_git(repo: Path, days):
    scopes = Counter()
    types = Counter()
    args = ["git", "-C", str(repo), "log", "--pretty=%s"]
    if days:
        args += [f"--since={days}.days.ago"]
    try:
        out = subprocess.run(args, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return types, scopes
    pat = re.compile(r"^(\w+)(?:\(([^)]+)\))?:")
    for line in out.stdout.splitlines():
        m = pat.match(line.strip())
        if m:
            types[m.group(1)] += 1
            if m.group(2):
                scopes[m.group(2)] += 1
    return types, scopes


def section(title, counter, top, hint):
    print(f"\n## {title}")
    if not counter:
        print("  (无数据)")
        return
    for name, n in counter.most_common(top):
        flag = "  ⭐候选" if n >= 3 else ""
        print(f"  {n:>4}  {name}{flag}")
    if hint:
        print(f"  → {hint}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=os.getcwd())
    ap.add_argument("--top", type=int, default=12)
    ap.add_argument("--days", type=int, default=0)
    a = ap.parse_args()
    repo = Path(a.repo)

    files = session_files(repo)
    print(f"# 工作流挖掘报告  repo={repo}  会话文件={len(files)}")
    if not files:
        print("  (未找到本项目会话记录；仅用 git log)", file=sys.stderr)

    bash, skills, verbs = mine_sessions(files)
    types, scopes = mine_git(repo, a.days)

    section("高频 Bash 命令（重复手敲 → 可封装成 skill 脚本）", bash, a.top,
            "≥3 次的命令序列优先沉淀")
    section("高频 Skill 调用（已有 skill 的使用热度）", skills, a.top,
            "热度高的 skill 优先用 skill-auditor 审计优化")
    section("用户请求高频动作（重复意图 → 工作流候选）", verbs, a.top, "")
    section("git commit type 频次", types, a.top, "")
    section("git commit scope 频次（高频 scope = 高频工作域）", scopes, a.top,
            "高频 scope 常对应一个值得沉淀的工作流")

    print("\n## 下一步")
    print("  把 ⭐候选 交给 skill-smith CREATE 模式，逐个确认是否沉淀成 skill。")


if __name__ == "__main__":
    main()
