# Research: AIGoCode 全量研究

- **Query**: 查清 AIGoCode (aigocode.com) 全量模型清单 + endpoints，为 aidog `platform-presets.json` 补全提供权威数据源
- **Scope**: external（官方文档）
- **Date**: 2026-07-09

---

## 数据来源（URL + 抓取时间 2026-07-09）

| 文档页 | URL | 用途 |
|---|---|---|
| 模型清单（权威） | https://www.aigocode.com/docs/api/models | 全量 12 模型 ID + 展示名 |
| Base URL 总表 | https://www.aigocode.com/docs/getting-started/base-url | 三协议 Base URL + 工具地址 |
| Claude Code 接入 | https://www.aigocode.com/docs/coding-tools/claude-code | `ANTHROPIC_BASE_URL` 实测值 |
| Codex 接入 | https://www.aigocode.com/docs/coding-tools/codex | `base_url` 实测值 + gpt-5.4 |
| Gemini CLI 接入 | https://www.aigocode.com/docs/coding-tools/gemini-cli | `GOOGLE_GEMINI_BASE_URL` 实测值 |
| Quickstart | https://www.aigocode.com/docs/getting-started/quickstart | OpenAI 全路径示例 |
| 价格页 | https://www.aigocode.com/pricing | SPA 客户端渲染，初 HTML 无定价表（未取到上下文窗口） |

抓取方式：`curl -x http://127.0.0.1:7890` 走代理（直连 DNS 解析到 199.16.156.38 等会 timeout）。页面为 Next.js SPA，正文由 RSC flight payload (`self.__next_f.push`) 携带，用正则剥离后即得服务端渲染文本。

---

## API Endpoints

### 官方 Base URL 总表（来源：base-url 页）

| 协议 | 服务端最终路径前缀（"协议地址"） |
|---|---|
| OpenAI Compatible | `https://api.aigocode.com/v1` |
| Anthropic Compatible | `https://api.aigocode.com/v1` |
| Gemini Compatible | `https://api.aigocode.com/v1beta` |

> 注意：这是**服务端最终 URL 前缀**，不是各客户端填的 base_url。官方原话：「不同接入方式对 Base URL 的填写方式略有不同」「例如 Claude Code、Codex、Gemini CLI、Cherry Studio、OpenCode、OpenClaw、Hermes 的教程里都会明确写该填根地址还是协议地址」。

### 各客户端实际填写（来源：三份 coding-tools 文档）

| 客户端 | 环境变量 | 官方填值 | 推导最终 URL |
|---|---|---|---|
| Claude Code | `ANTHROPIC_BASE_URL` | `https://api.aigocode.com`（根） | SDK 追加 `/v1/messages` → `…/v1/messages` |
| Codex CLI | `base_url`（config.toml） | `https://api.aigocode.com`（根） | SDK 追加 `/v1/chat/completions` |
| Gemini CLI | `GOOGLE_GEMINI_BASE_URL` | `https://api.aigocode.com`（根） | SDK 追加 `/v1beta/models/{model}:generateContent` |
| OpenAI SDK 直连 | `base_url` | `https://api.aigocode.com/v1` | `…/v1/chat/completions`（quickstart 实证） |

### 与 aidog preset 的对照（aidog 在 base_url 后追加协议 path）

| 协议 | aidog 当前 base_url | 是否正确 | 说明 |
|---|---|---|---|
| anthropic | `https://api.aigocode.com` | ✅ 正确 | aidog 追加 `/v1/messages`，与 Claude Code 一致 |
| openai | `https://api.aigocode.com/v1` | ✅ 正确 | aidog 追加 `/chat/completions`，与 OpenAI SDK 一致 |
| gemini | `https://api.aigocode.com` | ✅ 正确 | aidog 追加 `/v1beta/models/...`，与 Gemini CLI 一致 |

**结论**：现有 3 个 endpoint base_url **全部正确**，无需改动 `endpoints` 字段。仅 `model_list` 需补全。

---

## 模型范围确认

**AIGoCode 不是 Claude-only 中转，是多供应商聚合**，覆盖 4 大类：
- Anthropic Claude（5 款）
- OpenAI GPT（3 款）
- Google Gemini（3 款）
- 图像生成（1 款，image-2，非 LLM）

**不提供**：DeepSeek / Qwen / GLM / Kimi / MiniMax / Grok 等国产或 xAI 模型。

---

## 全量模型清单（12 款，权威源：/docs/api/models）

官方原话：「模型 ID 必须完整匹配。不同套餐、分组或上游状态可能会影响实际可用模型，控制台和服务端配置始终优先于文档。」

### Claude 系（5，Anthropic，走 anthropic 协议）

| model id | 展示名 | 备注 |
|---|---|---|
| `claude-opus-4-8` | Claude Opus 4.8 | 旗舰，长上下文/复杂推理档 |
| `claude-opus-4-7` | Claude Opus 4.7 | |
| `claude-opus-4-6` | Claude Opus 4.6 | |
| `claude-sonnet-4-6` | Claude Sonnet 4.6 | sonnet 仅此一款 |
| `claude-haiku-4-5-20251001` | Claude Haiku 4.5 | **id 含日期后缀 `-20251001`**，非裸 `claude-haiku-4-5` |

### OpenAI 系（3，走 openai 协议）

| model id | 展示名 | 备注 |
|---|---|---|
| `gpt-5.5` | GPT-5.5 | GPT 旗舰档 |
| `gpt-5.4` | GPT-5.4 | codex 文档示例模型 |
| `gpt-5.4-mini` | GPT-5.4 Mini | 成本敏感档 |

### Google 系（3，走 gemini 协议）

| model id | 展示名 | 备注 |
|---|---|---|
| `gemini-3.1-pro-preview` | Gemini 3.1 Pro Preview | Pro 档，长上下文 |
| `gemini-3.5-flash` | Gemini 3.5 Flash | flash 档 |
| `gemini-3-flash-preview` | Gemini 3 Flash Preview | |

### 图像（1，非 LLM）

| model id | 展示名 | 备注 |
|---|---|---|
| `image-2` | Image 2 | 图像生成，**aidog 路由不适用**（仅文本对话） |

> 上下文窗口：官方模型页未列；pricing 页是纯客户端渲染，初 HTML 抓不到数值。建议：不在 preset 中填 `context_length`，由用户控制台或 `fetchModels` 拉取。

---

## 三档默认推荐（供 `models.default`）

AIGoCode 是多协议聚合，单 `default` 分支按 aidog 三协议对称推荐主推模型：

```json
"models": {
  "default": {
    "anthropic": "claude-sonnet-4-6",
    "openai": "gpt-5.4",
    "gemini": "gemini-3.5-flash"
  }
}
```

理由：
- anthropic → sonnet 是 sonnet 档唯一选择（仅 `claude-sonnet-4-6` 一款），平衡型默认
- openai → `gpt-5.4`（非 mini、非 5.5 旗舰），平价主力；codex 文档官方示例就是 5.4
- gemini → `gemini-3.5-flash`（成本敏感场景，flash 优先；pro-preview 不做默认）

候选手动切换档：`claude-opus-4-8`（Claude 旗舰）/ `gpt-5.5`（GPT 旗舰）/ `gemini-3.1-pro-preview`（Gemini Pro）。

---

## 现有 7 模型核对（对照官方 12 模型）

当前 `platform-presets.json` aigocode 的 `model_list.default`：

| 当前 id | 官方状态 | 处置 |
|---|---|---|
| `claude-opus-4-8` | ✅ 在官方表 | 保留 |
| `claude-sonnet-4-6` | ✅ 在官方表 | 保留 |
| `claude-opus-4-7` | ✅ 在官方表 | 保留 |
| `claude-opus-4-6` | ✅ 在官方表 | 保留 |
| `claude-haiku-4-5` | ⚠️ **id 不精确** | 改为 `claude-haiku-4-5-20251001`（官方含日期后缀） |
| `claude-opus-4-5` | ❌ **已下架**（官方表无） | 删除 |
| `claude-sonnet-4-5` | ❌ **已下架**（官方表无） | 删除 |

**需新增 6 款**（image-2 非文本对话，不入 aidog 路由，跳过）：
- `gpt-5.5` / `gpt-5.4` / `gpt-5.4-mini`（openai 协议）
- `gemini-3.5-flash` / `gemini-3.1-pro-preview` / `gemini-3-flash-preview`（gemini 协议）

建议最终 `model_list.default`（11 款，按「Claude → GPT → Gemini」分组排序，image-2 排除）：

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-4-6",
    "claude-haiku-4-5-20251001",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gemini-3.1-pro-preview",
    "gemini-3.5-flash",
    "gemini-3-flash-preview"
  ]
}
```

---

## Caveats / Not Found

1. **id 格式风险**：当前 preset 用裸 id（如 `claude-opus-4-8`），与官方 `/docs/api/models` 完全一致，**无需添加 `provider/` 前缀**。但 `claude-haiku-4-5-20251001` 必须保留日期后缀，否则服务端不识别。
2. **image-2 是否入 model_list**：官方归类「图像生成」非对话模型。aidog 仅路由对话，建议不入 `model_list`。如用户反馈需要，可单独追加但走 openai images API（aidog 当前不支持）。
3. **上下文窗口**：官方模型页与可抓取的 pricing 初 HTML 均无数值。如需填 `models.default.*.context_length`，需用浏览器渲染 pricing 页或调用 `/v1/models` 带 key 拉取（本次未带 key）。
4. **`/v1/models` 探测**：`GET https://api.aigocode.com/v1/models` 返 `404 + {"code":"API_KEY_REQUIRED"}`，证实端点存在但需鉴权（Authorization Bearer / x-api-key / x-goog-api-key）；本次未持 key，无法拉服务端真值列表，全量来源以官方文档页为准。
5. **3 个 endpoint 的 client_type**：现有 preset 已正确（anthropic→claude_code / openai→codex_tui / gemini→default），本次研究范围不含改动。
6. **价格表未取到**：pricing 页 (`/pricing`) 是纯 SPA，初 HTML 仅含 1 个 `<title>Ai Go Code</title>` 与脚本引用，数值由客户端运行时拉取；本次未走 headless 浏览器，定价数据缺。aidog 用 `est_cost` 走 `resolve_price` 回退链，preset 不强制定价，故不影响补全。
