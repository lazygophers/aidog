# Research: RightCode 全量研究

- **Query**: 查清 RightCode (right.codes) 全量模型清单 + API endpoint 形态，为 platform-presets.json 补全提供权威数据源
- **Scope**: external（官方 docs + 公开 models API）
- **Date**: 2026-07-09

---

## 数据来源（核心证据）

| 来源 | URL | 价值 | 取得时间 |
|---|---|---|---|
| **公开模型 API（权威主源）** | `https://right.codes/models/public` | 返 `{upstreams:[{name,prefix,remark,models:[{name,input_price,output_price,billing_mode,cache_*}]}]}`，全量、结构化、含价格 | 2026-07-09 |
| Curl 调用示例（endpoint 路径权威） | `https://docs.right.codes/docs/rc_extension/curl.html`（src: `rcdoc/src/docs/rc_extension/curl.md`） | 实际 base_url + path 拼接示例 | 2026-07-09 |
| Codex 手动配置 | `rcdoc/src/docs/rc_cli_config/codex.md` | config.toml 示例 + `wire_api=responses` + 模型名 | 2026-07-09 |
| Claude Code 手动配置 | `rcdoc/src/docs/rc_cli_config/claudecode.md` | 双渠道（官渠 `/claude` + 逆向 `/claude-aws`） | 2026-07-09 |
| Gemini 手动配置 | `rcdoc/src/docs/rc_cli_config/gemini.md` | GOOGLE_GEMINI_BASE_URL + 模型 id | 2026-07-09 |
| 文档站首页（类别证据） | `https://docs.right.codes/` | 自述"Codex、Claude Code 大模型 API 分发平台" | 2026-07-09 |
| 文档站 GitHub 源（旁证） | `https://github.com/1198722360/rcdoc` | VuePress 源 markdown + `ModelsPlaza.vue` 组件（暴露 `/models/public` endpoint） | 2026-07-09 |

**注意**：`https://right.codes/pricing` 返回 404（不存在定价页）。定价真值源是 `/models/public` API 的 `input_price` / `output_price` / `request_price` 字段（单位 USD / 1M tokens，`billing_mode="request"` 时为 USD/次）。

---

## API Endpoints

### host 别名

`https://right.codes` 与 `https://www.right.codes` 双 host 并存，均可用（curl 示例与配置文档混用，未观察到差异）。

### 1. Claude 官方渠道（anthropic 协议）

- **base_url**: `https://www.right.codes/claude`（或 `https://right.codes/claude`）
- **path**: `/v1/messages`（标准 anthropic Messages API）
- **鉴权**: `Authorization: Bearer <key>` 或 `x-api-key: <key>`（二者兼容）
- **号池**: Claude Max，**仅限 Claude Code CLI / 插件使用**（remark 原文："仅可在Claude Code CLI或插件使用"）；切号重建缓存有 80% 费用返还
- **适用模型**: `claude-*` 系列（见下）

### 2. Claude awsq 逆向渠道（anthropic 协议）

- **base_url**: `https://right.codes/claude-aws`
- **path**: `/v1/messages`
- **特点**: 支持 **1M 上下文**，缓存率对标 Max 渠道；remark 标注"此渠道暂时活了，且用且珍惜"（不稳定）
- **适用模型**: claude 子集（haiku-4-5 / opus-4-6/4-7/4-8 / sonnet-4-6 / sonnet-5）

### 3. Codex 渠道（OpenAI 双接口）

- **base_url**: `https://right.codes/codex/v1`
- **两条 path**：
  - `/responses`（wire_api = responses，**官方推荐**，Codex CLI 默认走此；支持缓存）
  - `/chat/completions`（标准 OpenAI 兼容，由 /responses 转换而来；**不支持缓存**，且请求体内 system prompt 被强制替换为 codex 默认 instructions → 实际无效）
- **鉴权**: `Authorization` 或 `x-api-key`
- **适用模型**: `gpt-5.4*` / `gpt-5.5*` / `codex-auto-review`

### 4. Gemini 渠道（Google 原生协议）

- **base_url**: `https://right.codes/gemini`
- **path**: `/v1beta/models/{model}:streamGenerateContent?alt=sse`（**google gemini 原生协议**，非 openai 兼容）
- **鉴权**: `x-goog-api-key: <key>`
- **特点**: remark 标注"Gemini cli 逆向，不太稳定"
- **适用模型**: `gemini-*` 系列

### 5. DeepSeek V4（双协议双渠道）

- **OpenAI 格式**: base_url `https://right.codes/deepseek`（标准 openai `/v1/chat/completions`）
- **Anthropic 格式**: base_url `https://right.codes/deepseek/anthropic`（anthropic `/v1/messages`）
- **官方正价、官方同价**，文档 `https://api-docs.deepseek.com`
- **适用模型**: `deepseek-v4-flash` / `deepseek-v4-pro`

### 6. 画图（OpenAI Images 异步）

- **base_url**: `https://www.right.codes/draw`
- **path**: `/v1/images/generations`（固定带 `"async": true`，返 `task_id` 后轮询 `/tasks`）
- **按次计费**（`billing_mode="request"`）
- **适用模型**: `gpt-image-2` / `gpt-image-2-vip` / `nano-banana` / `nano-banana-2` / `nano-banana-2-lite` / `nano-banana-pro`

### 7. 阿里特供 / GLM阿里 / Kimi阿里（**测试中，官方标注"勿用"**）

- prefix: `/ali-sale`、`/glm-ali`、`/kimi-ali`
- **所有三个渠道 remark 均明示「测试中，勿用 / 请勿使用」**
- 建议从 preset 排除（不稳定，仅记录存在）

### endpoint 形态结论（对照现有 preset）

| preset 现配 | 实际正确性 | 备注 |
|---|---|---|
| `https://www.right.codes/claude` (anthropic, claude_code) | ✅ 正确 | 另有 awsq 渠道 `/claude-aws` 可补 |
| `https://right.codes/codex/v1` (openai, codex_tui) | ✅ 正确 | 路径含 `/codex` 是因为 RightCode 把 Codex 号池单独成渠（区别于通用 openai）；非「通用 openai 兼容端点」——通用 openai 兼容在 RightCode 不存在，每渠道 prefix 独立 |
| **缺失**: gemini 渠道 | ❌ 未配 | 应补 `https://right.codes/gemini` (google 协议, gemini_cli) |
| **缺失**: deepseek 双协议 | ❌ 未配 | 应补 `/deepseek` (openai) + `/deepseek/anthropic` (anthropic) |
| **缺失**: claude-aws 渠道 | ❌ 未配 | 可选（不稳定逆向） |

**base_url 各异的原因**：RightCode 不是 new-api/one-api 那种统一网关，而是**按 CLI 工具/号池分渠道**：每渠道对应一个独立上游（Codex 号池 / Claude Max 号池 / Claude awsq 逆向 / Gemini CLI 逆向 / DeepSeek 官方 / 各厂商阿里代理），用 prefix 区分，且每渠道**协议形态严格对应其 CLI 工具**（CC→anthropic、Codex→responses/completions、Gemini→google 原生）。故无单一通用 endpoint。

---

## 模型范围确认

**RightCode 是多供应商聚合平台**，不限于 Claude。覆盖：

- **Anthropic Claude**（官渠 + awsq 逆向）
- **OpenAI GPT-5.x / Codex**
- **Google Gemini**（CLI 逆向）
- **DeepSeek V4**
- **国产（阿里代理）**：GLM / Kimi / MiniMax / Qwen（全部测试中）
- **图像**：gpt-image / nano-banana（Gemini image）

现有 preset 的 model_list 只列 7 个 claude → **严重不全**。

---

## 全量模型清单

> 所有 id 均为 API 调用级精确字符串（裸 id，无 `provider/` 前缀），来源 `/models/public`。
> 价格单位：token 模式 = USD / 1M tokens；request 模式 = USD / 次。
> "avail" 字段全部为 true（截至 2026-07-09）。

### Claude 官方渠道（`/claude`，9 个）

| Model ID | 输入 | 输出 | cache_read | cache_create | 备注 |
|---|---|---|---|---|---|
| `claude-fable-5` | 10 | 50 | 1 | 12.5 | 新增，preset 缺 |
| `claude-haiku-4-5-20251001` | 1 | 5 | 0.1 | 1.25 | **preset 写成 `claude-haiku-4-5`（缺日期后缀，错误）** |
| `claude-opus-4-5-20251101` | 5 | 25 | 0.5 | 6.25 | **preset 写成 `claude-opus-4-5`（缺日期后缀，错误）** |
| `claude-opus-4-6` | 5 | 25 | 0.5 | 6.25 | ✓ |
| `claude-opus-4-7` | 5 | 25 | 0.5 | 6.25 | ✓ |
| `claude-opus-4-8` | 5 | 25 | 0.5 | 6.25 | ✓ |
| `claude-sonnet-4-5-20250929` | 3 | 15 | 0.3 | 3.75 | **preset 写成 `claude-sonnet-4-5`（缺日期后缀，错误）** |
| `claude-sonnet-4-6` | 3 | 15 | 0.3 | 3.75 | ✓ |
| `claude-sonnet-5` | 2 | 10 | 0.2 | 2.5 | 新增，preset 缺，最便宜 sonnet |

### Claude awsq 逆向（`/claude-aws`，6 个，子集，支持 1M 上下文）

`claude-haiku-4-5-20251001`、`claude-opus-4-6`、`claude-opus-4-7`、`claude-opus-4-8`、`claude-sonnet-4-6`、`claude-sonnet-5`（价格同官渠）

### Codex（`/codex`，8 个，OpenAI 协议）

| Model ID | 输入 | 输出 | cache_read | 备注 |
|---|---|---|---|---|
| `codex-auto-review` | 2.5 | 15 | 0.25 | 自动 review 专用 |
| `gpt-5.4` | 2.5 | 15 | 0.25 | |
| `gpt-5.4-high` | 2.5 | 15 | 0.25 | reasoning=high |
| `gpt-5.4-medium` | 2.5 | 15 | 0.25 | reasoning=medium |
| `gpt-5.4-mini` | 0.75 | 4.5 | 0.075 | 最低价 |
| `gpt-5.4-xhigh` | 2.5 | 15 | 0.25 | reasoning=xhigh |
| `gpt-5.5` | 5 | 30 | 0.5 | 最新旗舰 |
| `gpt-5.5-openai-compact` | 5 | 30 | 0.5 | compact 变体 |

### Gemini（`/gemini`，8 个，Google 原生协议，不稳定）

| Model ID | 输入 | 输出 | 备注 |
|---|---|---|---|
| `gemini-2.5-flash` | 0.3 | 2.5 | |
| `gemini-2.5-pro` | 1.25 | 10 | |
| `gemini-3-flash-preview` | 0.5 | 3 | |
| `gemini-3-pro-preview` | 2 | 12 | gemini.md 文档默认推荐 |
| `gemini-3.1-pro` | 2 | 12 | |
| `gemini-3.1-pro-preview` | 2 | 12 | |
| `gemini-3.1-pro-preview-customtools` | 2 | 12 | custom tools 变体 |
| `gemini-3.5-flash` | 1.5 | 9 | |

### DeepSeek V4（双协议同价，2 个）

| Model ID | 输入 | 输出 | cache_read | 备注 |
|---|---|---|---|---|
| `deepseek-v4-flash` | 1 | 2 | 0.02 | 低价档 |
| `deepseek-v4-pro` | 3 | 6 | 0.025 | 正价档，官方同价 |

### 国产系 — 阿里特供（`/ali-sale`，12 个，**测试中勿用**）

| Model ID | 输入 | 输出 | billing | 备注 |
|---|---|---|---|---|
| `glm-4.7` | 2 | 8 | tiered | |
| `glm-5` | 4 | 18 | tiered | |
| `glm-5.1` | 6 | 24 | tiered | |
| `kimi-k2.5` | 4 | 21 | token | |
| `kimi-k2.6` | 6.5 | 27 | token | |
| `MiniMax-M2.5` | 2.1 | 8.4 | token | 注意大小写 |
| `MiniMax-M2.7` | 4.2 | 16.8 | token | |
| `qwen3.6-flash` | 1.2 | 7.2 | tiered | |
| `qwen3.6-max-preview` | 9 | 54 | tiered | |
| `qwen3.6-plus` | 2 | 12 | tiered | |
| `qwen3.7-max` | 12 | 36 | token | 最贵 |
| `qwen3.7-plus` | 2 | 8 | tiered | |

### 国产系 — GLM阿里（`/glm-ali`，4 个，**测试中勿用**）

`glm-4.7`、`glm-5`、`glm-5.1`（同上）、`glm-5.2`（input 8 / output 28）

### 国产系 — Kimi阿里（`/kimi-ali`，3 个，**测试中勿用**）

`kimi-k2.5`、`kimi-k2.6`（同上）、`kimi-k2.7-code`（input 6.5 / output 27）

### 画图（`/draw`，6 个，按次计费，非 chat）

`gpt-image-2`（$0.04）、`gpt-image-2-vip`（$0.13，官方直连）、`nano-banana`=gemini-2.5-flash-image（$0.14）、`nano-banana-2`=gemini-3.1-flash-image-preview（$0.12）、`nano-banana-2-lite`（$0.05）、`nano-banana-pro`=gemini-3-pro-image-preview（$0.18）

---

## 三档默认推荐（供 `models.default`）

按"质量梯度 + 多供应商覆盖"建议（仅稳定渠道，排除测试中的阿里系与不稳定 gemini 逆向、不稳定的 awsq；按价格递增）：

| 档位 | 推荐模型 | 端点 | 理由 |
|---|---|---|---|
| **经济档** | `deepseek-v4-pro` | `/deepseek` (openai) 或 `/deepseek/anthropic` | $3/$6，官方同价，性价比最高 |
| **均衡档（sonnet 等价）** | `claude-sonnet-5` | `/claude` (anthropic) | $2/$10，比 sonnet-4-6 还便宜，Claude 系最新 |
| **旗舰档** | `gpt-5.5` | `/codex/v1` (openai responses) | $5/$30，Codex 旗舰 |

> 备选均衡：`claude-sonnet-4-6`（$3/$15，preset 已有，稳定成熟）。
> 备选旗舰：`claude-opus-4-8`（$5/$25，anthropic 旗舰）。
> 国产首选（若需）：`glm-5.1` 或 `kimi-k2.6`（均在阿里特供渠道，**测试中**，不建议入默认）。

---

## 现有 7 模型核对（preset 现状问题清单）

| preset 现有 | API 实际 | 问题 |
|---|---|---|
| `claude-opus-4-8` | `claude-opus-4-8` | ✅ 正确 |
| `claude-sonnet-4-6` | `claude-sonnet-4-6` | ✅ 正确 |
| `claude-opus-4-7` | `claude-opus-4-7` | ✅ 正确 |
| `claude-opus-4-6` | `claude-opus-4-6` | ✅ 正确 |
| `claude-haiku-4-5` | `claude-haiku-4-5-20251001` | ❌ **id 缺日期后缀，API 调用会失败** |
| `claude-opus-4-5` | `claude-opus-4-5-20251101` | ❌ **id 缺日期后缀，API 调用会失败** |
| `claude-sonnet-4-5` | `claude-sonnet-4-5-20250929` | ❌ **id 缺日期后缀，API 调用会失败** |

**Preset 应补的稳定模型**（共 9 claude + 8 codex + 8 gemini + 2 deepseek = 27，排除测试中的阿里系与画图）：

- Claude 官渠：补 `claude-fable-5`、`claude-sonnet-5`；修正 3 个日期后缀
- Codex：`codex-auto-review`、`gpt-5.4`、`gpt-5.4-high`、`gpt-5.4-medium`、`gpt-5.4-mini`、`gpt-5.4-xhigh`、`gpt-5.5`、`gpt-5.5-openai-compact`
- Gemini：8 个全补（但 remark 自述"不太稳定"，可选）
- DeepSeek：`deepseek-v4-flash`、`deepseek-v4-pro`

---

## Caveats / 不确定

1. **`/models/public` 返的 id 是否即裸 API 调用 id**：高度可信（ModelsPlaza 组件的「复制模型名称」按钮直接复制 `model.name` 给用户粘贴到 CLI config，且 curl 示例 `claude-sonnet-4-5-20250929` 与该字段完全一致）。无 contrary 证据。
2. **host `www.` vs 裸域**：docs 内 curl 示例混用（codex/claude/draw 用 `www.`，gemini 用裸域），未观察到功能差异；推测: 两 host CNAME 同后端，preset 任意选一即可。
3. **gemini 渠道稳定性**：remark 原文"Gemini cli 逆向，不太稳定"——是否入 preset 由 main 决策；研究只确认存在。
4. **阿里特供 / GLM阿里 / Kimi阿里**：官方 remark 全部明示「测试中，勿用」，**强烈建议从 preset 排除**（即便模型 id 已知）。
5. **`tiered` billing_mode**（部分国产模型）：价格随 tier 阶梯变化，`input_price`/`output_price` 为首档或标牌价；不影响 model id，仅影响计费精度。
6. **`gpt-5.3-codex`**：codex.md 文档的 warning 提到此 id（`codex -m gpt-5.3-codex`），但 `/models/public` 未列出 → 推测: 已下线或老 alias，不应入 preset。
7. **未尝试带 key 实际调用验证**：本次为免鉴权公开端点研究，未做端到端请求测试；id 正确性基于文档 + 公开 API 字段双重交叉（claude/codex curl 示例与 `/models/public` 字段一致，已交叉确认）。
8. **pricing URL 失效**：preset 现填 `https://right.codes/pricing` 实际返 404；正确价格源应改为 `https://right.codes/models/public` 或 docs 的 models 页 `https://docs.right.codes/docs/rc_quick_start/models.html`。
