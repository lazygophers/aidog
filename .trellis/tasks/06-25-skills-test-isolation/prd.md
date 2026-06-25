# skills 测试隔离 (P0)

## 需求 (用户)

「执行单元测试或别的检查的时候，会直接操作系统的skills，预期单元测试应该是独立的、隔离的、不影响用户设备的」

cargo test 跑 skills 测试时真实操作用户 `~/.agents/.skill-lock.json` (shell out `npx skills enable/disable/add`)。必须隔离, 测试零副作用。

## 根因 (已定位)

`src-tauri/src/gateway/skills/test_bulk.rs`:
- `align_agents_different_agents_does_not_panic` (line 21): 真实调 `align_agents(Claude, Codex, SkillScope::Global, None)` → shell out `npx skills enable/disable` 操作用户 `~/.agents`
- `enable_all_does_not_panic` (line 29): 真实调 `enable_all(Claude, SkillScope::Global, None)` → 操作用户 skills
- 注释自承 "May succeed or fail depending on installed skills" = 操作真实环境
- `align_agents_same_agent_noop` (line 13) from==to noop 安全

安全 (不 shell out): `test_env.rs` / `test_proxy_env.rs` 的 `Command::new("npx")` 只构造 cmd 测 env 注入, 不执行。

## 方案

### 方案 A (推荐, 最小改动) — 测试用 tempdir + HOME 注入

测试 fn 内:
1. 建 `tempdir()` fixture
2. `std::env::set_var("HOME", &tempdir)` 注入临时 HOME
3. 调 align_agents/enable_all → 操作落 tempdir, 不碰用户 `~/.agents`
4. 测试结束清理 tempdir (tempfile crate 自动 Drop)

需确认 align_agents/enable_all 的 HOME 读取路径 (是否走 dirs::home_dir() → 受 HOME env 影响)。若 dirs::home_dir() 在测试进程内读 HOME env, 则 set_var 生效。

注意: `std::env::set_var` 在多线程测试不安全 (cargo test 默认多线程)。若 align_agents/enable_all 跑 Global scope → dirs::home_dir() 读 HOME → 全局 env set_var 有竞态。更安全:
- **方案 A 改**: 测试改用 `SkillScope::Project { path: tempdir }` (Project scope 锁文件走 path, 不读 HOME) → 无 env 全局污染, 线程安全

### 方案 B — mock cmd builder

align_agents/enable_all 接受可注入 Command 构造器 (fn pointer / trait), 测试传 mock (不真实执行 npx, 只记录调用)。改动大但彻底。

### 方案 C — #[ignore] 集成测试

标 `#[ignore]`, cargo test 默认跳过, 需 `--ignored` 显式跑。最简但弱化覆盖。

## 推荐: 方案 A 改 (Project scope + tempdir)

agent 读 bulk.rs 的 align_agents/enable_all 实现, 确认:
1. SkillScope::Project 是否走 path 而非 HOME (list.rs:169 锁文件 Project scope 走 path ✓)
2. shell out npx 命令是否带 `-C path` 或 `--scope` 指定项目目录 (避免操作全局)
3. 改测试用 `SkillScope::Project { path: tempdir }` + tempdir fixture

若 npx skills CLI 不支持项目 scope 隔离 → 退方案 B (mock) 或方案 C (#[ignore])。

## 验收

1. `cargo test` 跑 skills 相关测试**零副作用** (不碰用户 `~/.agents/.skill-lock.json`, 不 shell out 真实 npx 修改用户环境)
2. 测试仍跑 (不简单删除/#[ignore] 除非必要) — 覆盖 align/enable 逻辑
3. 测试线程安全 (不依赖全局 env set_var 竞态)
4. `cargo test` + `cargo clippy --all-targets -- -D warnings` 全绿, 无新 warning
5. 手动验: cargo test 前后 `~/.agents/.skill-lock.json` mtime/内容不变

## 不改

- 生产逻辑 (align_agents/enable_all 业务行为不变)
- 非 skills 测试
