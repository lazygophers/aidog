# Registry JSON Schema

`src-tauri/defaults/registry/` 数据的字段形状约束（JSON Schema draft-07）。registry 手维护、禁机器生成覆盖，schema 的作用是**抓手滑**：字段名拼错、类型写错、必填漏填，在提交前就报出来。

## 文件

| Schema | 约束对象 | 说明 |
|---|---|---|
| `index.schema.json` | `index.json` | 平台清单 + 远程同步逐文件拉取清单（漏登记 = 线上永远同步不下来） |
| `platform.schema.json` | `platforms/<code>/platform.json` | 平台预设（端点 / 模型映射 / 品牌 / 高峰窗口），65 个 |
| `model.schema.json` | `platforms/<code>/models/<model>.json` | 模型条目（能力 / 限额 / 计价分档），1010 条 |

三个 schema 均 `additionalProperties: false`：新字段必须先改 schema + 加代码适配，再写数据。这是「所有字段都在 app 代码里适配」的强制闸门——schema 里没有的字段写进 JSON 会直接校验失败。

## 校验

```bash
yarn check:registry
```

脚本 `scripts/check-registry.mjs` 用 ajv（devDependency）逐文件校验，支持 `AIDOG_REGISTRY_DIR` 环境变量指到 fixture 目录。与 Rust 侧 `aidog_db::test_registry` 的漂移断言互补：**那边锁清单一致性**（index 与磁盘零差集、每模型必有 official 条目等跨文件不变量），**这边锁单文件形状**。

## 目录结构约定

```
registry/
  index.json                       # 清单（schema/index.schema.json）
  schema/                          # 本目录；build.rs 不扫这里（只读 index.json + platforms/）
  platforms/<code>/
    platform.json                  # 平台预设（schema/platform.schema.json）
    models/<model>.json            # 模型条目（schema/model.schema.json）；vendor 子目录模型如 mistralai/codestral-2.json
```

## 字段 → 消费方映射

改字段前先看消费方；加新字段时同步改这里。

### platform.json

| 字段 | 消费方 |
|---|---|
| `client_type` | 前端协议选择器 / Rust preset 解析 |
| `endpoints.default[]`（protocol / base_url / client_type / coding_plan） | `src/domains/platforms/defaults.ts`（getDefaultEndpoints）、Rust endpoint 路由；URL = base_url + provider_api_path，禁额外拼接 |
| `models.default` / `models.peak` / `models.coding_plan` | `defaults.ts::pickModelsBranch`（三分支）、`gateway/router/candidates.rs::resolve_effective_models`（peak 三层级联） |
| `model_list.default` / `.coding_plan` | `defaults.ts::pickBranch`（下拉冷启动候选） |
| `peak_hours[]` | Rust `gateway/peak_hours.rs`（resolve_multiplier / is_in_peak_window）、前端 `utils/peakHours.ts`（isCurrentlyPeak，cross-layer 对称） |
| `is_coding_plan` | Rust `gateway/router/ordering.rs`（coding 平台排序靠后）、前端 platformPaste 机制 B |
| `codingKeyPrefixes` | 前端 `utils/platformPaste.ts`（粘贴分享帖时 API key 前缀命中即升级 coding 变体） |
| `name`（8 locale 全必填） | 前端 `useProtocolMeta` / 协议选择器 label |
| `source_urls.docs` / `.pricing` | 前端模型信息页 / 官方价核对入口 |
| `homepage` | 前端平台卡片外链 + logo fallback（favicon） |
| `logo_url` | Rust `gateway/logo_sync.rs` 三路拉取（simpleicons slug → favicon → clearbit），本项目不存储任何平台 logo |
| `keywords` | 前端 `platformPaste.ts` 粘贴匹配 + 拼音搜索 |
| `color` | 前端徽标 / 卡片品牌色（hex） |

### model.json

| 字段 | 消费方 |
|---|---|
| `model_id` | 平台真实请求名；DB 主键一半（platform_code + model_id，ADR 0005）；须与文件名一致 |
| `canonical_model` | 跨平台聚合键（`model_entry.rs` 缺省回落 model_id） |
| `display_name` | 展示名，读取层回落 model_id（票 T10，`model_entry.rs::with_display_name`） |
| `family` / `version` / `predecessor` | DB 列 `model_entry`（`MODEL_ENTRY_COLUMNS`），模型信息页前后代链 |
| `capabilities` | 前端 ModelInfo 页 `CapabilityBadges` |
| `builtin_tools_excluded` | DB 列；内置工具过滤 |
| `max_input_tokens` / `max_output_tokens` / `context_window` | 估算 / 请求裁剪（`gateway/estimate`）、模型信息页 |
| `official` | 模型维度列表默认展示官方条目（不变量：每 model_id 至少一条 official=true） |
| `input/output/cache_read/cache_creation_cost_per_token` | `aidog_db::resolve_price` 计费解析（ADR 0006 三级顺序） |
| `default_price` | 条目无独立价时的兜底价对象 |
| `peak` | 高峰**绝对价**：命中 peak_hours 窗口且条目带 peak → 用绝对价，平台倍率压成 1.0（禁双重计价，`price_resolve.rs`） |
| `context_tiers[]` | 按上下文长度分档计价 |
| `time_tiers[]` | 按生效时间切换价目（start_at，latest wins）；条目可内嵌 `context_tiers`（时间 × 上下文二维） |

## 加新字段流程

1. 确认 app 代码消费点（Rust / TS）已写好适配；
2. 改对应 schema（`additionalProperties: false` 会挡住未登记字段）；
3. 写数据文件；
4. `yarn check:registry` + `cd src-tauri && cargo test`（跨文件不变量在 test_registry.rs）；
5. 更新本文档映射表。
