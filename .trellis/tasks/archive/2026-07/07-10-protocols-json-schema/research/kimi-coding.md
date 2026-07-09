# Research: Kimi Code Plan（月之暗面 Kimi Code 会员编程权益）

- **Query**: 调研 Kimi 是否有官方 Coding Plan 套餐，为新增 `kimi_coding` 独立协议提供真实字段值
- **Scope**: external（官方文档站）+ internal（preset 模板对照）
- **Date**: 2026-07-10

## 关键结论（先读）

**Kimi 确有官方订阅制编程套餐，正式名称为「Kimi Code」（会员权益名），文档/工具内称「Kimi For Coding」，不是 GLM 那种按量付费的「Coding Plan」而是 Kimi 会员订阅档位附带的编程权益。** 与 `glm_coding` 完全可同模式拆独立协议：

- 独立域名 `api.kimi.com`（普通版是 `api.moonshot.cn`）
- 独立 path 前缀 `/coding/`（普通版无）
- 独占模型 ID `kimi-for-coding`（+ 高速版 `kimi-for-coding-highspeed`）
- 独立鉴权体系：API Key 在 `https://www.kimi.com/code/console` 创建，与开放平台 Key 不通用（错误码 401 `Invalid Authentication` 专门提示「Key 和 Base URL 均不通用」）
- 独立计费：会员订阅（按周/月额度刷新），不是按量付费
- 文档原文：「Kimi Code 和 Kimi 开放平台是两套独立系统，Key 和 Base URL 均不通用」

与 glm_coding 模板对照：glm_coding 普通版 `/api/paas/v4` → coding 版 `/api/coding/paas/v4`（多 `/coding/`）。Kimi 完全同模式：普通版 `https://api.moonshot.cn/v1` → coding 版 `https://api.kimi.com/coding/v1`（独立域名 + `/coding/` 路径）。

## Findings

### 6 字段填齐（直接供 implement agent 抄）

#### 1. base_url

| 协议 | 推荐值 | 说明 |
|---|---|---|
| **OpenAI 兼容** | `https://api.kimi.com/coding/v1` | Codex / Roo Code / OpenCode 等；客户端拼 `/chat/completions` |
| **Anthropic 兼容** | `https://api.kimi.com/coding/` | Claude Code；客户端拼 `/v1/messages`（注意：Base URL 末尾带斜杠，与 glm_coding 的 `https://open.bigmodel.cn/api/anthropic` 不同模式，因为 Kimi 把 messages 路径放在 `/coding/v1/messages`，需保留 `/coding/` 给客户端拼 `/v1/messages`） |

**实测验证**（2026-07-10）：
- `GET https://api.kimi.com/coding/` → HTTP 200 `{"message":"Welcome to the Kimi For Coding API!"}`
- `GET https://api.kimi.com/coding/v1/models`（无鉴权）→ HTTP 401 `{"error":{"message":"Invalid Authentication","type":"invalid_authentication_error"}}`
- `GET https://api.kimi.com/coding/v1/messages` → HTTP 404 `resource_not_found_error`（该端点只接受 POST，符合 Anthropic Messages API 约定）

**来源**：
- https://platform.kimi.com/docs/guide/kimi-cli-support.md 「服务地址」表格
- https://www.kimi.com/code/docs/kimi-code/membership.html（控制台与 endpoint 对照）
- https://www.kimi.com/code/docs/kimi-code/faq.html（平台区别表）
- https://www.kimi.com/code/docs/third-party-tools/other-coding-agents.html（Claude Code 实际 env：`ANTHROPIC_BASE_URL=https://api.kimi.com/coding/`）

#### 2. provider_api_path

| 协议 | 路径 | 最终 URL |
|---|---|---|
| OpenAI | `/chat/completions` | `https://api.kimi.com/coding/v1/chat/completions` |
| Anthropic | `/v1/messages` | `https://api.kimi.com/coding/v1/messages` |

OpenAI 兼容格式（同普通版 kimi 协议），无特殊路径。Anthropic 端点路径与开放平台 `https://platform.moonshot.cn/anthropic` 不同——开放平台用裸 `/anthropic`，Kimi Code 用 `/coding/` 前缀 + 客户端拼 `/v1/messages`。

**注意 aidog base_url 约定**（CLAUDE.md「URL 构造」）：base_url 含版本前缀，`provider_api_path()` 只返回 `/chat/completions`。对 OpenAI 协议端点完全适用，base_url 填 `https://api.kimi.com/coding/v1` 即可；Anthropic 协议端点 base_url 填 `https://api.kimi.com/coding/`，与现 glm_coding anthropic base_url `https://open.bigmodel.cn/api/anthropic` 同性质（都是不带 `/v1` 的根，由客户端拼）。

**来源**：同上服务地址表 + aidog `src-tauri/defaults/platform-presets.json:91-130`（glm_coding 模板）

#### 3. cp 独占模型

**只有两个模型 ID**（与 glm_coding 的多模型列表不同，Kimi Code 用单一固定 ID，后端自动指向最新旗舰模型）：

| Model ID | 说明 | 会员要求 |
|---|---|---|
| `kimi-for-coding` | 普通版（基准速度），后端当前指向 K2.7 Code | 所有 Kimi Code 会员可用 |
| `kimi-for-coding-highspeed` | 高速版，输出速度约普通版 5–6 倍（180~260 Token/s），同一模型 | **需 Allegretto 及以上档位**，权限不足返 401 |

**关键约束**：
- 模型 ID 固定，**不要填 `kimi-k2.7-code`**（那是开放平台按量付费的 ID，Kimi Code 不识别，会报 `Not found the model kimi-for-coding` 404）
- 高速版 ID 必须严格为 `kimi-for-coding-highspeed`，写错会被普通版兜底承接（不报错但不加速）
- Claude Code 中开 Thinking（Option+T / Alt+T）才会调到 K2.7 Code，否则路由到 K2.6

**推荐 model_list**：`["kimi-for-coding", "kimi-for-coding-highspeed"]`
**推荐 models.default**：`{"default": "kimi-for-coding", "opus": "kimi-for-coding-highspeed", "sonnet": "kimi-for-coding", "haiku": "kimi-for-coding"}`（参考 other-coding-agents 文档「Claude Code 中选择 → 实际调用的 Kimi 模型」映射表：Opus→高速版，Sonnet/Haiku→普通版）

**来源**：
- https://platform.kimi.com/docs/guide/kimi-cli-support.md「模型 ID」段
- https://www.kimi.com/code/docs/third-party-tools/other-coding-agents.html（Roo Code 配置表 + Claude Code 映射表）
- https://www.kimi.com/code/docs/kimi-code/error-reference.html（401/404 错误说明）

#### 4. client_type

**两档**：

| client_type | 用途 | 端点 |
|---|---|---|
| `codex_tui` | OpenAI Codex CLI / Roo Code / OpenCode（OpenAI 兼容） | `https://api.kimi.com/coding/v1` |
| `claude_code` | Anthropic Claude Code（Anthropic 兼容） | `https://api.kimi.com/coding/` |

与 glm_coding 同模式（glm_coding 两个 endpoint 分别标 `codex_tui` / `claude_code`）。官方还有自家 `Kimi Code CLI`（OAuth 自动认证，不走 API Key），但 aidog 不需要单独 client_type，归 `codex_tui` 即可。

**来源**：https://platform.kimi.com/docs/guide/codex-kimi.md（Codex 接入）+ https://www.kimi.com/code/docs/third-party-tools/other-coding-agents.html（Claude Code 接入）

#### 5. 鉴权

- **API Key 入口**：`https://www.kimi.com/code/console`（注意是 `www.kimi.com` 不是 `platform.kimi.com`）
- **创建上限**：最多 5 个 Key，仅创建时显示一次（同 OpenAI / Anthropic 模式）
- **请求头**：`Authorization: Bearer <API_KEY>`（OpenAI 协议）或 `x-api-key: <API_KEY>`（Anthropic 协议，Claude Code 走 `ANTHROPIC_API_KEY`）
- **环境变量名**：`KIMI_API_KEY` / `MOONSHOT_API_KEY`（文档混用，aidog 用户层无关）
- **Key 前缀**：**未找到**官方公布的统一前缀约束（glm cp 有特殊前缀是 Zhipu 自家规则；Kimi 文档未提及 Key 前缀规则）。Key 不通用：在 Kimi 开放平台 `platform.kimi.com` 创建的 Key 不能用于 Kimi Code，反之亦然，混用返 401 `Invalid Authentication`。
- **OAuth**：官方 Kimi Code CLI / VS Code 扩展走 OAuth `/login`，免 Key；第三方工具必须用 API Key
- **客户标识禁令**：文档明确「务必保持工具真实身份标识，篡改 User-Agent 视为违规，可能导致会员权益暂停」（aidog 透传 User-Agent 即可，不需特殊处理）

**来源**：
- https://platform.kimi.com/docs/guide/kimi-cli-support.md「获取 API Key」
- https://www.kimi.com/code/docs/kimi-code/error-reference.html「Invalid Authentication」条目

#### 6. 与普通版（开放平台 kimi 协议）区别

| 对比项 | Kimi 开放平台（现 `kimi` 协议） | Kimi Code（拟新增 `kimi_coding`） |
|---|---|---|
| Base URL（OpenAI） | `https://api.moonshot.cn/v1` | `https://api.kimi.com/coding/v1` |
| Base URL（Anthropic） | `https://platform.moonshot.cn/anthropic`（现 preset 实际值） | `https://api.kimi.com/coding/` |
| 模型 | `kimi-k2.7-code` / `kimi-k2.7-code-highspeed` / `kimi-k2.6` 等（明文 ID） | `kimi-for-coding` / `kimi-for-coding-highspeed`（固定别名，后端自动升级） |
| 鉴权 | 开放平台 Key（platform.kimi.com） | Kimi Code 控制台 Key（www.kimi.com/code/console） |
| 计费 | 按量付费，充值即用，每 1M tokens 单独计价（kimi-k2.7-code：input 缓存命中 ¥1.3 / 未命中 ¥6.5 / output ¥27） | 会员订阅（按月/年），周额度（7 天刷新）+ 每 5 小时滚动频控，无 token 单价 |
| 限速 | 按累计充值分 Tier 0–5（Tier0 ¥0：并发 1/RPM 3/TPM 50万；Tier5 ¥2万：并发 1000/RPM 1万/TPM 500万） | 每 5 小时约 300–1200 次请求，最高并发 30 |
| 文档 | https://platform.kimi.com/docs | https://www.kimi.com/code/docs/ |
| 错误码特征 | 标准 OpenAI 错误 | 401 `Invalid Authentication`（Key/URL 混用）、402 会员权益异常、403 周期额度耗尽、429 `kimi monthly usage limit` |

**关键差异点（拆独立协议的理由）**：URL / 模型 ID / Key / 计费 / 错误语义 五个维度全部不同。aidog 现有 `kimi` 协议的 Key 与 `kimi_coding` 完全不能混用——用户切换必须重新填 Key，这正是独立协议存在的价值（同 glm_coding 思路）。

**来源**：
- 普通版：https://platform.kimi.com/docs/pricing/chat-k27-code.md（K2.7 Code 单价）+ https://platform.kimi.com/docs/pricing/limits.md（Tier 表）
- Kimi Code：https://www.kimi.com/code/docs/kimi-code/membership.html（额度规则）+ https://platform.kimi.com/docs/guide/kimi-cli-support.md（频控数字）
- 对比表原文：https://www.kimi.com/code/docs/kimi-code/membership.html「平台对比」段

## Files Found（内部）

| File Path | Description |
|---|---|
| `src-tauri/defaults/platform-presets.json:91-130` | `glm_coding` 模板（同模式参照） |
| `src-tauri/defaults/platform-presets.json:149-166` | 现 `kimi` 协议（普通版，对照） |
| `src-tauri/src/gateway/models/protocol.rs:30-31` | Rust enum `GlmCoding` + `#[serde(rename = "glm_coding")]`（kimi_coding 应加 `KimiCoding` + `#[serde(rename = "kimi_coding")]`） |
| `src-tauri/src/gateway/coding_plan.rs` | `default_is_coding_plan()` 按 serde 名查 preset，需 `is_coding_plan: true` 配合 |

## 推荐结论：可拆 `kimi_coding` 独立协议

### 推荐字段值（可直接填 platform-presets.json）

```json
"kimi_coding": {
  "is_coding_plan": true,
  "client_type": "codex_tui",
  "endpoints": {
    "default": [
      {"protocol": "openai", "base_url": "https://api.kimi.com/coding/v1", "client_type": "codex_tui", "coding_plan": true},
      {"protocol": "anthropic", "base_url": "https://api.kimi.com/coding/", "client_type": "claude_code", "coding_plan": true}
    ]
  },
  "models": {
    "default": {"default": "kimi-for-coding", "opus": "kimi-for-coding-highspeed", "sonnet": "kimi-for-coding", "haiku": "kimi-for-coding"}
  },
  "model_list": {"default": ["kimi-for-coding", "kimi-for-coding-highspeed"]},
  "name": {"en-US": "Kimi Code (Moonshot Membership)", "zh-Hans": "Kimi 编程套餐（月之暗面会员）", ...},
  "desc": {"en-US": "Moonshot Kimi Code membership endpoint (kimi-for-coding alias auto-upgrades to latest flagship)", "zh-Hans": "月之暗面 Kimi 会员编程权益端点（kimi-for-coding 别名自动指向最新旗舰模型）", ...},
  "source_urls": {"docs": "https://www.kimi.com/code/docs/", "pricing": "https://www.kimi.com/membership/pricing"},
  "homepage": "https://www.kimi.com/code/",
  "logo_url": "moonshotai"
}
```

### 同步要改的点（implement agent 注意）

1. **Rust `Protocol` 枚举**（`src-tauri/src/gateway/models/protocol.rs`）：新增
   ```rust
   #[serde(rename = "kimi_coding")]
   KimiCoding,
   ```
2. **前端 `PROTOCOLS`**（`src/domains/platforms/`）：新增独立 `value: "kimi_coding"`
3. **`coding_plan.rs` 单测**（line 49-50 `glm_coding_flagged`）：建议补 `kimi_coding_flagged` 同款断言
4. **Quota 查询**（`src-tauri/src/gateway/quota.rs`）：Kimi Code 走会员订阅，无传统余额概念，余额查询可能 N/A——是否复用现 kimi quota 接口需 implement agent 单独评估（spec: 当前 preset JSON 普通版 kimi 无 quota 字段，kimi_coding 同样无需）
5. **无 peak_hours**：Kimi Code 文档未提高峰倍率（不像 glm_coding 有 3x/2x 高峰乘数），preset 不填 `peak_hours`

### 与 glm_coding 的同 / 异

| 维度 | glm_coding | kimi_coding |
|---|---|---|
| 拆独立协议模式 | ✅ 同 | ✅ 同 |
| `is_coding_plan: true` | ✅ | ✅ |
| 双协议端点（openai + anthropic） | ✅ | ✅ |
| `client_type` 双档（codex_tui + claude_code） | ✅ | ✅ |
| 模型数 | 多（glm-5.2 / 5-turbo / 4.7...） | **少**（仅 kimi-for-coding + highspeed 2 个） |
| peak_hours | 有（3x/2x） | **无** |
| 余额/配额查询 | 走开放平台 | 走会员控制台（API 可能不开放） |

## Caveats / Not Found

1. **会员档位价格未取到具体数字**：landing 页 `https://www.kimi.com/code/` 的「Kimi Code Plan」表格与 `https://www.kimi.com/membership/pricing` 价格均由后端 API 动态渲染，静态 HTML/JS bundle 抓不到。文档只提到「Allegretto 及以上档位」可解锁高速版，档位名称用乐章速度命名（Allegretto 等），具体金额需登录或抓后端 API。**不影响拆协议**（preset 不需要存价格）。
2. **API Key 前缀规则未找到**：glm cp 有特殊 Key 前缀约束（CLAUDE.md 提及），Kimi 文档无类似明文规定。推测为普通 `sk-` 前缀（OpenAI 风格），但未在文档证实——aidog 不做前缀校验所以无影响。
3. **Anthropic base_url 末尾斜杠敏感**：文档表格写 `https://api.kimi.com/coding/`（带斜杠），Claude Code 的 `ANTHROPIC_BASE_URL` 也写带斜杠版本。implement agent 填 preset 时建议保留末尾斜杠（与 glm_coding 的 `/api/anthropic` 不带斜杠风格略异，但这是 Kimi 官方约定，客户端拼 `/v1/messages` 时不会双斜杠——Claude Code 自身处理）。
4. **加油包（Extra Usage）按量计费是 Kimi Code 的兜底机制**：会员订阅额度用尽后可开「加油包」按实际用量扣人民币（不在订阅额度内）。aidog 的 cost 估算若要精确，理论上需识别订阅 vs 加油包扣费模式——但这超出 preset 范围，标记为未来工作。

## 需要: <问题>（转 main）

- Kimi Code 是否开放余额/配额查询 API（类似 `quota.rs` 现有 NewAPI 接口）？若开放，aidog 可在 PlatformCard 显示剩余周额度；若不开放，PlatformCard 余额栏对 kimi_coding 应隐藏。建议 implement agent 不接入 quota，保持与现 `glm_coding` 一致（glm_coding 也无 quota 查询）。
