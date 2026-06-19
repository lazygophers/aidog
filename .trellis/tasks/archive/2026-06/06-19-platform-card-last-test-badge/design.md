# Design — platform-card-last-test-badge

## 架构概览

```
[Groups 批量测试]  ──┐
[Platforms 快速测试] ──┼──> window CustomEvent "aidog-platform-test-completed" {platformId}
[ModelTestPanel]   ──┘                    │
                                          ▼
                          usePlatformCards 监听 → refreshLastTest(id)
                                          │
                                          ▼
                     platformApi.lastTestResult(id) → Tauri command
                                          │
                                          ▼
                db::get_last_test_result (proxy_log 最近一条 source_protocol='test')
                                          │
                                          ▼
                          lastTestMap[id] → PlatformCard 常驻徽章
```

数据源单一事实源 = `proxy_log`（model_test 已落），无新增持久化。

## 模块切分

| 模块 | 文件 | 执行层 | 独占资源 |
| --- | --- | --- | --- |
| 后端查询 | `db.rs` `models.rs` `lib.rs` | sub-agent (S1) | 这 3 rust 文件 + api.ts |
| TS 封装 | `api.ts` | sub-agent (S1) | api.ts（与 S2 共享，S1 先） |
| 前端徽章 | `PlatformCard.tsx` | sub-agent (S2) | 该 tsx |
| hook state/事件 | `usePlatformCards.ts` | sub-agent (S2) | 该 ts |
| Platforms 接线 | `Platforms.tsx` | sub-agent (S2) | 该 tsx |
| 测试触发点派发 | `Groups.tsx` `ModelTestPanel.tsx` | sub-agent (S2) | 这 2 tsx |

## 契约

- Tauri command `get_last_test_result(platform_id: u64) -> Option<LastTestResult>`
- `LastTestResult { success: bool, status_code: i32, duration_ms: i32, created_at: i64, error: String }`
- TS 同构（字段名一致，invoke camelCase `platformId`）
- 事件 `aidog-platform-test-completed` detail `{ platformId: number }`

## 取舍

- 选「新增独立徽章」而非「改 health 点语义」：health = 最近 5 次聚合（含真实流量），徽章 = 最近一次测试，语义不同，并存更准。
- 选 per-platform 事件而非批量事件：单卡精准刷新，避免整页重拉。
- 不新增持久化表：proxy_log 已是事实源，查询即得。

## 回滚形状

纯增量（新查询/command/类型/徽章/事件），无 schema 变更，revert commit 即净回滚。

## 风险

| 风险 | 缓解 |
| --- | --- |
| invoke 参数 camelCase 失配 | 照 usageStats 邻居对齐，S2 review 核 |
| 事件名两端拼写不一致 | 抽常量 |
| 新 i18n key 漏语言 | yarn check:i18n 把关 |
| 批量测试高频查询 | 本地查询量小，可接受；必要时 debounce |
