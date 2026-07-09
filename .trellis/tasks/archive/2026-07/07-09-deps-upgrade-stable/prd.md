# 依赖全部升级最新稳定版（确保稳定优先）

## Goal

把前端（package.json）+ Rust 后端（Cargo.toml）依赖升级到**最新稳定版**，前置约束 = **确保稳定**（不引入破坏性回归）。分层评估风险，按风险档分批升级 + 每批验证门禁，跨 major 的破坏性升级逐个评估迁移成本，能升则升、风险过大的留版本 + 记原因。

**为什么**：用户要「依赖全部升级最新但确保稳定」。一次性盲目跳最新（`cargo upgrade --incompatible` + `yarn upgrade-interactive --latest` 全勾）几乎必炸——rusqlite 0.32→0.37、rand 0.8→0.9、dirs 5→6 等 major bump 带 API 破坏，DB/加密/路径层炸了回归重。需分档策略：semver 内安全升（cargo update + yarn upgrade 在范围内）+ 逐个评估 major bump。

## 现状（已读 manifest）

### 前端 package.json（35 项）
- 框架：react/react-dom ^19.1.0、react-i18next ^17、i18next ^26
- 构建：vite ^7、@vitejs/plugin-react ^4、vitest ^4、typescript ~5.8.3（tilde 锁 minor）
- Tauri 插件：@tauri-apps/api ^2 + 9 个 plugin（clipboard-manager/dialog/fs/notification/opener/process/shell/updater）
- 工具：@dnd-kit/*、clsx、pinyin-pro、qrcode、react-markdown、remark-gfm、yaml
- 测试：@testing-library/* 4 项、jsdom、@vitest/coverage-v8

### Rust Cargo.toml（~40 项）
- **高危跨 major**：rusqlite 0.32、tokio-rusqlite 0.6、rand 0.8、dirs 5、serde_yml 0.0.13、tts 0.26
- **Tauri 生态**：tauri 2（+ 9 plugin）—— 锁 major 2，minor 升
- **web 栈**：axum 0.8、hyper 1、hyper-util 0.1、reqwest 0.12、tower 0.5
- **序列化/时间**：serde 1、serde_json 1、toml 0.8、chrono 0.4、uuid 1、semver 1
- **runtime/异步**：tokio 1、futures 0.3、tracing 0.1、tracing-subscriber 0.3、tracing-appender 0.2
- **加密**：aes-gcm 0.10、hmac 0.12、rustls 0.23、rustls-pemfile 2
- **其他**：flate2 1、tempfile 3、url 2、regex 1、dirs 5

### 工具链
- 无 rust-toolchain.toml pin（rustc 用系统默认）。
- Tauri 2.0 项目（非 1.x，迁移已过）。

## 风险分档（待用户确认策略）

| 档 | 操作 | 风险 | 验证 |
| --- | --- | --- | --- |
| **A 安全升**（semver 内） | `cargo update`（守 Cargo.toml 上界）+ `yarn upgrade`（守 ^/~ 范围） | 低 | cargo test + yarn build |
| **B minor 跨**（Tauri 2.x、axum/hyper/reqwest minor） | 放宽 Cargo.toml 上界到最新 minor | 中 | 全门禁 + dev 冒烟 |
| **C major 跨低风险**（dirs 5→6、tts、serde_yml、aes-gcm、hmac） | 改 Cargo.toml major 版本 + 迁移 API 调用点 | 中高 | 全门禁 + 受影响单测 |
| **D major 跨高风险**（rusqlite 0.32→0.37、tokio-rusqlite 0.6→0.7、rand 0.8→0.9） | 评估 changelog → 逐 API 迁移 → 专项测试 | 高 | DB 层专项测 + 全量回归 |
| **E 框架 major 锁**（tauri 2→3 若出、react 19→20） | **禁动**（跨框架 major = 重写级） | 极高 | N/A |

## Requirements（待 brainstorm + grill 确认范围）

### R0 范围与策略（**MUST 用户拍板**，AskUserQuestion）
- R0.1 范围：前端 + Rust 全部 / 仅前端 / 仅 Rust？
- R0.2 风险上限：A+B（守 major，最稳）/ A+B+C（跨低风险 major）/ A+B+C+D（全跨含高危 rusqlite 等，最激进）？
- R0.3 批次粒度：每档一批 commit / 每依赖一批 / 一锅出？
- R0.4 工具链 rustc：是否 pin（rust-toolchain.toml）？

### R1 升级前基线（必须先建）
- R1.1 记录当前 `cargo tree` + `yarn why` 版本快照（git commit baseline）。
- R1.2 跑当前全门禁（cargo test + cargo clippy + yarn build）记绿基线，升后对比。

### R2 升级执行（按 R0.2 选定档）
- R2.1 A 档：`cargo update` + `yarn upgrade`（守范围）。跑门禁。
- R2.2 B 档（若选）：grep Tauri 2.x / axum / hyper / reqwest 最新 minor，放宽 Cargo.toml 上界。跑门禁。
- R2.3 C 档（若选）：逐依赖跨 major（dirs/tts/serde_yml/aes-gcm/hmac 等），每个 grep 调用点 + changelog 迁移 + 门禁。
- R2.4 D 档（若选）：rusqlite / tokio-rusqlite / rand 跨 major专项——读 changelog / release notes、迁移 API（rusqlite 0.32→0.37 Connection/Statement 签名变化、rand 0.8→0.9 trait 路径变化）、跑受影响单测 + DB 层回归。
- R2.5 前端跨 major（若选）：typescript 5.8→5.9、@types/* 同步、@dnd-kit/* minor、@testing-library/*。

### R3 稳定性门禁（每批跑完才进下一批）
- R3.1 Rust：`cargo build` + `cargo clippy`（无新 warning）+ `cargo test`（db/proxy/converter/router/usage_color 等全过）。
- R3.2 前端：`yarn build`（tsc + vite）+ tsc 无新 type error。
- R3.3 跨层：`yarn tauri dev` 启动冒烟（应用能起、平台列表/代理请求基本功能）—— 可选，子代理在 worktree 内无法跑 dev GUI，降级为编译通过即可。
- R3.4 回退预案：某依赖升级炸门禁 → 单独 revert 该依赖（baseline 可控），不影响其他已升。

### R4 记录
- R4.1 升级清单：每依赖 old→new + 升级档（A/B/C/D）+ 是否破坏性 + 验证结果。
- R4.2 留版本原因清单：未升的依赖（如 tauri 2→3）记原因。
- R4.3 journal 记录 rusqlite 等 major 迁移踩坑（可作 sediment 候选）。

## Acceptance Criteria（待 R0 定档后细化）
- [ ] 选定档内依赖全升级到最新稳定版
- [ ] cargo test + cargo clippy + yarn build 全绿
- [ ] 无新 warning（clippy）/ 无新 type error（tsc）
- [ ] 跨 major 的破坏性升级逐依赖迁移调用点 + 验证
- [ ] 升级清单 + 留版本原因清单落 journal
- [ ] 主仓零改动（worktree 内）

## Definition of Done
- 选定范围 + 风险档内依赖全升，门禁全绿
- 破坏性升级有迁移记录
- 留版本的依赖有原因记录
- baseline 可回退

## Out of Scope（待定）
- 框架跨 major（Tauri 2→3 / React 19→20）= 禁
- 新增依赖（仅升现有）
- 改 lockfile 策略（yarn.lock / Cargo.lock 沿用）

## Technical Notes
- rusqlite 0.32→0.37 changelog：Connection API、Features gate、Error 类型有变；本仓 src-tauri/src/gateway/db.rs 直 SQL，调用点需逐 grep。
- rand 0.8→0.9：`rand::thread_rng()` 改 `rand::rng()`、`Rng::gen_range` 改 `Random::random_range` 等 trait 路径变。
- dirs 5→6：API 大体兼容，个别平台 path 变化。
- serde_yml 0.0.13：未正式 release，API 漂移大；评估换 serde_yaml（陈旧但稳）或留 0.0.13。
- 既有 guide：`.trellis/spec/backend/index.md`（Rust 约定）+ `.trellis/spec/frontend/index.md`。

## Decision (ADR-lite) — 用户超时按推荐档固化（2026-07-09）

**Context**：用户 AskUserQuestion 超时未答（300s），按 CoreRule 选推荐档（可事后 re-ask）。

**Decision**（推荐档）：
1. **范围** = 前端 + Rust 全部。
2. **风险上限** = A+B+C：
   - A 档（cargo update + yarn upgrade，守 semver）
   - B 档（Tauri 2.x / axum / hyper / reqwest / tower 等 minor 放宽上界）
   - C 档（跨低风险 major：dirs 5→6、tts、serde_yml、aes-gcm、hmac 等，逐依赖迁移调用点）
   - **D 档禁动**：rusqlite 0.32→0.37、rand 0.8→0.9、tokio-rusqlite 0.6→0.7 —— 高危跨 major，**留当前版本 + 记原因**（DB 层 + 加密层核心，回归风险高于收益）。
3. **批次** = 分档分批：A → B → C 顺序，每批门禁绿才进下一档。每档一个 commit。
4. **框架 major 锁**（E 档）：Tauri 2→3 / React 19→20 禁动。

**Consequences**：
- rusqlite / rand / tokio-rusqlite 不升 = 留版本清单注明「跨 major 高危，本次跳过，需专项迁移任务」。
- C 档 dirs 6 / serde_yml 最新 / tts 最新需 grep 调用点迁移（dirs 本仓 src-tauri 内有多处 path 拼接，serde_yml 在 import_export + SmartPaste share 解析，tts 在语音通知）。
- A+B+C 全门禁绿后可 finish；D 档另开专项任务。

## Acceptance Criteria（固化后）
- [ ] A 档：cargo update + yarn upgrade 跑完，cargo test + clippy + yarn build 绿
- [ ] B 档：Tauri 2.x / axum / hyper / reqwest / tower 等 minor 升到最新，门禁绿
- [ ] C 档：dirs 6 / serde_yml / tts / aes-gcm / hmac 跨 major 升级，调用点迁移，门禁绿（含受影响单测）
- [ ] D 档留版本清单：rusqlite / rand / tokio-rusqlite 记「跳过 + 原因 + 专项迁移提示」
- [ ] 升级清单落 journal（每依赖 old→new + 档 + 验证）
- [ ] 主仓零改动（worktree 内）

## 调度状态
- 任务建为 planning 态，排队等 active 集空槽（当前 purge + protocol-name-not-type 在跑，上限 2）。
- 槽位释放后 → grill 硬门 2（若用户回来可 re-ask 确认档）→ start → exec 分档分批。
