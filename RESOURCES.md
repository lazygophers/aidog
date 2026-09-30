# RESOURCES

aidog 路由机制教学资源。全部为一手来源（第 4 类：本项目源码，问题即「本项目怎么做的」故升最高优先）。

## 源码真值源

| 主题 | 位置 | 说明 |
|---|---|---|
| 路由主流程（阶段 0~6） | `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs` | select_candidates_ctx，每阶段带中文注释 |
| 单平台短路 / 闸门判定 | `src-tauri/crates/aidog_core/src/gateway/router/mod.rs` | candidate_state、sole_platform、effective_weight |
| 五种排序策略 | `src-tauri/crates/aidog_core/src/gateway/router/ordering.rs` | coding plan 上浮、快过期先用、粘性绑定 |
| 模型槽位匹配 | `src-tauri/crates/aidog_core/src/gateway/router/model_mapping.rs` | opus/sonnet/haiku/gpt 包含匹配 |
| RoutingMode 枚举定义 | `src-tauri/crates/aidog_db/src/models/protocol.rs:291-305` | 5 值 serde 名 |
| 熔断三态 + 失败分类 | `src-tauri/crates/aidog_core/src/gateway/scheduling.rs` | 文件头注释是最完整的失败分类表 |
| 现有中文文档（极简） | `docs/docs/zh/core-concepts/groups-routing.mdx` | 仅 10 行概述，不足以当讲义 |

## 测试文件（机制行为的事实补充）

`router/test_candidates.rs`（1484 行）、`test_ordering.rs`、`test_mod.rs`——每种边界行为（单平台 bypass、熔断全空回退、peak 审计落库）都有对应测试名可引用。
