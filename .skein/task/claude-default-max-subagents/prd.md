# 默认设置加 CLAUDE_CODE_MAX_SUBAGENTS_PER_SESSION=1000 — PRD

## 目标
- [ ] src-tauri/defaults/settings.json env 段新增 CLAUDE_CODE_MAX_SUBAGENTS_PER_SESSION=1000，放开单会话 subagent 并发上限。

## 边界
- [ ] 范围内：settings.json env 段加 1 键（subagent 相关，CLAUDE_CODE_FORK_SUBAGENT 附近）。
- [ ] 范围外：不改 Rust 同步注入逻辑（settings.json 是前端 DEFAULT_SETTINGS 派生 + do_sync_group_settings 真值源，已有路径自动同步）；不动其他 env 键。

## 验收标准
- [ ] yarn build 通过
- [ ] yarn test 全过
- [ ] settings.json env 段含 CLAUDE_CODE_MAX_SUBAGENTS_PER_SESSION=1000

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json
