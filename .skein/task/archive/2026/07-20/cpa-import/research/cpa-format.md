# CLIProxyAPI "cpa 格式" 调研

> task: cpa-import | 调研日期: 2026-07-13 | 仓库: router-for-me/CLIProxyAPI (branch main, last commit 2026-07-12)

## 核心结论

**"cpa" = CLIProxyAPI 项目缩写**，不是独立文件格式，没有 `.cpa` 后缀文件。

证据：
- 项目内部代号 CPA：响应头 `X-CPA-VERSION` / `X-CPA-COMMIT` / `X-CPA-BUILD-DATE` / `X-CPA-SUPPORT-PLUGIN`（`internal/api/handlers/management/handler.go` Middleware）；KV 前缀 `cpa:claude:...` / `cpa:codex:...`（`internal/runtime/executor/helps/*.go`）；插件文档通篇 "CPA exposes it under ..."（`examples/plugin/*/README*.md`）。
- 全仓库 code search "cpa" 命中 26 文件 59 处，**无一**指向独立 `.cpa` 文件或导出格式；全部是项目缩写或赞助商 URL 参数（`?source=cpa`）。
- 无文件名含 `cpa`（`ghSearchCode match:"path"` 返空，warning 提示 unproven absence 但与上条互证）。

**"cpa-grok / cpa-codex" 含义**：用户对 CLIProxyAPI `config.yaml` 内按 provider 类型分组的配置段的简称 —— cpa-grok = grok 相关配置（xai OAuth channel 或 openai-compatibility 中的 grok provider），cpa-codex = `codex-api-key` 段或 codex OAuth channel。**不是独立的 provider type 枚举值**。

推测: 用户希望 aidog 能导入 CLIProxyAPI 的 `config.yaml`，把里面已配置好的多个 provider（含 grok/codex 等）一键迁入 aidog platform。

## 格式定义

### 文件类型
- **单文件 YAML**，文件名通常 `config.yaml`（示例 `config.example.yaml`，26784 bytes / 501 行）。
- 解析入口：`internal/config/config.go::LoadConfig` (line 705) → `gopkg.in/yaml.v3` 解码进 `Config` struct。
- 一个 config.yaml 含**所有 provider**（多 provider 共存于一文件），不是每 provider 一文件。
- 另有 **OAuth 凭据文件**：每 provider 一个 JSON，放在 `auth-dir`（默认 `~/.cli-proxy-api`，`const DefaultAuthDir` config.go:26）。仓库 `auths/` 目录仅 `.gitkeep`（无示例）。

### 顶层字段（Config struct，`internal/config/config.go:30-170`）

仅列与 provider/平台导入相关的字段（跳过 server/host/tls/plugins/logging 等运维项）：

| YAML key | Go 类型 | 说明 | 引用 |
|---|---|---|---|
| `gemini-api-key` | `[]GeminiKey` | Gemini API key provider 列表 | config.go:120 |
| `interactions-api-key` | `[]GeminiKey` | Native Interactions API（复用 GeminiKey 结构） | config.go:123 |
| `codex-api-key` | `[]CodexKey` | Codex / OpenAI Responses API key | config.go:126 |
| `claude-api-key` | `[]ClaudeKey` | Claude API key | config.go:136 |
| `openai-compatibility` | `[]OpenAICompatibility` | OpenAI 兼容 provider（含 name + 多 key） | config.go:151 |
| `vertex-api-key` | `[]VertexCompatKey` | Vertex 兼容端点 | config.go:155 |
| `oauth-model-alias` | `map[string][]OAuthModelAlias` | OAuth channel 级模型别名 | config.go:166 |
| `oauth-excluded-models` | `map[string][]string` | OAuth channel 级排除模型 | config.go:158 |
| `api-keys` | `[]string` | 本代理自身的客户端鉴权 key（非上游 provider） | config.example.yaml |
| `auth-dir` | `string` | OAuth 凭据文件目录 | config.go:51 |

### Provider 条目字段表

各 provider struct 共有字段（config.go:440-693）：

| 字段 | ClaudeKey | CodexKey | GeminiKey | OpenAICompatibility | 类型 | 说明 |
|---|---|---|---|---|---|---|
| `api-key` | ✓ | ✓ | ✓ | — (用 api-key-entries) | string | **明文** API key |
| `api-key-entries` | — | — | — | ✓ | `[]{api-key, proxy-url}` | 多 key 列表 |
| `name` | — | — | — | ✓ | string | provider 名（UA 等用） |
| `base-url` | ✓ | ✓ | ✓ | ✓ | string | 上游 base URL |
| `prefix` | ✓ | ✓ | ✓ | ✓ | string | 模型前缀路由（如 `test/model`） |
| `models` | ✓ | ✓ | ✓ | ✓ | `[]{name,alias,display-name,force-mapping}` | 模型映射 |
| `excluded-models` | ✓ | ✓ | ✓ | — | []string | 排除模型，支持 `*` 通配 |
| `headers` | ✓ | ✓ | ✓ | ✓ | map[string]string | 自定义请求头 |
| `proxy-url` | ✓ | ✓ | ✓ | — (per-key) | string | per-provider 代理 |
| `disable-cooling` | ✓ | ✓ | ✓ | ✓ | bool | 禁用冷却 |
| `priority` | ✓ | ✓ | ✓ | ✓ | int | 路由优先级 |
| `disabled` | — | — | — | ✓ | bool | 禁用开关 |
| `websockets` | — | ✓ | — | — | bool | Codex WS |
| `cloak` | ✓ (ClaudeKey) | — | — | — | CloakConfig | Claude 伪装 |
| `image` / `input-modalities` / `output-modalities` / `thinking` | — | — | — | ✓ (OpenAICompatibilityModel) | various | 模型能力声明 |

引用: ClaudeKey config.go:440-480, CodexKey config.go:507-539, GeminiKey config.go:566-594, OpenAICompatibility config.go:621-649, OpenAICompatibilityModel config.go:662-688。

## 最小 cpa(config.yaml) 示例（原文引用）

摘自 `config.example.yaml`（注释块，即推荐用法）：

```yaml
# OpenAI compatibility providers
openai-compatibility:
  - name: "openrouter"
    prefix: "test"
    base-url: "https://openrouter.ai/api/v1"
    api-key-entries:
      - api-key: "sk-or-v1-...b780"
        proxy-url: "socks5://proxy.example.com:1080"
      - api-key: "sk-or-v1-...b781"
    models:
      - name: "moonshotai/kimi-k2:free"
        alias: "kimi-k2"
        display-name: "Kimi K2"
        image: false
        input-modalities: [text, image]
        output-modalities: [text]

# Codex API keys
codex-api-key:
  - api-key: "sk-atSM..."
    base-url: "https://www.example.com"
    models:
      - name: "gpt-5-codex"
        alias: "codex-latest"
        force-mapping: true

# Claude API keys
claude-api-key:
  - api-key: "sk-atSM..."
    base-url: "https://www.example.com"
    models:
      - name: "claude-3-5-sonnet-20241022"
        alias: "claude-sonnet-latest"

# Gemini API keys
gemini-api-key:
  - api-key: "AIzaSy...01"
    prefix: "test"
    base-url: "https://generativelanguage.googleapis.com"
    models:
      - name: "gemini-2.5-flash"
        alias: "gemini-flash"
```

OAuth channel 别名（grok 走 xai channel）：

```yaml
oauth-model-alias:
  xai:
    - name: "grok-4.3"
      alias: "grok-latest"
  codex:
    - name: "gpt-5"
      alias: "g5"
  claude:
    - name: "claude-sonnet-4-5-20250929"
      alias: "cs4.5"
```

OAuth 凭据 JSON 文件示例（config.example.yaml 注释引用，放在 auth-dir）：

```json
{
  "type": "codex",
  "email": "user@example.com",
  "model-aliases": [
    {"name": "gpt-5.3-codex-spark", "alias": "gpt-5.5"},
    {"name": "gpt-5.3-codex-spark", "alias": "gpt-5.4"}
  ]
}
```

来源 URL:
- https://github.com/router-for-me/CLIProxyAPI/blob/main/config.example.yaml
- https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/config/config.go

## 平台类型枚举全集 + aidog Protocol 映射建议

### CLIProxyAPI 侧的 "类型"

CLIProxyAPI **没有单一 provider type 枚举**，类型由两层表达：

1. **config.yaml 顶层 key 段**（决定协议/执行器，硬编码 6 段）：`gemini-api-key` / `interactions-api-key` / `codex-api-key` / `claude-api-key` / `openai-compatibility` / `vertex-api-key`。
2. **OAuth channel 名**（决定 OAuth 凭据路由，`oauth-model-alias` map 的 key，7 个）：`vertex` / `aistudio` / `antigravity` / `claude` / `codex` / `kimi` / `xai`（config.example.yaml `oauth-model-alias` 注释列出）。
3. **Auth.Provider** 是 `string`（`sdk/cliproxy/auth/types.go:53` `Provider string`），非固定枚举；AuthKind 仅两值 `apikey` / `oauth`（`sdk/cliproxy/auth/classification.go:6-7`）。

### aidog Protocol 映射建议

aidog 有 64 个 Protocol 变体（anthropic/openai/codex/gemini/glm/glm_coding/glm_en/kimi/minimax/...）。映射策略按 config 段：

| cpa config 段 / channel | 协议特征 | 建议 aidog Protocol |
|---|---|---|
| `claude-api-key` | Anthropic Messages API | `anthropic` |
| `codex-api-key` | OpenAI Responses (`/responses`) | `codex` |
| `gemini-api-key` / `interactions-api-key` | Gemini generateContent | `gemini` |
| `vertex-api-key` | Vertex 兼容 | `gemini`（推测: aidog 无独立 vertex，按 base_url 路由） |
| `openai-compatibility` 中 name 含 `kimi` / `glm` / `minimax` / `deepseek` / `grok` / `qwen` 等 | OpenAI `/chat/completions` 兼容 | 按 name 关键词匹配到 aidog 对应 Protocol（`kimi`→kimi, `glm`→glm, `grok`→推测需新增或归 openai, `minimax`→minimax） |
| `openai-compatibility`（通用/无法识别 name） | OpenAI 兼容 | `openai`（兜底） |
| OAuth channel `xai` (grok) | xAI Grok | 推测: aidog 需确认是否有 grok/xai protocol；无则归 `openai` 兼容 |
| OAuth channel `kimi` | Kimi OAuth | `kimi` |
| OAuth channel `claude`/`codex`/`vertex`/`aistudio`/`antigravity` | 各自 OAuth | 对应 anthropic/codex/gemini（OAuth 凭据 import 需单独处理 auth-dir JSON） |

**关键差异**：cpa 的 `openai-compatibility` 一个段 = aidog 一个 platform，但 aidog 把 OpenAI 兼容的各厂商拆成独立 Protocol（kimi/glm/minimax/deepseek...）。导入时需 **name 关键词路由** 把 openai-compatibility 条目分发到对应 Protocol，不能一律归 `openai`。具体关键词表需对照 aidog `src-tauri/defaults/platform-presets.json` 61 协议逐一核对（本次未读该文件，标 待核对）。

## 敏感数据处理观察

- **API key 明文**：config.yaml 中 `api-key: "sk-..."` 全明文（config.example.yaml 各段示例均为明文）。无哈希/加密。
- **management secret-key 例外**：`remote-management.secret-key` 启动时 bcrypt 哈希（`hashSecret` config.go:1172，`looksLikeBcrypt` config.go:1098 判重哈希）。
- **OAuth token**：存 auth-dir JSON 文件（`access_token` / `refresh_token` 等），.env.example 中 `PGSTORE_DSN` / `GITSTORE_GIT_TOKEN` / `OBJECTSTORE_SECRET_KEY` 被 redacted（octocode 检测到 secret 自动脱敏），说明项目有 secret 识别机制但仅作用于文档示例。
- **写入保留注释**：`SaveConfigPreserveComments` (config.go:1183) 写回 config.yaml 时保留注释与结构 —— 推测: aidog 若做双向同步可参考，但单向导入无需。
- **导入对 aidog 的影响**：导入 config.yaml 必然把明文 api_key 带入 aidog。aidog 侧 platform 已存 api_key（明文，见 platform-presets.json 模式），语义一致，无需额外脱敏；但应在 UI 提示用户文件含明文凭证。

## 导入语义建议（给 aidog cpa-import 实现方）

1. **解析目标**：读用户提供的 `config.yaml`（YAML），映射到上述 6 个 provider 段。
2. **每段 → aidog platform**：一个 `*-api-key` 数组元素 = 一个 aidog platform；`openai-compatibility` 一个 list 元素（含 name）= 一个 platform。
3. **字段映射**：
   - `base-url` → aidog platform base_url（注意 aidog base_url 含版本前缀如 `/v1`；cpa base-url 通常也含 `/v1`，语义一致，直接用）。
   - `api-key` / `api-key-entries[0].api-key` → platform api_key（多 key 取首个或拆多 platform，推测: 取首个最简）。
   - `models[].name` → platform models（cpa name=上游真名，alias=客户端别名；aidog models 通常用真名，alias 概念 aidog 无直接对应，推测: 导入 name，丢弃 alias 或记入 extra）。
   - `prefix` / `headers` / `proxy-url` / `disabled` → aidog platform.extra 或忽略（aidog 无对应字段时）。
4. **OAuth 凭据**：auth-dir 下的 JSON 文件是另一回事（OAuth token，非 API key），本次仅调研 config.yaml 格式；OAuth import 需单独任务。
5. **Management API 替代**：CLIProxyAPI 暴露 `/v0/management/...` REST（`internal/api/handlers/management/`），README 称支持 "one-click provider import and config sync"。推测: 若 aidog 能调 CPA 的 Management API 拉取，可绕过直接解析 YAML；但需 CPA 实例运行 + secret-key，适用场景窄。直接解析 config.yaml 更通用。

## 引用清单

- `config.example.yaml` — https://github.com/router-for-me/CLIProxyAPI/blob/main/config.example.yaml （格式一手证据，501 行示例）
- `internal/config/config.go` — https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/config/config.go （Config struct line 30-170；ClaudeKey 440-480；CodexKey 507-539；GeminiKey 566-594；OpenAICompatibility 621-649；LoadConfig 705；hashSecret 1172；SaveConfigPreserveComments 1183）
- `sdk/cliproxy/auth/types.go:53` — `Auth.Provider string`（非枚举）
- `sdk/cliproxy/auth/classification.go:6-13` — AuthKind (`apikey`/`oauth`) + AuthSource 常量
- `internal/api/handlers/management/handler.go` — CPA 缩写证据（X-CPA-* 响应头）
- `internal/runtime/executor/helps/*.go` — `cpa:claude:*` / `cpa:codex:*` KV 前缀（CPA 缩写证据）
- OAuth channel 全集（vertex/aistudio/antigravity/claude/codex/kimi/xai）— config.example.yaml `oauth-model-alias` 段注释
- 本地 clone 不存在：`/tmp/CLIProxyAPI`（pathValidationFailed，octocode local 工具不在允许目录）；全部数据来自 GitHub main branch（last commit 2026-07-12 14:13 UTC, by Luis Pater）

## 待确认 / 风险

- **aidog Protocol 全集未核**：本调研未读 `src-tauri/defaults/platform-presets.json`，映射表中 grok/xai/vertex 是否有对应 Protocol 需实现方核对。
- **alias 语义丢失**：cpa 的 name/alias 双名机制在 aidog 无直接落点，导入策略需产品决策。
- **VertexCompatKey 字段未读全**：`internal/config/vertex_compat.go` 未展开（本次符号表未覆盖），如需精确 vertex 导入需补读。
- **OAuth 凭据文件格式**：仅从 config.example.yaml 注释得一例，仓库无完整示例文件（auths/ 仅 .gitkeep）；完整字段需读 `sdk/cliproxy/auth/store.go` 或实际 OAuth 流程代码。
