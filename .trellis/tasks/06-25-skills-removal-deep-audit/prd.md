# skills 被移除问题 — 深度审计 + 系统性加固 (P0)

## 现状铁证

- `~/.agents/.skill-lock.json` = **0 skills, 360 bytes** (用户 skills 已被清空)
- `~/.agents/skills/` 目录现状待 agent 查

## 用户诉求

「还是存在 skills 被移除的问题，请深度检索以确保这个问题不再出现（最高优先级）」

两层:
1. **止血**: 确保当前 master 跑 cargo test / clippy / build 不再删 ~/.agents
2. **加固**: 系统性防御, 保证未来任何测试 / 检查都不碰用户 ~/.agents

## 时间线 (已重建)

1. agent aec6d4c2 (skills-test-isolation 首轮) 二分定位时跑全 `cargo test --lib`, 真实执行 test_ops.rs:160 `uninstall_all(&Global)` → `npx skills remove --all -g` → **删空用户 ~/.agents** (他报告 sha c9945d7740 → e2c854b9)
2. agent a3c3c9ce 修 test_ops.rs (mock args builder) + test_bulk.rs (Project+tempdir), merge 到 master (614b8c1)
3. a3c3c9ce 实测 `cargo test --lib` 1039 passed sha 不变 — 但**那时 ~/.agents 已空**, 「不变」= 持续空, 不是「未删」
4. 用户再跑发现仍空 / 疑再删

## 已审计 (main grep)

- `test_bulk.rs`: 已隔离 (Project scope + tempdir) ✓
- `test_ops.rs`: 已隔离 (uninstall_all/update 改 mock args; empty_*_fails 早返回不 spawn) ✓
- `skills_sync.rs` 测试模块: 纯函数 / 空 entries / unknown agent / disabled agent 早返回, **不真实 run_npx** ✓
- `test_env.rs` / `test_proxy_env.rs`: Command::new 只构造测 env 注入, 不执行 ✓
- 无 integration tests / benches / examples / build.rs 污染 ✓
- runtime 调用全是 Tauri commands (前端 invoke 触发, 非 cargo test 自动跑) ✓

## 待 agent 深度审计 (read-only + 实测)

### S1 — 实测当前 master 是否还删 (硬验证)

1. 备份当前 `~/.agents/` (含 .skill-lock.json + skills/ 子目录) 的 sha256 + 树快照
2. 跑 `cargo test --lib --all-features` 全量 (含所有 #[test])
3. 跑 `cargo test --all-targets` (含 doctest 等)
4. 跑 `cargo clippy --all-targets -- -D warnings` (确认 clippy 不执行副作用)
5. 对比 ~/.agents sha + 树快照 — **任何变化 = 仍有删除路径, 必须定位**

### S2 — 若 S1 发现变化, 二分定位

- `cargo test --lib gateway::skills` 单模块
- `cargo test --lib gateway::import_export`
- `cargo test --lib commands`
- 逐模块跑 + 前后对比 ~/.agents
- 找出具体测试 fn

### S3 — 系统性加固 (确保不再出现)

**目标**: 测试代码结构上不可能碰用户 ~/.agents, 不靠「记得用 Project scope」。

候选方案 (agent 评估择优, 可组合):

- **方案 A — 测试 helper 强制 Project scope + tempdir**: 提取 `isolated_scope()` helper (test_bulk.rs 已有 `isolated_project_scope()`), 所有 skills 写操作测试必须用。加 lint 规则 / 约定注释。
- **方案 B — cfg(test) HOME 重定向**: 编译期测试 build script 把 HOME 指向 tempdir。风险: dirs::home_dir() 在测试进程读 HOME env, 但 cargo test 多线程 set_var 竞态 (memory [[skills-test-isolation]] PRD 已述)。仅单线程测试 (--test-threads=1) 可用。
- **方案 C — run_npx_in_scope 加 #[cfg(test)] 拦截**: 测试编译时 run_npx_in_scope 改为不 spawn (返回 mock result 或 panic on Global scope)。最硬 — 编译期保证测试不 spawn npx 写 Global。**推荐评估**: 在 `npx.rs` 的 `run_npx_in_scope` 加 `#[cfg(test)]` 分支, 若 scope==Global 则 panic("测试禁操作 Global scope, 用 Project+tempdir") 或返回 stub。这样任何测试误调 Global 真实 spawn 会立即编译期/运行期暴露, 不可能静默删用户数据。
- **方案 D — CI 门禁**: 加一个脚本 cargo test 前后对比 ~/.agents, 变化即 fail。本地也可手动跑。

推荐 **方案 C** (编译期/运行期硬拦, 最彻底) + 方案 A (helper 约定)。

### S4 — 用户 skills 恢复 (可选, 问用户)

- 查 ~/.agents 是否有备份 (Time Machine / git / .aidogx 导出)
- 查是否可从 ~/.claude/skills/ / ~/.codex/ symlink 反推
- 若不可恢复 → 报告用户, 列出可重装清单 (从锁文件 git 历史 / skills.sh catalog)

## 验收 (硬门)

1. **S1 实测**: cargo test --lib + --all-targets + clippy 全跑, ~/.agents (lockfile + skills/ 目录) sha + 树快照**完全不变**
2. S3 加固方案落地 (优先方案 C: run_npx_in_scope cfg(test) 拦截 Global)
3. 加固后重跑 S1 验证仍不变 (且拦截器存在时测试仍绿)
4. `cargo test` + `cargo clippy --all-targets -- -D warnings` 全绿, 无新 warning
5. 用户 skills 恢复方案给出 (或确认不可恢复 + 重装清单)

## 不改

- 生产业务逻辑 (skills enable/disable/install/uninstall 行为不变)
- 测试覆盖意图 (args 断言保留)

## 关联 memory

- [[skills-test-isolation]] (已归档, 本 task 延续)
- [[extractexpiryat-false-positive-fallback]] (无关, 历史误关联清理)
