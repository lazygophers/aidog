# 删 cpa-* 影响面 (C)

## C1. platform-presets.json 删除清单

`src-tauri/defaults/platform-presets.json` 4 条目 (line 1112-1175):
- `cpa-grok` (line 1112): endpoint openai_responses https://api.x.ai, models grok-*, client_type codex_tui。
- `cpa-aistudio` (line 1128): endpoint gemini https://generativelanguage.googleapis.com, models gemini-*。
- `cpa-antigravity` (line 1144): endpoint gemini https://cloudcode-pa.googleapis.com, models gemini/claude 混。
- `cpa-vertex` (line 1160): endpoint gemini base_url="" (用户填), models gemini-*。

删后: 前端 PROTOCOLS 下拉 / getDefaultEndpoints / getProtocolLabelMap 自动不含 cpa-* (动态派生, 见 B5)。app data `~/.aidog/platform-presets.json` 若用户本地已拷贝旧版, 需考虑回退 bundled 的时机 (defaults.rs::get_defaults_json 读 app data → 损坏回退 bundled, 但"已含 cpa-* 的有效旧版"不会自动回退, 推测: 需 migration 或版本比对)。

## C2. Protocol enum 删除影响面

Rust `Protocol` enum (`protocol.rs:162-174`) 删 4 变体后波及:

| 位置 | 影响 |
|---|---|
| `protocol.rs:224-228` test roundtrip | 删 4 case |
| `converter/request.rs:38-54` convert_request | 删 CpaGrok arm + CpaAistudio\|Antigravity\|Vertex arm |
| `converter/request.rs:83-86` passthrough_api_path | 同上删 2 arm |
| `converter/test_request.rs:221-243` | 删 cpa 相关 test |
| `cpa_import/mapper.rs:83,88-91,268,305-308` | resolve_protocol 映射删 cpa 分支 (整 cpa_import 模块删则连根拔) |
| `cpa_import/parser.rs:95` default_base_url 注释引用 | 整模块删 |

serde 兼容性: **旧库 platform 表里 `platform_type = "cpa-grok"` 的行, 删 enum 变体后 `serde_json::from_str` 会失败** → `platform.rs:17` `unwrap()` **panic 风险**。必须先做数据迁移 (C3) 才能删 enum。

## C3. 已入库 cpa-* 平台数据处置 (关键决策点, 需用户裁)

**现状**: 用户已通过 CpaImportModal 创建的 cpa-* 平台记录在 `platform` 表, `platform_type` 存 `"cpa-grok"` 等 JSON 字符串 (platform.rs:90)。proxy_log 历史请求通过 `platform_id` 关联这些平台。

**选项** (不拍板, 列权衡):

1. **删除 cpa-* 平台行 (软删 deleted_at 或硬删)**
   - 优: 最干净, 统计立即不含 cpa。
   - 劣: proxy_log 历史记录的 platform_id 变孤儿 (platform_id 指向已删行); group_platform 关联断裂; 用户配置丢失。
2. **迁移到新独立模块表**
   - 优: 保留用户配置, 符合"独立数据表"需求。
   - 劣: 新模块表结构未定 (PRD TODO); cpa-grok/aistudio 的 OAuth token / base_url 字段映射需设计; antigravity/vertex 路由本就不支持, 迁移价值低。
3. **保留作历史, 仅从路由候选排除**
   - 优: proxy_log 统计连续。
   - 劣: 违背"整体干净统计"需求, 死数据残留。

**孤儿 proxy_log 风险** (任何选项都需考虑): 若删/改 platform_id, proxy_log.platform_id=旧id 的行聚合统计时 (query_stats.rs GROUP BY platform_id) 会落到"无平台"桶 (Stats.tsx:316 platform_id=0 隧道请求) 或查不到 platform name。stats_agg_hourly 表 (预聚合) 同样含 platform_id, 需同步处理或接受历史偏差。

## C4. 统计残留

- **group usage stats** (group.rs::get_group_usage_stats 按 group_name 聚合): cpa 平台曾在某 group → proxy_log.group_name 记录 → 删平台后历史 group_name 记录仍在, 统计仍含。需明确"干净统计"是否要求回溯清历史。
- **stats_agg_hourly** (预聚合表, stats_agg.rs): 含 platform_id 维度, 删平台后预聚合行不自动清。
- **model 维度统计**: cpa 平台请求的 model (grok-4 / gemini-*) 仍在 proxy_log.model, 按模型聚合不受 platform 删除影响。
- **pricing**: model_price 表无 cpa 专属条目, 无残留。

## C5. 关联处清理检查表

| 项 | 位置 | 删 cpa-* 后动作 |
|---|---|---|
| Tauri command 注册 | startup.rs:229-231 | 删 3 command 注册 |
| commands_platform mod | lib.rs:15 | 删 pub mod cpa_import |
| aidog_core gateway mod | gateway/mod.rs:3 | 删 pub mod cpa_import |
| cpa_import 目录 | aidog_core/src/gateway/cpa_import/ | 整目录删 (3 文件) |
| commands_platform/cpa_import.rs | | 整文件删 |
| 前端 | 见 cpa-frontend-footprint.md B6 清单 | |

## 关键约束 / 风险

1. **panic 风险**: 删 Protocol enum 变体前必须先迁移/清理 DB 里 `platform_type LIKE 'cpa-%'` 的行, 否则 `platform.rs:17` from_str unwrap panic。
2. **serde 兼容**: 若新模块仍用 Protocol enum 加新变体 (如 `internal-grok`), 旧 cpa-* 字符串反序列化失败同上, 需 migration 或 from_str 容错 (from_db_str 模式)。
3. **proxy_log 历史不可逆**: 已落库的 proxy_log 行 (含 platform_id 指向 cpa 平台) 删平台后成为孤儿, 统计聚合行为需明确 (推测: query_stats 对不存在的 platform_id 会显示空名或归"无平台")。
4. **新模块独立表 = 全新数据**, 与旧 cpa 平台数据无自动迁移路径, 用户需重新配置 (符合"完全推翻"需求, 但需向用户确认是否接受历史配置丢失)。

**需用户裁**: C3 数据迁移策略 (删 / 迁 / 留) + 是否要求回溯清 proxy_log/stats_agg 历史。
