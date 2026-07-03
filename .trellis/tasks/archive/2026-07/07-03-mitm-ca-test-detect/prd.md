# PRD — mitm CA 安装失败测试驱动检测（阶段 B，渐进测可测层）

## 背景
阶段 A 诊断（console.error `[ca-install]` 全字段 + setError 非空兜底）+ 阶段 C（keychain 实状双向校验）已 merge master。
阶段 B = osascript 根因修，原阻塞等用户实机复现 stderr。用户决策（2026-07-03）：改用**测试驱动检测**，不靠手动复现。

## 用户决策（grill 已确认）
**测试边界 = 渐进测可测层**（非架构改后端接管执行）。osascript admin GUI 密码交互黑盒接受间接覆盖，测试驱动聚焦：
1. `classifyTrustError` 分类逻辑（前端纯函数后端化 + Rust 单测）
2. osascript 命令字面 / AppleScript 语法集成测试（非交互，CI 可跑）

## 目标
- 把 `classifyTrustError`（当前 `MitmConfig.tsx:25-48` 前端纯函数）后端化为 Rust 真源 `classify_trust_error`，消除前后端双源分类逻辑
- Rust 单测覆盖三 OS × 各 exit code / stderr 组合（含 `code=null/undefined` 兜底，验证「无文案」根因是否分类逻辑 bug）
- osascript 命令语法集成测试（spawn `osacompile`/`osascript -e` 编译检查，不跑 admin，验 AppleScript 串语法合法）

## 产出

### D1 — classify_trust_error 后端化（Rust 真源 + 单测）
- `ca.rs` 加 `pub fn classify_trust_error(name: &str, code: Option<i32>, stderr: &str) -> TrustErrorKind`
  - `TrustErrorKind` enum: `Cancel` / `AuthFail` / `NoAgent` / `CmdFail`（镜像前端 `TrustErrorKind`）
  - 三 OS 分支逻辑**逐行等价**前端 `MitmConfig.tsx:25-48`（linux 126/127+agent / macos (-128)/authorization / windows 1223）
  - `code: Option<i32>`（不是 i32）— Tauri shell plugin `out.code` 可能 null/undefined，后端用 Option 显式建模
- `ca.rs` `#[cfg(test)]` 加单测矩阵：
  - linux: code=126→Cancel / code=127+stderr含"agent"→NoAgent / code=127+stderr含"polkit"→NoAgent / code=127+其他→AuthFail / code=1→CmdFail
  - macos: stderr含"(-128)"→Cancel / stderr含"authorization"→AuthFail / stderr含"鉴权"→AuthFail / 其他→CmdFail
  - windows: stderr含"1223"→Cancel / stderr含"cancel"→Cancel / 其他→CmdFail
  - **兜底**：code=None 三 OS 均落 CmdFail（非 panic，验证「无文案」不是分类崩）
- 新 command `mitm_classify_trust_error(name, code, stderr)` → `TrustErrorKind`（lib.rs `#[tauri::command]` + commands/mitm.rs 转发）
- 前端 `MitmConfig.tsx`：删本地 `classifyTrustError`，改 `await mitmApi.classifyTrustError(spec.name, out.code ?? null, out.stderr ?? "")` invoke 后端

### D2 — osascript 命令语法集成测试（ca.rs `#[cfg(test)]`）
- macOS only (`#[cfg(target_os = "macos")]`)，CI Linux 跳过不报错
- 测试 `trust_ca_command` 产出的 AppleScript 串语法合法：
  - spawn `/usr/bin/osacompile -e <applescript串> -o /tmp/空.scpt`（仅编译，不执行，不需 GUI/admin）
  - exit 0 = 语法合法；非 0 = AppleScript 串本身有语法错（检测转义/引号 bug）
- 同测 `untrust_ca_command` 的 delete-certificate 串
- 非 macOS 平台：测试用 `#[cfg(not(target_os="macos"))]` 空桩占位（保持跨平台 cargo test 绿）

### D3 — Tauri 边界契约同步
- `services/api/mitm.ts` 加 `classifyTrustError(name, code, stderr)` invoke 封装（snake_case args）
- `TrustErrorKind` TS union type 同步 Rust enum 变体

## 验证
- [ ] `classify_trust_error` Rust 单测矩阵全绿（三 OS × 各组合 + None 兜底）
- [ ] `cargo test classify` + `cargo test trust_ca_command` + `cargo test osacompile`（macOS）/ 跨平台 cargo test 绿
- [ ] `cargo clippy` 0 warning
- [ ] 前端 `classifyTrustError` 删除，invoke 后端单源（grep `MitmConfig.tsx` 无本地分类函数）
- [ ] `yarn build` 绿
- [ ] `node scripts/check-i18n.mjs` exit 0（无新 key，分类文案 key 复用 mitm.installCancel/installAuthFail/installNoAgent/installFailed）
- [ ] 跨层契约对齐（Rust command 签名 ↔ TS invoke args ↔ serde 序列化）

## 非目标
- ❌ osascript admin GUI 交互路径直接测试（黑盒，测试环境无密码框）— 接受间接覆盖（语法锁 + 诊断日志）
- ❌ 架构改后端接管执行（std::process + SUID/polkit）— 用户决策渐进，不押大改
- ❌ 真 root CA 装载端到端测试（需 GUI + admin，非可测层）
- ❌ Windows/Linux 提权链路改动（本 task macOS osascript 焦点，Windows/Linux 仅分类逻辑单测跟随）

## 调度
- 与 `07-03-proxy-http-relay-p2` 并行（文件域不相交：本 task = ca.rs / commands/mitm.rs / MitmConfig.tsx / mitm.ts vs proxy-p2 = connect.rs / handler.rs）
- 单 subtask（D1+D2+D3 一体，文件集高度耦合：classify_trust_error 后端化贯穿 Rust command + TS invoke + 前端调用点），单 subagent 一次做

## grill 自审 trace（main 同步过轴）
- 轴 A 目标 ✓ 封闭（后端化 + 单测 + 语法集成）
- 轴 B 产出 ✓ 可验收（cargo test 矩阵 + clippy + build + i18n）
- 轴 C 验证 ✓ 可执行断言（cargo test classify/osacompile + check-i18n）
- 轴 D 资源 ✓ 文件集列明，与 proxy-p2 不相交
- 轴 E 依赖 ✓ 单 subtask 无内部依赖
- 轴 F 失败模式：osacompile 测试在 CI 无 macOS runner → `#[cfg]` 空桩占位跨平台绿
