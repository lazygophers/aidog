---
updated: 2026-07-07
rewrite-version: 3
supersedes:
  - guides/cross-layer-thinking-guide.md (v0 descriptive + Trellis internals; v1 renamed → cross-layer-rules.md)
authored-by: trellisx-spec
mode: optimize
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
