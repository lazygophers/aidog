# skills 被移除复现 — 止血移除方案 (P0)

## 用户决策 (止血优先)

「要不先移除所有测试文件、自动化逻辑、定时逻辑可能涉及到skills删除的地方以确保可用，现在直接不可用了」

**策略变更**: 不再追求优雅加固守卫, 直接**砍掉所有非用户主动触发 skills 删除的代码路径**。止血 > 优雅。用户当前 app 不可用 (skills 持续被删)。

## 范围 (三类全砍)

### 1. 测试文件 (可能删 skills 的全删/禁)
- `src-tauri/src/gateway/skills/test_bulk.rs` (align/enable_all, 历史已隔离但仍风险)
- `src-tauri/src/gateway/skills/test_ops.rs` (uninstall/uninstall_all/update)
- `src-tauri/src/gateway/skills/test_npx.rs` (守卫测试)
- 其他 skills 测试凡调真实写操作 (enable/disable/install/uninstall/align/enable_all/update/uninstall_all) 的 — 全删或 #[ignore]
- `src-tauri/src/gateway/import_export/skills_sync.rs` 测试模块 (import 真实 npx add 路径)
- **保留**: 纯函数 / args 断言 / 早返回 / tempdir 隔离的测试 (不碰 ~/.agents)

### 2. 自动化逻辑 (非用户主动)
agent 深度审计找:
- startup / app init 自动跑 align_agents / enable_all / uninstall
- import_export 自动 skills_sync (import .aidogx 时真实 npx add — 增不删, 但若含 remove 逻辑砍)
- 任何 hook / 事件订阅触发 skills 删除
- skills_sync.rs:141 独立 Command::new("npx") 路径

### 3. 定时逻辑
- cron / interval / tokio timer 调 skills 删除操作
- 后台周期任务

## 保留 (用户主动, 正常功能)

- `skills_uninstall` / `skills_uninstall_all` Tauri command (用户点删除按钮触发) — **功能保留**, 但加保护: 确认是否真用户触发 (非自动)
- ops.rs 生产 uninstall 逻辑 (用户主动用)
- enable/disable/install/align/enable_all 用户主动触发

## 加固 (防御纵深, 即使漏网也不删)

`ops.rs` fs 兜底删 (line 270-348) + npx remove 路径:
- 加运行时守卫: 仅当 Tauri command 层用户主动调用时执行 (非 cfg(test), 非 automation context)
- 或: 全局开关 env `AIDOG_ALLOW_SKILLS_DELETE`, 默认关, 用户主动删时临时开

agent 评估最优。

## 验收 (硬门)

1. **全仓无任何「非用户主动」skills 删除路径** (测试 / 自动化 / 定时 / 启动钩子)
2. `cargo test --lib` + `--all-targets` + `cargo clippy -- -D warnings` 全跑, ~/.agents 全程不变
3. `cargo run` / app dev 启动后无任何自动 skills 删除
4. 用户主动 uninstall 功能仍工作 (正常删指定 skill)
5. 无新 warning

## 不改 (保留功能)

- 用户主动 skills uninstall / uninstall_all (Tauri command 层)
- skills 列表读取 / 安装 / 启用禁用 (用户主动)

## 关联

- [[skills-removal-cfg-test-hard-guard]] (前序, cfg(test) 守卫 — 不够, 本 task 升级为全路径砍)
- [[skills-test-isolation]] (已归档前序)
