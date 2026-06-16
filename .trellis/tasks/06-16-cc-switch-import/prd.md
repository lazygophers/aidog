# 导入 cc-switch 平台配置

> 集成进 aidog 现有「导入导出」模块，支持选择性导入 cc-switch 的供应商配置。

## 背景 & 目标

cc-switch（github.com/farion1231/cc-switch）是被广泛使用的 Claude Code / Codex / Gemini 供应商切换工具。aidog 的平台预设最初就移植自 cc-switch 的 presets（见记忆 `platform-protocol-design`：「预设来源：cc-switch 的 presets JSON，已全部移植到 `getDefaultEndpoints()`」）。大量 aidog 目标用户的现有配置都存在 cc-switch 里。

**目标**：在 aidog 现有「导入导出」模块新增一个「从 cc-switch 导入」入口，读取 cc-switch 本地配置，按 `base_url` 自动适配到 aidog 平台预设，转换为 aidog 的 Platform（+ 可选模型映射 / 代理 / 密钥 / 自动更新设置），允许用户勾选要导入哪些供应商和哪些配置维度，逐项处理与现有 Platform 的冲突。

**非目标**：不做反向（aidog → cc-switch 导出）；不接管 cc-switch 的运行时切换；不导入 cc-switch 的 MCP / prompts / skills / 代理日志 / 健康状态 / 定价表（见 Out of Scope）。

---

## cc-switch 配置结构调研结论（带实证）

cc-switch 当前（main）已从 JSON 配置演进为 **SQLite 为 SSOT**，但 provider 的核心数据形态稳定，且仍保留旧 JSON 迁移路径。两种磁盘格式并存：

### 1. 存储位置与格式（实证：`docs/user-manual/zh/5-faq/5.1-config-files.md`）

- 数据目录默认 `~/.cc-switch/`，可在设置中自定义（云同步用）。
- 当前格式：`~/.cc-switch/cc-switch.db`（SQLite，SSOT）。
- 旧格式：`~/.cc-switch/config.json`（`MultiAppConfig`），仍有迁移逻辑（`src-tauri/src/database/migration.rs:3` 「将旧版 config.json (MultiAppConfig) 数据迁移到 SQLite」，`migrate_from_json` / `migrate_providers`）。
- 设备级 `~/.cc-switch/settings.json`（language/theme/autoStart/各 app configDir）——**不跨设备同步**，对应 aidog「控制的自动更新等」需求里的一部分。

### 2. providers 表（SSOT 核心，实证：`src-tauri/src/database/schema.rs:27-47`）

```
providers(
  id TEXT, app_type TEXT, name TEXT,
  settings_config TEXT NOT NULL,   -- ← 关键：目标 app 的配置 JSON/TOML 片段
  website_url TEXT, category TEXT, created_at INTEGER, sort_index INTEGER,
  notes TEXT, icon TEXT, icon_color TEXT, meta TEXT DEFAULT '{}',
  is_current BOOLEAN, in_failover_queue BOOLEAN,
  PRIMARY KEY (id, app_type)
)
provider_endpoints(provider_id, app_type, url, added_at)   -- 端点候选/测速列表
```

- **`app_type`** 取值：`claude` / `codex` / `gemini` / `opencode` / `hermes` / `openclaw` / `omo`（实证：`commands/config.rs` 的 `validate_common_config_snippet` 与 `app_config.rs` AppType）。aidog 只关心 `claude` 与 `codex`（+ 可选 `gemini`）。
- **`settings_config`** 是该供应商对应**目标 app 配置文件的片段**，形态因 app_type 而异（见下表）。
- `is_current` 标识当前激活供应商；`in_failover_queue` 是 cc-switch 的故障转移队列成员标识。

### 3. settings_config 形态（实证：preset 文件即真实写盘形态）

#### Claude 类（`src/config/claudeProviderPresets.ts`）

`ProviderPreset.settingsConfig` 就是写入 `~/.claude/settings.json` 的片段，核心是 `env`：

```jsonc
// 实证：claudeProviderPresets.ts Shengsuanyun 预设
{ "env": {
    "ANTHROPIC_BASE_URL": "https://router.shengsuanyun.com/api",
    "ANTHROPIC_AUTH_TOKEN": "",            // 或 ANTHROPIC_API_KEY（apiKeyField 决定）
    "ANTHROPIC_MODEL": "anthropic/claude-sonnet-4.6",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "anthropic/claude-haiku-4.5",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "anthropic/claude-sonnet-4.6",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "anthropic/claude-opus-4.8"
} }
```

preset 额外携带 `apiKeyField`（`ANTHROPIC_AUTH_TOKEN` | `ANTHROPIC_API_KEY`）、`apiFormat`（`anthropic` | `openai_chat` | `openai_responses` | `gemini_native`）、`category`、`endpointCandidates`。**实际用户的 settings_config 同形**（preset 只是初始模板）。

#### Codex 类（`src/config/codexProviderPresets.ts`）

cc-switch 把 Codex provider 拆成两部分（实证：`CodexProviderPreset` 字段 + `codex_config.rs` `write_codex_live_atomic` 写 auth.json + config.toml）：

```jsonc
// auth → ~/.codex/auth.json
{ "OPENAI_API_KEY": "sk-xxx" }
// config → ~/.codex/config.toml（TOML 字符串）
model_provider = "custom"
model = "gpt-5.5"
model_reasoning_effort = "high"
disable_response_storage = true
[model_providers.custom]
name = "<provider>"
base_url = "https://.../v1"
wire_api = "responses"            // ← 对应 aidog endpoint protocol openai_responses
requires_openai_auth = true
```

> SQLite 里 codex provider 的 `settings_config` 列存的是把 auth + config 聚合后的 JSON（cc-switch 内部用 serde 序列化 ProviderConfig），解析时需同时取出 `OPENAI_API_KEY`、`base_url`、`model`、`wire_api`。**实现期需用真实 cc-switch.db 样本核实该列的精确 JSON 结构**（见 Open Questions / `需要:`）。

### 4. cc-switch 自身导出格式（实证：`src-tauri/src/commands/import_export.rs`）

cc-switch 的「导出配置」是 **SQL dump**（`export_config_to_file` → `db.export_sql`，扩展名 `.sql`），导入是 `import_sql`。**这是整库覆盖式，不适合做 aidog 选择性导入的源**——因此 aidog 应直接读 cc-switch 的原始存储（DB 或 config.json），而非吃它的 .sql 导出。

### 字段映射表（cc-switch → aidog）

| cc-switch | aidog（Platform / Endpoint / Group） | 说明 |
|---|---|---|
| `providers.name` | `platform.name` | 平台名；冲突走逐项决策 |
| `providers.app_type=claude` | endpoint `protocol` + `client_type=claude_code` | Claude 类 |
| `providers.app_type=codex` | endpoint `protocol`(openai/openai_responses) + `client_type=codex_tui` | Codex 类 |
| `settingsConfig.env.ANTHROPIC_BASE_URL` | `platform.base_url` / `endpoint.base_url` | **base_url 适配主键** |
| `settingsConfig.env.ANTHROPIC_AUTH_TOKEN`/`_API_KEY` | `platform.api_key` | 密钥同步（需求 4） |
| codex `auth.OPENAI_API_KEY` | `platform.api_key` | 密钥同步 |
| codex `config.toml base_url` | `platform.base_url` / `endpoint.base_url` | base_url 适配 |
| codex `config.toml wire_api=responses` | endpoint `protocol=openai_responses` | 协议映射 |
| `settingsConfig.env.ANTHROPIC_MODEL` / `_DEFAULT_*_MODEL`；codex `config.model` | `platform.models`(PlatformModels 各 slot) | 模型映射（需求 2） |
| `provider_endpoints.url` | `platform.endpoints[].base_url`（多候选） | 多端点候选 |
| 经 base_url 匹配得到的 aidog preset | `platform.platform_type`(Protocol 枚举) | **平台类型识别（需求 1）** |
| `~/.cc-switch/settings.json autoStart` 等 | aidog setting scope（`app` 等）/ 不导入 | 自动更新等（需求 5，见 Open Q） |
| cc-switch `meta` / `category` / `icon` | （丢弃或入 platform extra，可选） | 非核心 |

---

## 需求逐条拆为 Deliverable

### D1. 平台设置：自动添加平台类型 + **识别平台类型**（需求 1）

- 读取 cc-switch 每个 provider，提取 base_url（claude 从 `env.ANTHROPIC_BASE_URL`，codex 从 config.toml `base_url`）。
- 用 **base_url 适配链**（见下节）确定 aidog `Protocol`（platform_type）。
- 生成 aidog Platform 行：`name`、`platform_type`、`base_url`、`endpoints`（含正确 `protocol` + `client_type` + `coding_plan`）。
- 复用 `Platforms.tsx` 的 `getDefaultEndpoints(protocol, codingPlan?)`（`src/pages/Platforms.tsx:150`）作为命中 preset 后的 endpoints 骨架，再用 cc-switch 实际 base_url 覆盖对应 endpoint。

### D2. 模型映射：添加模型映射（需求 2）

- Claude 类：`ANTHROPIC_MODEL` → 主模型；`ANTHROPIC_DEFAULT_HAIKU/SONNET/OPUS_MODEL` → 对应 PlatformModels slot（haiku/sonnet/opus）。
- Codex 类：config.toml `model` → 主模型 slot。
- 写入 aidog `platform.models`（`PlatformModels`，models.rs:235）；未提供的 slot 留空（向后兼容，由 fetchModels 兜底，见记忆 `platform-default-model`）。

### D3. 代理配置（需求 3）

- cc-switch 代理配置存于 DB `proxy_config` 表（app_type 维度：listen_address/port/timeout/熔断阈值等），与 aidog 的 ProxyLogSettings / breaker 字段**语义不完全对齐**。
- **MVP 决策（待确认）**：代理配置默认**不导入**（cc-switch 的 proxy_config 是它自己的本地代理服务器配置，aidog 有独立代理子系统，强行映射易错）。若用户需要，作为可勾选的「实验性」子项，仅映射可安全对应的字段（端口/监听地址）。详见 Open Questions。

### D4. 密钥同步（需求 4）

- Claude：`env.ANTHROPIC_AUTH_TOKEN` 或 `ANTHROPIC_API_KEY`（按 provider 的 `apiKeyField`）→ `platform.api_key`。
- Codex：`auth.OPENAI_API_KEY` → `platform.api_key`。
- 密钥为空字符串的 provider（cc-switch preset 模板）→ 仍导入平台但 api_key 留空，UI 提示需补填。
- 密钥不进日志（沿用现有 import_export 安全边界）。

### D5. 控制的自动更新等（需求 5）

- cc-switch 的 `~/.cc-switch/settings.json` 含 `autoStart` / `windowBehavior` / `theme` / `language` 等设备级设置。
- aidog 对应概念：开机自启、自动更新（version-cicd-updater 子系统）、主题、语言（setting scope=app）。
- **MVP 决策（待确认）**：仅迁移有明确对应的（如 language → aidog locale）；`autoStart` 映射到 aidog 自启设置（若存在对应能力）。cc-switch 特有的（windowBehavior、各 configDir）丢弃。详见 Open Questions。

---

## base_url → 平台匹配 + 回退链路设计（关键）

复用现有「智能粘贴」解析器的成熟逻辑（`src/utils/platformPaste.ts`，记忆 `platform-smart-paste-parser`），**不重复造轮子**：

```
对每个 cc-switch provider：
1. 取 base_url（claude: env.ANTHROPIC_BASE_URL；codex: config.toml base_url）。
2. 【preset 精确/关键词匹配】用 Platforms.tsx PROTOCOLS（含 keywords，:17-87）
   对 provider.name + base_url 做 normalizeForMatch + keyword substring 匹配
   （复用 platformPaste.ts matchPlatform(text, presets) :142）。
   命中 → platform_type = 该 preset.value；endpoints 取 getDefaultEndpoints()
   骨架并用实际 base_url 覆盖同协议 endpoint。
3. 【base_url host 匹配（增强）】若关键词没命中，把 base_url 的 host 与
   getDefaultEndpoints() 各 preset 的内置 base_url host 比对（如
   open.bigmodel.cn → glm、api.moonshot.cn → kimi）。命中 → 用该 preset。
4. 【协议回退（兜底）】仍无匹配：
   - app_type=claude（或 base_url 含 /anthropic）→ platform_type = "anthropic"
     （默认 claude 协议），endpoint { protocol:"anthropic", base_url, client_type:"claude_code" }
   - app_type=codex（或 wire_api=responses / base_url 含 /v1|openai）→
     platform_type = "openai"（默认 openai-response 协议*），
     endpoint { protocol:"openai_responses" 或 "openai", base_url, client_type:"codex_tui" }
   * 用户原话「codex 类用默认 openai-response 协议」：codex provider 的 config.toml
     wire_api="responses" → endpoint protocol = openai_responses；wire_api="chat" → openai。
```

协议倾向判断复用 `guessProtocol(url)`（platformPaste.ts:116：`/anthrop/`→anthropic、`/gemini|generativelanguage/`→gemini、`/openai|\/v1/`→openai）。

**实现位置抉择**（待 D 确认）：该匹配链是纯函数，**优先放前端**（与 platformPaste.ts 同址，复用 PROTOCOLS preset + getDefaultEndpoints），后端只负责读 cc-switch DB/JSON 并把 provider 原始字段交给前端转换。理由：preset 住前端（记忆 `aidog-add-platform-skill` 反直觉点 1），匹配逻辑必须能访问 preset，放后端要在 Rust 重复一份 preset → 双写腐化风险。

---

## 与现有「导入导出」模块的集成点

现有模块（`src-tauri/src/gateway/import_export/`，记忆 `import-export-module`）是 **`.aidogx` 对称导入导出**：导出自身 → 加密单文件 → 导入解密 + 逐项冲突 + 写 DB。cc-switch 导入是**异源单向导入**，形态不同（无加密容器、源是外部工具的 DB/JSON）。

**集成方案（推荐 Approach A：新增独立 import source，复用冲突/写入层）**：

1. **新增 Tauri command**（lib.rs）：
   - `ccswitch_detect()` → 探测 `~/.cc-switch/cc-switch.db` 或 `config.json` 是否存在 + 路径（含自定义目录探测）。
   - `ccswitch_read(path?)` → 读取并解析为「中间表示」`Vec<CcProvider>`（原始字段，不做平台匹配）。
   - `ccswitch_import(selection, decisions)` → 按用户选择 + 冲突决策写入。
2. **复用现有 apply 层**：把转换后的 Platform 行喂给现有 `apply.rs` 的 `upsert_platform_row` / 冲突检测（`detect_conflicts` 按 platform name）/ `ConflictItem` / `Decision{Overwrite,Skip,Rename}` 机制——**不另造一套冲突 UI**。
   - 即：cc-switch import 的后端把选中的 provider 转成 `Payload.platform`（+ 可选 models），再走现有 preview/apply 路径。
3. **不新增 `.aidogx` scope**：cc-switch import 是平行入口，不污染 export scope 列表（ALL_SCOPES 保持 7 项）。

**选择性导入两级粒度**：
- 级 1：**选哪些供应商**（provider 列表多选，默认全选；展示 name + 识别出的 platform_type + base_url + 是否冲突）。
- 级 2：**选哪些配置维度**（勾选框：平台类型✓ / 模型映射 / 密钥 / 代理 / 自动更新等——对应 D1–D5，默认勾选 D1+D2+D4，D3/D5 默认关）。

### 选择性导入 UI 方案

在 `src/components/settings/ImportExport.tsx`（AppSettings 的 importexport tab）现有「导出 / 导入」两块之外，新增第三块「从 cc-switch 导入」卡片：

1. 「检测 cc-switch」按钮 → 调 `ccswitch_detect`，显示找到的配置路径（或「未检测到，可手动选择目录」）。
2. provider 列表（级 1 多选）：每行 checkbox + 平台名 + 识别徽标（platform_type，匹配成功/回退用不同色，复用 StatChip / colorScale）+ base_url + 冲突标记。
3. 维度勾选条（级 2）：D1–D5 复选。
4. 「预览」→ 复用现有 conflict 弹窗逐项决策（`ConflictItem` / `setDecision`）。
5. 「导入」→ `ccswitch_import` → `ImportReport`（applied/skipped/errors）。
6. i18n：新增 `importExport.ccswitch.*` key，8 语言全补（沿用 check-i18n.mjs 防线，记忆 `frontend-i18n-coverage`）。

---

## MVP 边界与非目标

**MVP 包含**：
- 读取 cc-switch 当前 SQLite（`cc-switch.db` providers 表）+ 旧 `config.json` 两种源。
- app_type ∈ {claude, codex} 的 provider 导入（gemini 视情况，见 Open Q）。
- D1（平台类型识别 + base_url 适配回退链）、D2（模型映射）、D4（密钥同步）。
- 供应商多选 + 维度勾选 + 复用现有冲突逐项决策。

**Out of Scope（明确不做）**：
- aidog → cc-switch 反向导出。
- 导入 cc-switch 的 MCP servers / prompts / skills（aidog 有独立 MCP/Skills 模块，语义不同）。
- 导入 proxy_request_logs / provider_health / model_pricing / stream_check_logs。
- 接管 cc-switch 的 `is_current` 运行时激活态、`in_failover_queue` 故障转移队列。
- D3（代理配置）、D5（自动更新等）若确认语义不可靠对齐则降级为「不做 / 仅 language」。
- 吃 cc-switch 的 `.sql` 导出文件（直接读原始存储）。
- 加密 / `.aidogx` 容器（cc-switch import 是明文外部源，无需）。

---

## 验收标准

- [ ] 检测到本机/指定目录的 cc-switch 配置（DB 或 config.json），列出全部 claude/codex provider。
- [ ] 每个 provider 正确识别 platform_type：命中 preset 关键词或 base_url host → 对应 Protocol；未命中 → claude 类回退 anthropic、codex 类回退 openai(_responses)。回退链 4 步全覆盖且有单测。
- [ ] 模型映射：ANTHROPIC_MODEL / DEFAULT_*_MODEL / codex model 正确落 PlatformModels 对应 slot。
- [ ] 密钥：AUTH_TOKEN/API_KEY/OPENAI_API_KEY 正确落 platform.api_key；空密钥不报错。
- [ ] 选择性导入：能取消勾选个别 provider 与个别维度（D1–D5），结果只写勾选项。
- [ ] 冲突：与现有同名 Platform 冲突时走现有逐项 Overwrite/Skip/Rename 决策（复用 import_export 机制，不新造）。
- [ ] 集成点落到具体：新增 `ccswitch_*` command + 复用 `apply.rs` upsert/冲突；UI 在 ImportExport.tsx 第三块；不新增 export scope。
- [ ] `cargo clippy` / `cargo test` / tsc 零 warning；i18n 8 语言 check-i18n 零缺失。
- [ ] cc-switch 配置结构调研结论在 design.md / 代码注释中可追溯到仓库文件路径。

---

## Decision (ADR-lite)

**Context**：cc-switch 已 SQLite 化，自身导出是整库 SQL dump，不适合选择性导入；aidog 平台预设住前端、冲突/写入层在 import_export/apply.rs 已成熟。

**Decision**：cc-switch import 作为 import_export 模块的**第三个平行入口**（异源单向），后端新增 `ccswitch_*` command 读原始存储 + 转中间表示，平台匹配回退链放**前端纯函数**（复用 PROTOCOLS preset + getDefaultEndpoints + platformPaste.ts matchPlatform/guessProtocol），最终转成 `Payload.platform` 复用现有 preview/apply 冲突机制。不新增 export scope，不引入加密。

**Consequences**：复用最大化、双写风险最低；代价是前后端需约定一个 cc-switch provider 中间表示 DTO。代理/自动更新维度因语义不对齐降级为可选/不做。需用真实 cc-switch.db 样本核实 codex provider 的 settings_config 列精确 JSON 结构。

---

## 拆分建议（parent/child）

判定：**单一交付为主，可选拆 2 child**。核心链路（读源 → 匹配回退 → 转换 → 复用 apply → UI）耦合紧，建议作为 parent 主交付一次性完成 D1+D2+D4 + UI + 集成。两个可独立验收的增量：

- **child-1（可选，实验性）**：D3 代理配置导入（依赖 parent 的中间表示 DTO 落地后再做；语义对齐风险高，独立验收）。
- **child-2（可选）**：D5 自动更新/设备设置迁移（同样依赖 parent；范围小且独立）。

若 D3/D5 在 Open Questions 确认为「不做」，则退化为**单一交付**，无 child。

---

## Research References

- cc-switch 仓库（gh api 直读 main 分支）：
  - `docs/user-manual/zh/5-faq/5.1-config-files.md` — 存储目录/DB 表清单/各 app 配置文件形态。
  - `src-tauri/src/database/schema.rs:27-61` — providers + provider_endpoints 表结构。
  - `src-tauri/src/commands/import_export.rs` — cc-switch 自身导出=SQL dump（不适合选择性导入）。
  - `src-tauri/src/database/migration.rs:3-68` — 旧 config.json(MultiAppConfig)→DB 迁移，证明旧 JSON 源仍需支持。
  - `src/config/claudeProviderPresets.ts` — ProviderPreset/settingsConfig(env) 形态 + apiKeyField/apiFormat。
  - `src/config/codexProviderPresets.ts` + `src-tauri/src/codex_config.rs` — codex auth.json + config.toml(wire_api) 形态。
- aidog 现有实现：
  - `src-tauri/src/gateway/import_export/{mod,collect,apply}.rs` — Payload/冲突/upsert_platform_row 复用点。
  - `src/components/settings/ImportExport.tsx:33-138` — scope 卡片 + 冲突决策 UI（集成点）。
  - `src/utils/platformPaste.ts:116,142` — guessProtocol / matchPlatform（base_url→preset 复用）。
  - `src/pages/Platforms.tsx:17-87`(PROTOCOLS+keywords) / `:150`(getDefaultEndpoints) — 平台预设单一事实源。
  - `src-tauri/src/gateway/models.rs:235`(PlatformModels) `:330`(PlatformEndpoint) `:408`(Platform) — 目标数据模型。
- 相关记忆：`import-export-module` / `platform-smart-paste-parser` / `aidog-add-platform-skill` / `platform-protocol-design` / `platform-default-model`。
