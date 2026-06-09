---
updated: 2026-06-09
rewrite-version: 1
supersedes:
  - guides/cross-layer-thinking-guide.md (v0 descriptive + Trellis internals)
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
- 新增后端 command 必须同时更新前端类型定义 + API 函数，缺一不改

## Data Flow (MUST)

- 数据流必须单向: Rust command → `invoke` → React `useState` → JSX render
- 禁在 React 组件中直接操作 Tauri store / 文件系统 — 必经 `src/services/` 层
- 异步数据获取必须用 `useEffect + useState<loading>` pattern
- 错误必须 `catch` 并至少 `console.error`，禁静默丢弃

## Format Contracts (MUST)

- 后端返回 timestamp 必须为 ISO 8601 string (`chrono::DateTime<Utc>.to_rfc3339()`)
- enum 值跨层必须用 lowercase kebab-case (`"openai"` not `"OpenAI"` / `"OPENAI"`)
- 可空字段必须用 `| null`（非 `| undefined`）在 API boundary
- 新增字段必须 backward compatible: 旧前端面对新后端响应不得 crash
- 集合字段空时必须返回 `[]` 而非 `null`

## CRUD Pattern (MUST)

- 每个 resource 必须在 `api.ts` 提供 `{ create, list, get, update, delete }` 五件套（若业务需要）
- list 返回 `T[]`，get 返回 `T | null`
- create/update 入参 interface 字段必须与 Rust struct 一致

## Verification

```bash
# 所有 invoke 集中在 api.ts
grep -rn 'invoke(' src/ | grep -v 'services/api.ts' | grep -v 'vite-env'  # 必须 0 行

# 每个 invoke 有泛型标注
grep -rn 'invoke(' src/services/api.ts | grep -v 'invoke<'  # 必须 0 行

# snake_case 字段名
grep -rn 'camelCase\|camel_case' src/services/api.ts  # 必须 0 行
```
