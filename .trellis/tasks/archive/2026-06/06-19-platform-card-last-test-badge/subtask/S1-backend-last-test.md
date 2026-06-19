---
id: S1
slug: backend-last-test
deliverable: D1
parent-task: 06-19-platform-card-last-test-badge
status: planned
execution-layer: sub-agent
isolation: worktree
depends-on: []
blocks: [S2]
estimated-tokens: 8000
---

# S1 · 后端最近测试查询 + command + api 封装

## 目标

新增后端查询取某 platform 最近一条 `source_protocol='test'` 的 proxy_log 行，暴露为 Tauri command，并在 `src/services/api.ts` 补 TS 类型与方法封装，供 S2 前端消费。

## 产出

- `src-tauri/src/gateway/db.rs`：新增 `pub async fn get_last_test_result(db, platform_id) -> Result<Option<LastTestResult>, String>`
- `src-tauri/src/gateway/models.rs`：新增 `LastTestResult` struct（`#[derive(Serialize)]`，字段见下）
- `src-tauri/src/lib.rs`：新增 `#[tauri::command] get_last_test_result` 并注册到 invoke_handler
- `src/services/api.ts`：新增 `LastTestResult` TS 类型 + `platformApi.lastTestResult(id)` 方法

## 验证

```bash
cd src-tauri && cargo build
cd src-tauri && cargo clippy -- -D warnings
cd src-tauri && cargo test
```

期望输出:
- cargo build: 退出码 0
- cargo clippy: 零 warning
- cargo test: 全绿（不要求新增测试，但既有测试不得回归）

## 资源

- 独占文件: `src-tauri/src/gateway/db.rs` `src-tauri/src/gateway/models.rs` `src-tauri/src/lib.rs` `src/services/api.ts`
- 端口 / 服务: 无
- 环境: 无
- 审批槽位: 否

## 依赖

无上游（首发 subtask）。

## 执行细节

### 数据模型（`models.rs`）

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct LastTestResult {
    pub success: bool,        // status_code 2xx → true
    pub status_code: i32,
    pub duration_ms: i32,
    pub created_at: i64,      // proxy_log.created_at（毫秒 epoch）
    pub error: String,        // 失败时取 response_body 截断 / 失败原因，成功空串
}
```

### 查询（`db.rs`）

照 `get_platform_usage_stats`（db.rs:2789）模式：用 `db.0.call` 闭包，WHERE 同样含 `platform_id=?1` 回溯（照搬 :2793 的 where_clause 形式，但 source_protocol 限定 'test'），`ORDER BY created_at DESC LIMIT 1`。取 `status_code / duration_ms / created_at / response_body`。无行 → `Ok(None)`。

> 注意：model_test 日志 `platform_id=真实id`（lib.rs:877），直接 `platform_id=?1` 即可命中，不必走 auto_from_platform 回溯；但为稳妥保留 `OR (platform_id=0 AND ...)` 分支不必要——直接 `platform_id=?1 AND source_protocol='test'`。若担心 mock 等场景 platform_id 不一致，以 lib.rs:877 实际写入值为准。

### command（`lib.rs`）

```rust
#[tauri::command]
async fn get_last_test_result(db: State<'_, Db>, platform_id: u64) -> Result<Option<LastTestResult>, String> {
    db::get_last_test_result(&db, platform_id).await
}
```
注册到 `invoke_handler`（照既有 command 列表追加，保持字母/分组约定）。

### api.ts

```ts
export interface LastTestResult {
  success: boolean;
  status_code: number;
  duration_ms: number;
  created_at: number;
  error: string;
}
// platformApi 内:
lastTestResult: (id: number) => invoke<LastTestResult | null>("get_last_test_result", { platformId: id }),
```
> invoke 参数名 `platformId`（camelCase）须与 Rust 参数 `platform_id` 经 Tauri 默认 snake→camel 转换后一致；若项目他处显式传 snake_case，照既有 `platformApi.usageStats` 等邻居约定对齐。

### Dispatch Prompt

> ⛔ 写盘 sub-agent 派发时 MUST 带 `isolation: worktree`。

```
Active task: .trellis/tasks/06-19-platform-card-last-test-badge
# 派发参数: isolation: worktree

## 目标
后端新增 get_last_test_result 查询 + Tauri command + api.ts 封装（见 subtask 文件执行细节）。

## 已知
- model_test 已落 proxy_log：platform_id=真实id, source_protocol="test", 成功 status_code=200（lib.rs:1036-1041）
- 邻居参考：get_platform_usage_stats (db.rs:2789)、platformApi.usageStats (api.ts)、invoke_handler command 注册区 (lib.rs)
- Tauri command 参数 camelCase 转换约定，照既有 command 对齐

## 工作目录与范围
- cwd: worktree 根
- 可改文件: src-tauri/src/gateway/db.rs, src-tauri/src/gateway/models.rs, src-tauri/src/lib.rs, src/services/api.ts
- 禁改文件: **/dist/**, **/*.generated.*, .trellis/**, 其它前端组件（属 S2）

## 输出格式
- 类型: diff + 验证命令输出
- 行数上限: 验证输出截断至关键尾部

## 验收标准
cargo build / cargo clippy -- -D warnings / cargo test 全过（见 subtask 验证节）。

## 失败处理
- 工具瞬时错误 → 重试 1 次
- 业务阻塞 → 输出 `需要: <问题>` 并停
- 资源不可用 → 报 Blocked

## Sub-agent 自防护
你已是 trellis-implement，直接做，禁再 spawn。
```

## 回滚

- 触发条件: clippy/test 回归无法快速修
- 步骤: `git checkout -- <改动文件>` 或 worktree 整体丢弃

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| invoke 参数名 camelCase 不匹配致前端 undefined | 运行时静默失败 | 照 platformApi.usageStats 邻居约定对齐，S2 review 时核 |
| source_protocol 值大小写/拼写不一致查不到行 | 返 None 徽章不显示 | 以 lib.rs:875 `"test".into()` 实际值为准 |

## 历史
- 2026-06-19: created
