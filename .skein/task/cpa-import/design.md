# CPA 格式导入 + cpa-* 平台类型 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json)。

调研依据: [research/cpa-format.md](research/cpa-format.md) (格式) + [research/cpa-format-round2.md](research/cpa-format-round2.md) (4 cpa-* 上游 + adapter 映射)。

## 架构

### 三段式: 解析 → 映射 → 预览确认创建

```
用户在平台添加页点「导入 CPA 配置」
→ Tauri 文件选择 (单文件 / 压缩包 / 文件夹 [+ 可选 auth-dir 凭据目录])
→ 后端 cpa_import_parse ── 解压/扫 + 识别 + 合并去重 + 映射 Protocol
→ 前端预览 UI (表格: 多选 / 改名 / 选模型 / 冲突检测 / 惰性余额查询)
→ 用户确认选中集
→ apply: 逐个 platform_create (非原子尽力, 返回 created/failed)
```

### 后端模块 (新增 `aidog_core/src/gateway/cpa_import/{parser,mapper}.rs` + `commands_platform/src/cpa_import.rs`)

**解析器** (`parser.rs`):
- 输入: 文件系统路径 (String) + 可选 auth-dir (Option<String>)。
- 判类型: 后缀 `.yaml`/`.yml`/`.json` → 单文件; `.zip`/`.tar.gz`/`.tgz`/`.tar` → 解压到 tempdir (`zip`+`tar`+`flate2`); 目录 → 递归; `.rar`/`.7z` → **首版不支持, UI 提示先解压** (YAGNI, 见取舍)。
- 候选文件筛选: 扩展名 yaml/yml/json 全收, 逐个 `serde_yaml`/`serde_json` 尝试解析 (失败跳过)。
- 识别: Value 含 6 cpa 段顶层 key 任一 (`gemini-api-key`/`codex-api-key`/`claude-api-key`/`openai-compatibility`/`vertex-api-key`/`interactions-api-key`) → 留; 否则跳过。
- 提取: 6 段每条目 → `CpaProvider { source_segment, name, base_url, api_key, models, prefix, headers, disabled }` (字段表见 research/cpa-format.md)。
- auth-dir 扫描 (可选): 递归扫 `.json`, 解析 `{type, email, access_token, refresh_token, model-aliases}`, 按 `type` (codex/claude/kimi/xai/vertex/aistudio/antigravity) 生成 OAuth 类 `CpaProvider` (api_key = access_token, refresh_token 丢弃)。
- 合并: 跨文件 `name`(openai-compat) 或 `source_segment + base_url`(api-key 段) 去重 — 保留首个, api_key 取首, models 并集。
- 输出: `ParseResult { providers: Vec<MappedPlatform>, skipped: Vec<SkipReason>, source_files: Vec<String> }`。

**映射器** (`mapper.rs`): `CpaProvider` → `MappedPlatform { protocol, name, base_url, api_key, models, extra }`。

| 来源 | 路由 | Protocol | adapter |
|---|---|---|---|
| `gemini-api-key` / `interactions-api-key` | 直映 | `gemini` | gemini |
| `codex-api-key` | 直映 | `codex` | codex |
| `claude-api-key` | 直映 | `anthropic` | anthropic |
| `vertex-api-key` | 直映 | `cpa-vertex` (新) | gemini (待 s2 验证 Vertex URL 结构) |
| `openai-compatibility` (name 含 glm/kimi/minimax/deepseek/qwen/openrouter/...) | 关键词 | 对应 Protocol | openai_completions |
| `openai-compatibility` (无匹配) | 兜底 | `openai` | openai_completions |
| OAuth `xai` | 凭据 type | `cpa-grok` (新) | openai_responses |
| OAuth `vertex` | 凭据 type | `cpa-vertex` (新) | gemini |
| OAuth `aistudio` | 凭据 type | `cpa-aistudio` (新) | gemini |
| OAuth `antigravity` | 凭据 type | `cpa-antigravity` (新) | gemini |
| OAuth `claude`/`codex`/`kimi` | 凭据 type | `anthropic`/`codex`/`kimi` | 各 |

- **adapter 映射基于二轮调研** (research/cpa-format-round2.md): 4 cpa-* 走原生 API (非 OpenAI 兼容), 不复用 openai adapter。grok 走 `/responses` → `openai_responses`; aistudio/antigravity/vertex 走 `generateContent` → `gemini`。
- **openai-compatibility 不转 cpa-***: 该段一律 openai 兼容族 (name 关键词路由), cpa-* 仅来自 OAuth channel 凭据 / vertex-api-key 段。
- name 关键词表对照 aidog preset 75 协议逐一核 (s2, grep preset key 集, 无匹配兜底 `openai`)。
- 字段映射: `base-url` → `base_url` (cpa 通常含 `/v1`, 直用); `api-key`/`api-key-entries[0].api-key` (多 key 取首) → `api_key`; `models[].name` → `models` (alias 丢, 备 `extra.cpa_aliases`); `prefix`/`headers` → `extra` (路由不读, 存档); `disabled=true` → status=disabled。

**Tauri command** (`commands_platform/src/cpa_import.rs`):
```rust
#[tauri::command]
async fn cpa_import_parse(path: String, auth_dir: Option<String>) -> Result<ParseResult, String>
// 解析 + 映射 + 合并去重, 返 MappedPlatform[] + skipped[]。纯读, 不建平台。

#[tauri::command]
async fn cpa_import_preview_quota(base_url: String, api_key: String) -> Result<PlatformQuota, String>
// 临时查余额不落库: query_quota(db, &base_url, &api_key, 0)。platform_id=0 → persist_quota_to_db None-guard return。仅 9 provider 有值, 余显「—」。

#[tauri::command]
async fn cpa_import_apply(platforms: Vec<CreatePlatformInput>) -> Result<BatchReport, String>
// 逐个 platform_create (现有 command 复用)。非原子: 尽力创建, 返 created/failed[]。
```
注册 `generate_handler!` — **新 command 需 yarn tauri dev 重启** (memory `tauri-rust-command-needs-restart`)。

### 新 4 Protocol 接线 (s2)
- Rust: `Protocol` 枚举加 `CpaGrok`/`CpaVertex`/`CpaAistudio`/`CpaAntigravity` (serde `cpa-grok`/`cpa-vertex`/`cpa-aistudio`/`cpa-antigravity`), `provider_api_path()` 返对应路径 (grok→`/responses` 复 codex 模式; aistudio/antigravity/vertex→generateContent 模式, 具体路径 s2 定)。
- preset: `platform-presets.json` 加 4 条, **每协议 MUST 配默认 model_list (对应上游 web 端模型列表)**: cpa-grok(grok-4/grok-code-fast 等), cpa-aistudio(同 gemini 模型集), cpa-antigravity(claude-*/gemini-3-pro 等), cpa-vertex(Vertex 公开模型如 gemini-2.5-pro, **不留空**)。vertex base_url region-specific, preset base_url 留空但 model_list 配默认; 预览时用户补 base_url。
- 前端: `PROTOCOLS` 加 4 独立 `value`; i18n label。
- 路由层: 4 协议 `platform_type` 走对应 adapter (openai_responses / gemini)。
- **手维护 preset, 禁机器覆盖** (CLAUDE.md 约束)。

### 前端 (新增 `src/components/platforms/CpaImportModal.tsx`)
仿 CcSwitchImport (检测 → 读取 → 预览 → 多选 → 冲突预览 → apply):
- 入口: 平台添加页「导入 CPA 配置」文字按钮 → modal (createPortal document.body, 遵 modal-window-center-rule)。
- 步骤 1 选源: Tauri `open` dialog (文件/压缩包/文件夹); 可选第二 dialog 选 auth-dir。
- 步骤 2 预览: 调 `cpa_import_parse` → 表格。
  - 列: ☑ | 名称(可编辑) | 协议(badge) | base_url | api_key(掩码) | 模型(可编辑, 默认 `getDefaultModels(protocol)`) | 余额(占位「—」) | 冲突(同名/同 base_url 已存 → 黄警告)。
  - 工具栏: [全部查询余额] [全选] [取消] 已选 N | [确认创建]。余额惰性: 并发 `cpa_import_preview_quota` ≤5, 失败/不支持显「—」。
- 步骤 3 apply: `cpa_import_apply` → toast (created N / failed M + 原因) + 刷新平台列表 + 关 modal。

## 数据流

```
[选源] config.yaml/压缩/文件夹 (+ 可选 auth-dir)
  ↓ cpa_import_parse
[解析] 解压/扫 → 全文件尝试 → 识别 cpa 段 → CpaProvider[] → name+base_url 去重合并
  ↓ map_to_aidog
[映射] segment/channel → Protocol (openai-compat name 路由; vertex→cpa-vertex; OAuth→cpa-*)
  ↓ MappedPlatform[]
[预览] 表格 + 冲突检测 + 惰性余额 (cpa_import_preview_quota, 不落库)
  ↓ 用户多选 + 改名 + 选模型 + 确认
[apply] cpa_import_apply → 逐个 platform_create (非原子尽力) → BatchReport
  ↓
toast + 刷新 + 关 modal
```

## 关键取舍

### adapter 映射: 复用现有 (非新写 adapter)
4 cpa-* 走原生 API (research/cpa-format-round2.md 颠覆 PRD 原推测)。**复用现有 adapter**: grok→`openai_responses`, aistudio/antigravity/vertex→`gemini`。代价: antigravity 路径 `/v1internal:*` 与 vertex URL 结构需 s2 验证 adapter 兼容, 不符则该协议「仅存配置, 路由暂不支持」标注。不新写 adapter (重, YAGNI; 4 协议 API 与现有 adapter 同族)。

### 压缩格式: 首版仅 zip/tgz/tar
`zip`+`tar`+`flate2` 成熟轻量。rar/7z 需 `unrar`+`sevenz-rust` (重, 编译复杂)。**首版砍 rar/7z, UI 明示「rar/7z 请先解压」**。yaml 居多场景, 压缩是附赠。

### OAuth 凭据: 可选扫描 auth-dir
config.yaml `oauth-model-alias` 是别名映射非凭据; 凭据在 auth-dir JSON (type/access_token/refresh_token)。**导入源额外让用户可选指定 auth-dir 目录**, 扫内 JSON 按 type 创建 cpa-* OAuth 平台 (access_token 当 api_key, refresh 丢弃, 过期手补)。仅 config.yaml 不指定 auth-dir → 不创 OAuth 平台 (只创 api-key 段)。

### openai-compatibility 不转 cpa-*
该段一律 openai 兼容族 (name 路由 glm/kimi/.../openai 兜底)。cpa-grok/cpa-aistudio/cpa-antigravity 仅来自 OAuth channel; cpa-vertex 来自 vertex-api-key 段或 OAuth vertex。来源语义清晰。

### 余额查询: 惰性
预览可能几十平台, 自动批量并发上游 API 慢 + 可能限流。「全部查询余额」按钮 (并发 ≤5) + 单行点查; 失败/不支持显「—」不阻塞。复用 `query_quota(... platform_id=0)` (None-guard 自动不落库)。仅 9 provider 支持 (DeepSeek/OpenRouter/GLM/Kimi/MiniMax/NewAPI/SiliconFlow/StepFun/Novita), cpa-* + 其他显「—」(CPA 无内置余额查询, research 确认)。

### apply: 非原子尽力
与 group-batch-ops 原子事务不同 (那是改已有): cpa 导入是新建独立平台, 一个失败不该拖全部。返 `BatchReport { created, failed[] }`, toast 失败原因, 成功的已入库。

### name+base_url 去重 + 多 key 取首
跨文件合并: 同 name / 同 segment+base_url 保留首个, api_key 取首, models 并集。`api-key-entries[]` 多 key 取首个 (拆多平台 = 后续迭代)。不与 aidog 已有平台去重 (冲突预览黄警告 + apply DB unique 拦)。

## 技术选型

- YAML: `serde_yaml` (aidog 现无 YAML 依赖, 需加; 轻 ~50KB)。
- 压缩: `zip` + `tar` + `flate2` (首版); rar/7z 砍。
- 余额: 复用 `gateway::quota::query_quota` (platform_id=0), 零新增。
- 平台创建: 复用现有 `platform_create` (apply 逐个调)。
- 冲突检测: 预览时 `list_platforms` 比对 name + base_url。
- 前端 modal: 复用 `components/shared/Modal.tsx` (b4 基元, createPortal 已处理 liquid glass 居中)。

## 风险

1. **adapter 兼容** (s2 验证): antigravity `/v1internal:*` + vertex URL 含 project/location — gemini adapter 是否兼容? 不符则协议「仅存配置, 路由暂不支持」标注, 不阻塞导入。
2. **serde_yaml 新依赖**: 增编译 + bundle ~50KB, 可接受。
3. **cpa-vertex preset base_url 留空 / model_list 配默认**: Vertex region-specific base_url 用户预览补全; model_list 配 Vertex 公开模型 (gemini-2.5-pro 等) 不留空 (用户要求每协议有默认模型)。
4. **临时余额不落库验证**: `query_quota(... 0)` + `persist_quota_to_db` None-guard (quota.rs:55-56 `let Some(pid) = platform_id else { return }`), 确认。
5. **auth-dir JSON 格式**: 完整字段待读 `sdk/cliproxy/auth/store.go`; s1 按已知字段 (type/email/access_token/refresh_token/model-aliases) 实现, 缺字段容错。
6. **新 Rust command 需重启 dev** (memory) → 验收先重启。

## 决策已定 (AskUserQuestion 拍板 2026-07-13)

- ✅ adapter: 复用现有 (grok→openai_responses, aistudio/antigravity/vertex→gemini)。不新写 adapter。
- ✅ 压缩: 首版仅 zip/tgz/tar, rar/7z 砍 (UI 提示先解压)。
- ✅ OAuth: 可选扫描 auth-dir (用户指定目录, 扫 JSON 按 type 创建 cpa-* OAuth 平台)。
- ✅ apply: 非原子尽力 (返 created/failed 报告)。
- ✅ 多 api-key: 取首个 (拆多平台 = 后续迭代, 范围外)。
