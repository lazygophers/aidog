# CPA 后端落点 (A)

## A1. cpa_import 模块 (`src-tauri/crates/aidog_core/src/gateway/cpa_import/`)

**模块导出** (`mod.rs:1-14`): pub re-export `map_provider` / `map_providers` / `MappedPlatform` (from mapper.rs) + `parse_cpa_config` / `CpaOAuthType` / `CpaProvider` / `ParseResult` / `SkipReason` (from parser.rs)。

**gateway/mod.rs:3**: `pub mod cpa_import;` (顶层 gateway 下挂)。

### parser.rs (41.7K, ~960 行)
- `CpaSourceSegment` enum (line 19-34): 7 变体 = config.yaml 顶层 key (gemini-api-key / interactions-api-key / codex-api-key / claude-api-key / openai-compatibility / vertex-api-key) + OAuth (auth-dir JSON 衍生)。
- `CpaOAuthType` enum (line 66-74): 7 变体 (Codex/Claude/Kimi/Xai/Vertex/Aistudio/Antigravity)。`default_base_url()` (line ~95) 给 Xai/Aistudio/Antigravity 返静态 base_url；Vertex 留空 (region-specific)；Codex/Claude/Kimi 留空 (非 OAuth 上游路由)。
- `CpaProvider` 结构: 含 source_segment / oauth_type / name / base_url / api_key / models / prefix / headers / disabled 等字段。
- `parse_cpa_config(path, auth_dir)`: 支持单文件 yaml/json、压缩包 zip/tgz/tar、文件夹递归、可选 auth-dir OAuth 凭据扫描、多文件 name+base_url 去重合并。注：dedup key 修复 (OAuth 段按 name/email 非 base_url, 见 memory `dedup-key-empty-field`)。
- `SkipReason`: 被跳过文件原因 (rar/7z、解析失败、无 cpa 段)。

### mapper.rs (14.2K)
- `MappedPlatform` struct (line 29-50): protocol (Protocol) / name / base_url / api_key / models / extra (序列化 JSON, 含 cpa_prefix/headers/源信息) / disabled / source_label。直接对应 `CreatePlatform` 字段。
- `resolve_protocol` (line 78-98): 段→Protocol 路由表。
  - GeminiApiKey / InteractionsApiKey → `Gemini`
  - CodexApiKey → `Codex`; ClaudeApiKey → `Anthropic`
  - VertexApiKey → `CpaVertex`
  - OAuth: Xai→CpaGrok, Vertex→CpaVertex, Aistudio→CpaAistudio, Antigravity→CpaAntigravity, Claude→Anthropic, Codex→Codex, Kimi→Kimi, None→OpenAI
  - OpenaiCompatibility → `protocol_for_openai_compat_name` 关键词表 (line 103+, 手维护, 与 presets 68 协议 key 对齐, 兜底 OpenAI)
- 测试 line 268/305-308: 段/oauth → Protocol 映射表断言。

## A2. Tauri command (`src-tauri/crates/commands_platform/src/cpa_import.rs`, 175 行)

**lib.rs:15**: `pub mod cpa_import;`。

**startup.rs:229-231**: 注册 3 个 command 到 invoke_handler。
- `cpa_import_parse(path, auth_dir)` (line 44-56): 纯读, 解析+映射 → `CpaImportParseResult { platforms: Vec<MappedPlatform>, skipped, source_files }`。
- `cpa_import_preview_quota(base_url, api_key, db)` (line 65-78): 临时查余额不落库 (platform_id=0 绕过 persist)。9 provider 支持, cpa-* 平台本身不查 quota。
- `cpa_import_apply(platforms, db)` (line 86-148): 逐个 `create_platform` 非原子, disabled 条目 post-create UpdatePlatform 置 status=Disabled。`auto_group: Some(true)` 触发自动分组。

辅助 struct: `CpaImportParseResult` / `CpaBatchReport` / `CpaBatchFailure`。

## A3. cpa-* Protocol 变体 (Rust)

**定义** `src-tauri/crates/aidog_core/src/gateway/models/protocol.rs:158-174`, 共 **4 变体**:

| 变体 | serde rename | wire (实际请求格式) | 备注 |
|---|---|---|---|
| `CpaGrok` | `cpa-grok` | openai_responses | xAI Grok 原生 /responses |
| `CpaAistudio` | `cpa-aistudio` | gemini | Google AI Studio generateContent |
| `CpaAntigravity` | `cpa-antigravity` | gemini (占位, 路由不支持) | 路径 /v1internal:* 不兼容 |
| `CpaVertex` | `cpa-vertex` | gemini (占位, 路由不支持) | URL 含 projects/locations 结构不兼容 |

注释 (line 159-160): "4 协议均无独立 Rust adapter, endpoints[].protocol 决定 wire format, platform_type 仅作平台标识"。

测试 `test_protocol_coding_variants` (line 224-228) 含 4 cpa-* roundtrip。

## A4. 路由 / converter 特判分支

**仅一处**: `src-tauri/crates/aidog_core/src/gateway/adapter/converter/request.rs`:
- `convert_request` (line 38-54): 4 个 cpa-* 各有 match arm 作为 wire 回退 (无 endpoint 匹配时)。CpaGrok → openai_responses body + /v1/responses path; CpaAistudio|Antigravity|Vertex → gemini body + /v1beta path。
- `passthrough_api_path` (line 83-86): 同 4 arm, 透传路径占位。
- 注释明确: "正常路径下 endpoint[].protocol 已显式声明同族 wire, 不会落到这里" → 即 cpa-* 平台的 endpoint preset 都显式声明 protocol (openai_responses/gemini), platform_type 仅作标识, converter 这些 arm 实质是 fallback 兜底。

**router/ 目录** (candidates.rs / selection.rs / ordering.rs / mod.rs / model_mapping.rs): grep 0 命中 cpa, **无任何 cpa 特判**。路由层按 endpoint.protocol 选 wire, 与 platform_type 无关。

**proxy/ 目录** (handler/forward/connect/endpoint/stream/...): grep 0 命中 cpa, **无任何 cpa 特判**。

## A5. DB schema (无 cpa 专属列)

**platform 表** (`schema_early.rs:13-60` + 后续 ALTER 增列, 完整列见 `platform.rs:6`):
- `platform_type TEXT NOT NULL DEFAULT ''` (line 17): 存 Protocol 的 serde JSON 字符串 (如 `"cpa-grok"` 带引号, 见 `platform.rs:90` `serde_json::to_string`)。**无 cpa 专属列**。
- extra TEXT 列存序列化 JSON, cpa 平台的 prefix/headers/cpa_prefix 源信息塞这里 (mapper.rs build_extra)。

**proxy_log 表** (`schema_early.rs:76-103`): **完全无 cpa 专属字段**。关联靠 `platform_id INTEGER` (→ platform 表) + `group_name TEXT` + `target_protocol TEXT`。cpa-* 平台请求落 proxy_log 时 target_protocol = endpoint 的 wire (openai_responses/gemini), 非 cpa-grok 字面量。

**无独立 cpa 表**: 全部 cpa 数据复用 platform 表 + proxy_log 表。

## A6. 统计 / pricing / peak_hours / model_test 对 cpa-*

- **group usage stats** (`db/group.rs::get_group_usage_stats`): 按 `proxy_log.group_name` 聚合, **无 cpa 特判** (grep 0)。cpa 平台被加入 group 后统计照常计。
- **pricing** (`db/model_price.rs:177-194`): `resolve_price(model_name, platform_type)` 优先级 `pricing[platform_type] > top_level > default_platform`, cpa-* 作为 platform_type 可命中 pricing 节点, 但 LiteLLM 价格表无 cpa-* key → 走 default_platform fallback。
- **peak_hours**: grep 0 命中 cpa, platform-presets.json 的 4 cpa-* 条目均无 peak_hours 子节点。
- **model_test**: 走通用 endpoint 路径, 无 cpa 特判。

**新模块设计关键约束**: cpa-* 本质是"复用现有 adapter (openai_responses/gemini) + 平台标识", 无独立 wire。删 cpa-* 协议后, converter request.rs 的 6 个 arm (convert_request 2 arm + passthrough_api_path 2 arm 分 4 变体) + protocol.rs 4 变体 + presets 4 条目是硬落点; DB 无 schema 改动需求 (新独立表另算)。
