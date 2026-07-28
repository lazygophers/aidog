# clippy collapsible_if 批量修 — PRD (主入口)

## 目标
- [x] make lint (cargo clippy -- -D warnings) 复绿
## 边界
- 范围: aidog_core gateway/* 22 文件 collapsible_if + commands_platform/src/batch.rs unused import
- 非目标: 不升级 rust toolchain, 不动 clippy 配置, 不改业务逻辑
## 验收标准
- [x] cargo clippy -- -D warnings 零 warning 零 error
- [x] cargo build 通过
- [x] 改动仅 collapse if/let-chain + 清 unused import, 零业务逻辑变更
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list clippy-collapsible-if`)
