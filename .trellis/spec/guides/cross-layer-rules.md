---
updated: 2026-07-10
rewrite-version: 6
supersedes:
  - guides/cross-layer-thinking-guide.md (v0 descriptive + Trellis internals; v1 renamed → cross-layer-rules.md)
authored-by: trellisx-spec
mode: sediment
---

# Cross-Layer Rules

何时被读: 改动跨越 Rust↔TypeScript 边界的功能时
谁读: trellis-implement sub-agent / main
不遵守的代价: 前后端契约断裂 → invoke 调用失败 / 类型不匹配 / 运行时 crash

---

## Tauri↔React Boundary (MUST)

- 后端新增 Tauri command 必须在前端 `api.ts` 添加对应 invoke 包装函数
- invoke 包装必须标注返回值泛型: `invoke<T>(command, args)`
- 前端入参类型必须与 Rust command 函数签名一一对应
- 字段名必须 snake_case（Tauri 序列化要求），前端 `api.ts` interface 用 snake_case
  - 后果: 字段非 snake_case → Tauri serde 反序列化字段名对不上 → 前端拿到 `undefined`，渲染空白且无报错，排查极耗时
- 新增后端 command 必须同时更新前端类型定义 + API 函数，缺一不改
  - 后果: 只加后端不加前端包装 → 前端无从调用，新 command 形同死代码；只改类型不改 invoke → 编译过但运行时参数缺失

## Data Flow (MUST)

- 数据流必须单向: Rust command → `invoke` → React `useState` → JSX render
- 禁在 React 组件中直接操作 Tauri store / 文件系统 — 必经 `src/services/` 层
- 异步数据获取必须用 `useEffect + useState<loading>` pattern
- 错误必须 `catch` 并至少 `console.error`，禁静默丢弃

## Tauri 窗口生命周期事件 (MUST)

- 窗口生命周期事件 (失焦 `Focused` / 关闭 `CloseRequested` / 缩放 `Resized` / 移动 `Moved`) **MUST 在 Rust 端 `Builder::on_window_event` handler 处理**, **禁 webview 端 `getCurrentWindow().onFocusChanged()` 等 IPC 监听做关键副作用** (关闭/隐藏/销毁窗口)
  - 后果: webview 端监听经 JS→Rust IPC 链, macOS 下偶发失效 (React mount 时序 / IPC 桥未就绪 / event 注册丢) → 失焦不关闭等静默 bug; Rust 端 handler 由 tao 同步派发, 先于且独立于 webview IPC, 根治
  - 范式源: Tauri `app.rs` 官方文档示例 (hide window on `Focused(false)`); tao macOS `window_delegate.rs` `window_did_resign_key` 同步 emit `WindowEvent::Focused(false)`
  - label 过滤: handler 内 `if window.label() == "<target>"` 限定窗口, 避免误伤其他窗口
  - 实例: popover 失焦 destroy → `startup.rs` `.on_window_event` 链 (src-tauri/src/startup.rs:29-33)
- **floating window (`always_on_top=true`) 例外 (MUST)**: `on_window_event Focused(false)` 对 `NSFloatingWindowLevel` 窗口**不够** — macOS 在 3 场景**不**调 `windowDidResignKey:` → `Focused(false)` 不派发: ① 点桌面壁纸 (Finder desktop `canBecomeKeyWindow=NO`, 无窗口接 key) ② 主窗口 `hide()` (silent_launch) 后点别的 app (无可见 key-eligible 窗口接 key) ③ 点菜单栏/Dock 空白。证据: tao 0.35.3 `Focused(false)` 唯一发射点 = `window_delegate.rs:384 windowDidResignKey:`; tao app_delegate 无 `applicationDidResignActive:`; tauri `RunEvent` 无 app 级失活事件
  - **正解**: floating popover 创建后**额外**调 `NSWindow.setHidesOnDeactivate:YES` (macOS 原生 popover 失活隐藏范式, app 转 inactive 自动隐藏, 覆盖 3 场景)。`setHidesOnDeactivate` 是 `NSWindow` 原生属性 (非 NSPanel 专属, Apple docs 已证), 经 `WebviewWindow::ns_window()` + objc2-app-kit `NSWindow` feature 调用, 6 行 unsafe (retain_autoreleased 拿所有权, 出作用域自动 release)
  - 组合覆盖: `setHidesOnDeactivate` (app 失活场景) + `on_window_event Focused(false)` (点主窗口场景, 主窗接 key 触发 resignKey) = 全场景
  - 实例: app_setup.rs popover `.build()` Ok 分支 `#[cfg(target_os="macos")]` 内 `ns_window.setHidesOnDeactivate(true)` (src-tauri/src/app_setup.rs:305)

## Format Contracts (MUST)

- 后端返回 timestamp 必须为 ISO 8601 string (`chrono::DateTime<Utc>.to_rfc3339()`)
- enum 值跨层必须用 lowercase kebab-case (`"openai"` not `"OpenAI"` / `"OPENAI"`)
  - 后果: enum 大写 / 大小写不一致 → 前端 switch/match 全部漏命中 → 走默认分支或抛错
- 可空字段必须用 `| null`（非 `| undefined`）在 API boundary
- 新增字段必须 backward compatible: 旧前端面对新后端响应不得 crash
- 集合字段空时必须返回 `[]` 而非 `null`
  - 后果: 空集合返 `null` → 前端 `.map()` / `.length` 直接 `TypeError` crash 整页

## CRUD Pattern (MUST)

- 每个 resource 必须在 `api.ts` 提供 `{ create, list, get, update, delete }` 五件套（若业务需要）
- list 返回 `T[]`，get 返回 `T | null`
- create/update 入参 interface 字段必须与 Rust struct 一致
- **update `#[serde(default)]` 字段须传全量** (MUST): Rust update struct 凡标 `#[serde(default)]` 的集合/Option 字段 (如 `env_vars`/`model_mappings`/`tags`), 前端 update payload **必须携带当前全量值**, 缺省即被 default (`[]`/`None`) 覆盖 → **静默清空已存数据** (非 partial merge, serde 无 `Option<T>` skip 语义除非 `#[serde(default, skip_serializing_if)]` 双标)
  - 反例: `handleAddMapping` 只传新 mapping 不带 `env_vars` → 后端 default `[]` → 用户已存 env_vars 被清空
  - 正解: 前端 update 前先持当前全量, 增删后整包传

## 反模式 (禁)

| 反模式 | 正确做法 | 触发后果 |
| --- | --- | --- |
| `invoke(` 散落在组件 / hook 内 | 全部集中到 `services/api.ts` | 契约分散难维护，类型泛型易漏标 |
| 字段名 camelCase | 一律 snake_case | Tauri serde 反序列化失败，前端拿 `undefined` |
| 顶级 invoke 参数 key 用 snake_case | 顶级参数 key 用 **camelCase** (InvokeArgs 转换层); 嵌套 struct **字段** 才用 snake_case (serde 默认, 无 rename_all) — 两层正交 | 顶级参数误 snake_case → Rust 拿 `None` 走 default; 嵌套字段误 camelCase → serde 反序列化失败 |
| enum 值大写 / 驼峰 | lowercase kebab-case | 前端 switch/match 漏命中走默认分支 |
| 空集合返 `null` | 返 `[]` | 前端 `.map()`/`.length` 抛 `TypeError` 崩页 |
| `catch` 后静默丢弃 | 至少 `console.error` | 错误吞掉，线上故障无迹可查 |
| update `#[serde(default)]` 字段前端漏传 | update payload 须含全量 default 字段 | default `[]`/`None` 覆盖已存 → 静默清空数据 |
| 组件内直接读写 Tauri store / 文件系统 | 必经 `src/services/` 层 | 绕过单向数据流，状态不可追踪 |
| webview `onFocusChanged()` 做关键副作用 (关闭/隐藏/销毁窗口) | Rust 端 `Builder::on_window_event` handler | macOS IPC 链偶发失效 → 失焦不关闭静默 bug |

## 持久化路径换、公共契约零改 (MUST)

换持久化路径（专属表 → `setting` / JSON / 他处）时，跨 Rust↔TS **公共契约层禁改** —— 只换 Rust 内部 DB 调用层：

- **公共契约层 = 三件**：① Rust 数据模型 struct 字段（名 / 类型 / 可空性）② `#[tauri::command]` 函数签名（参数名 / 类型 / 顺序 / 返回类型）③ `#[derive(Serialize, Deserialize)]` 序列化字段名（`#[serde(rename)]` / `rename_all`）
- **禁** 改这三层任一项 —— 前端 `src/services/api/<域>.ts` invoke 封装零改（cmd 字符串 + args 类型 + 返回泛型全不动）
- **仅允许** 改 Rust 内部 DB 访问层（`get_setting` / `set_setting` / 裸 SQL → setting 调用换）
- **后果**：换持久化路径若改公共契约 → 前端 invoke 参数 / 返回类型错位 → 运行时 `undefined` 静默失败，与「新增 command 未同步前端」同类 bug，但更隐蔽（不是漏加，是改坏）

**验收断言（diff 0，可复用）**：换持久化路径任务 finish 前，对 master 跑：
1. struct 字段 diff 0（`git diff master -- <struct 源文件>` 仅 `#[derive(...)]` 增 `Serialize, Deserialize` 或纯内部方法，字段定义行 0 改）
2. `#[tauri::command]` 函数签名 diff 0（`git diff master -G '#\[tauri::command\]'` 仅函数体改，签名行 0 改）
3. 前端 `src/services/api/<域>.ts` diff 0（`git diff master -- src/services/api/<域>.ts` 空）

实例：task 07-09-mitm-tables-to-setting（RootCa 6 字段 + WhitelistEntry 4 字段 + 13 个 `#[tauri::command]` 签名 0 diff，仅 SQL → `get_setting`/`set_setting` 调用换，前端 `src/services/api/mitm.ts` 零改）

## Rust enum → type alias arbitrary 全 JSON 驱动 (MUST)

Rust enum 当变体集合属「后端 JSON 真值源派生」类（值集合由 `src-tauri/defaults/*.json` 定义，前端派生层消费，如 `ClientType`）→ **MUST** 改 `pub type X = String`（serde 天然 arbitrary），**禁**保留枚举。

- **判定边界（MUST，区分两类 enum）**：
  - **适合 → String**：小枚举常量（client_type / 请求格式等 ≤20 变体），变体集合由 JSON 真值源驱动，Rust 不对变体做行为 match（或 match 仅分支选 fn，无变体专属数据）—— 改 String 后远端 JSON 新增变体不丢（原值保留）
  - **保留枚举**：协议类核心域 enum（如 `Protocol` 60+ 变体），路由 / converter 对变体做行为 match 臂依赖变体身份，变体扩展走 [Protocol 枚举变体扩展范式](../backend/protocol-enum-extension.md)（先 grep 同构变体命中点，零专属 match 臂则加枚举即覆盖）
- **enum 删后清理（MUST，grep 可验）**：
  - 未知值（JSON 新增变体 / 旧库残留）原值保留不丢 —— serde `String` arbitrary 天然支持；`deserialize_x_lenient` 改「空串/null → 默认值，非空原值保留」（不再回退 `Default::default()`）
  - migration `X::Variant` 字面量化 **禁驼峰**，用 serde rename 值（如 `ClientType::CodexTui` → `"codex_tui".to_string()`，非 `"CodexTui"`）
  - 测试 `X::Variant` → 字面量字符串（同 serde rename 值），禁保留枚举构造
  - Default impl + `default_for_x` fn **彻底删**（映射移 JSON 真值源 per-entry 字段，禁 Rust 残留第二份映射表）
- **公共契约字段名禁改（MUST）**：仅 Rust 内部类型简化（enum → type alias），公共契约层（struct 字段名 / serde 字段名 / command 签名 / 前端 TS union）字段名不动 —— 见上「持久化路径换、公共契约零改」段。前端 TS union 同步改 `string`（删字面量 union）。

**验收断言（grep 可复用）**：
```bash
# enum 彻底删（仅注释残留）
grep -rn '\bClientType::' src-tauri/  # 仅注释
grep -rn 'default_for_protocol\|default_client_for_protocol' src-tauri/  # 0
# type alias 落地
grep -n 'pub type ClientType = String' src-tauri/crates/aidog_core/src/gateway/models/platform.rs
# migration 字面量（serde rename 值，禁驼峰）
grep -n '"codex_tui"\|"claude_code"' src-tauri/crates/aidog_core/src/gateway/db/schema_early.rs
# 前端 union 同步
grep -n 'export type ClientType = string' src/services/api/types/part1.ts
```

实例：task 07-10-client-types-json-sync（Rust `ClientType` enum 13 变体 → `pub type ClientType = String`，83 命中点字面量化，migration `schema_early.rs` + 全 test 改 `"codex_tui"`/`"claude_code"` 字面量；前端 union → `string`；`Protocol` 枚举保留走变体扩展 spec）

## Verification

```bash
# 所有 invoke 集中在 api.ts
grep -rn 'invoke(' src/ | grep -v 'services/api.ts' | grep -v 'vite-env'  # 必须 0 行

# 每个 invoke 有泛型标注
grep -rn 'invoke(' src/services/api.ts | grep -v 'invoke<'  # 必须 0 行

# snake_case 字段名
grep -rn 'camelCase\|camel_case' src/services/api.ts  # 必须 0 行

# 窗口生命周期关键副作用只在 Rust on_window_event, 禁 webview IPC 监听
grep -rn 'onFocusChanged' src/  # 关键副作用必须 0 (仅注释指路 Rust 端可留)
```
