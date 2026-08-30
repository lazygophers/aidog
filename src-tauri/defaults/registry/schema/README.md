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

除 schema 外，脚本还检查 **models 目录/文件缺失**（`make lint` 门禁）：

1. **index 清单零差集（硬错）**：`index.json` 声明的 `models[]` 文件必须存在于 `models_dir`；磁盘上多出的未登记文件也是错。
2. **引用完整性**：`platform.json` 里 `models.*` 分支值 + `model_list.*` 条目引用的每个 model id 必须有对应 `models/<id>.json`（vendor 子目录路径 = id）。平台已带 models 目录（自建价目）时为**硬错**；完全没目录的中转平台降为 warning——`AIDOG_REGISTRY_STRICT=1` 时全部按错处理。

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
| `last_updated` | 内容变更时间 Unix 秒（每文件必填）。远程同步内容比较的审计元数据；改内容必跑 `scripts/bump-registry-last-updated.mjs` 盖戳并推高 index.json 全局值 |
| `client_type` | 前端协议选择器 / Rust preset 解析 |
| `endpoints.default[]`（protocol / base_url / client_type?） | `src/domains/platforms/defaults.ts`（getDefaultEndpoints，缺省 client_type 按 protocol 派生 clientTypeForProtocol）、Rust `registry.rs::endpoints_in`（同派生 derive_client_type）；URL = base_url + provider_api_path，禁额外拼接 |
| `models.default` / `models.peak` | `defaults.ts::pickModelsBranch`（两分支）、`gateway/router/candidates.rs::resolve_effective_models`（peak 三层级联） |
| `model_list.default` | `defaults.ts::pickBranch`（下拉冷启动候选） |
| `peak[]` | Rust `gateway/peak.rs`（resolve_multiplier / is_in_peak_window）、前端 `utils/timeWindow.ts`（isCurrentlyPeak，cross-layer 对称） |
| `is_coding_plan` | Rust `gateway/router/ordering.rs`（coding 平台排序靠后）、前端 `defaults.ts::isCodingPlanProtocol` / ProtocolOption.codingPlan（徽标 / 选择器） |
| `key_prefixes` | 前端 `platformPaste.ts::collectKeyPrefixes` + 优先级 2 平台直判（粘贴识别 key 提取正则与平台判定数据驱动；平台前缀禁在代码硬编码，通用 `sk-`/`sk_` 除外。coding 套餐平台是独立协议，其专属 token 前缀（tp- / sk-cp-）写在本字段，无独立 coding 前缀字段） |
| `name`（8 locale 全必填） | 前端 `useProtocolMeta` / 协议选择器 label |
| `source_urls.docs` / `.pricing` | 前端模型信息页 / 官方价核对入口 |
| `homepage` | 前端平台卡片外链 + logo fallback（favicon） |
| `logo_url` | Rust `gateway/logo_sync.rs` 三路拉取（simpleicons slug → favicon → clearbit），本项目不存储任何平台 logo |
| `keywords` | 前端 `platformPaste.ts` 粘贴匹配 + 拼音搜索 |
| `color` | 前端徽标 / 卡片品牌色（hex） |

### model.json

| 字段 | 消费方 |
|---|---|
| `last_updated` | 内容变更时间 Unix 秒（每文件必填）。改内容必跑 `scripts/bump-registry-last-updated.mjs` 盖戳并推高 index.json 全局值 |
| `model_id` | 平台真实请求名；DB 主键一半（platform_code + model_id，ADR 0005）；须与文件名一致 |
| `canonical_model` | 跨平台聚合键（`model_entry.rs` 缺省回落 model_id） |
| `display_name` | 展示名，读取层回落 model_id（票 T10，`model_entry.rs::with_display_name`） |
| `thinking_supported` / `thinking_toggleable` | 【可选 bool】是否支持 thinking / thinking 是否可由请求参数开关（false = 强制思考）。缺省=未标注。消费方：前端模型详情弹窗（`priceData.ts::parseEntryFlags`）；数据填充随 models 清单审计批次逐平台查证 |
| `family` / `version` / `predecessor` | DB 列 `model_entry`（`MODEL_ENTRY_COLUMNS`），模型信息页前后代链 |
| `capabilities` | 前端 ModelInfo 页 `CapabilityBadges`。理解类：text（必含）/ vision / tool_use / reasoning / audio / video / embedding；生成类（输入→输出细分）：text_to_image / image_to_image / image_edit / text_to_video / image_to_video / video_to_video / video_edit |
| `builtin_tools_excluded` | DB 列；内置工具过滤 |
| `max_input_tokens` / `max_output_tokens` / `context_window` | 估算 / 请求裁剪（`gateway/estimate`）、模型信息页 |
| `official` | 模型维度列表默认展示官方条目（不变量：每 model_id 至少一条 official=true） |
| `price` | 计价结构子树（2026-08-30 收归）：`input`（必填）/ `output` / `cache_read` / `cache_write`（$/token）+ `peak`（高峰**绝对价**：命中平台 peak 窗口且条目带 peak → 用绝对价，平台倍率压成 1.0，禁双重计价，`price_resolve.rs`）+ `context_tiers[]`（按上下文长度分档）+ `time_tiers[]`（按生效时间切换价目，start_at latest wins；档内可嵌 `context_tiers`，时间 × 上下文二维）。全数值字段，禁字符串表达式。旧顶层平铺价与 `default_price` 死字段已删；DB 未重同步的旧行由读取层归一化（`price_resolve::legacy_price_into` / `model_entry::ui_entry`） |

## 加新字段流程

1. 确认 app 代码消费点（Rust / TS）已写好适配；
2. 改对应 schema（`additionalProperties: false` 会挡住未登记字段）；
3. 写数据文件；
4. `yarn check:registry` + `cd src-tauri && cargo test`（跨文件不变量在 test_registry.rs）；
5. 更新本文档映射表。
