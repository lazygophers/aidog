---
title: subagent-hook-scope
layer: recall
category: skein
keywords: [subagent,hook,scope,worktree,output-format,repo-pollution]
status: active
protected: true
---

## subagent hook 禁写主仓报告文件

## 触发场景

派 researcher / workflow subagent 时，如果在 hook（如 subagent 中断返回）中允许它产生 `findings.md` 等总结文件，这些文件会写到主仓目录而非隔离的 worktree/output 里，污染主仓 git 状态或与既有文件冲突。

## 陷阱 & 正解

❌ **陷阱**：派 researcher 时不限制产物路径，允许 hook 中写报告文件

```python
# 派 researcher subagent
dispatch(skill="researcher", prompt="""
  任务：调研 X
  输出格式：产生 findings.md 和 recommendations.md 文件
""")

# researcher 完成后在 hook 中写主仓
# findings.md 落在 /Users/luoxin/persons/lyxamour/aidog/findings.md
# 污染主仓，git 状态混乱
```

主仓被污染，commit 历史掺杂临时分析文件。

✅ **正解**：subagent hook 放行 `research/` 目录下产物，报告由 main 代写

派 subagent 时明确说明：

```python
dispatch(skill="researcher", prompt="""
  目标：调研 X
  工作目录：/Users/luoxin/persons/lyxamour/aidog/research/
  
  输出格式 JSON（返回 SendMessage）：
  {
    "status": "done",
    "findings": [...],
    "recommendations": [...],
    "files_written": ["research/raw-logs.txt", "research/analysis.tsv"]
  }
  
  禁：产生 findings.md / report.md 等总结文件
  由 main 接收 JSON 后代写总结并 commit
""")
```

subagent 只在 `research/` 子目录写原始数据（日志、分析表等），总结由 main agent 基于 JSON 代写并 commit。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| 派时未限制产物路径 | 明确 `工作目录: research/`，禁产报告文件 |
| 允许 hook 写主仓文件 | hook 仅返回 JSON，由 main 代写总结 |
| `findings.md` 写进主仓 commit | 原始数据存 `research/`，总结由 main 组织 commit |

## 案例

派 researcher 调研某模块时，它直接在主仓根目录产生 `findings.md` 和 `recommendations.json`。这两个文件未被 gitignore，污染了 git status；后续 main commit 时需手动 `git rm` 清理。改为明确 worktree scope 后，researcher 写 `research/raw-findings.json`，main 接收 JSON 后代写 `research/FINDINGS.md` 并 commit，git 状态始终清洁。

## 适用

- 派遣 researcher / workflow / skill 等 data-producing subagent
- 避免主仓污染的一般原则
- worktree 隔离的最佳实践
