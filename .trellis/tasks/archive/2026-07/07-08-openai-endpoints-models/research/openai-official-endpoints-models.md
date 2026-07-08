# Research: OpenAI 标准 API canonical model 清单

- **Query**: 补全 `platform-presets.json` **openai 协议**(= 标准 API,`api.openai.com/v1` + API key)`model_list.default` 的官方 GA canonical 真值
- **Scope**: external(OpenAI 官方源)+ internal(model_price 表对齐)
- **Date**: 2026-07-08
- **方向修正(2026-07-08)**:openai 协议 = **标准 API 调用**,**不含 ChatGPT backend codex 端点**。原 endpoints 调研(CHATGPT_CODEX_BASE_URL)已撤销——该端点属 **codex 协议**(Codex CLI / ChatGPT 订阅),由另一 task 处理。endpoints 保持 `https://api.openai.com/v1` 不动。本文件聚焦 model_list。

---

## ⚠️ 撤销标注

原第 1 项「ChatGPT backend codex endpoint」调研结论(`https://chatgpt.com/backend-api/codex`,Codex 源码 `model-provider-info/src/lib.rs:38`)—— **撤销,不适用于 openai 协议**。该端点是 Codex CLI 走 ChatGPT 订阅鉴权的推理路径,归 codex 协议。openai 协议 endpoints 维持现状:

```jsonc
"endpoints": { "default": [{ "protocol": "openai", "base_url": "https://api.openai.com/v1", "client_type": "codex_tui" }] }
```

> `client_type = "codex_tui"` 含义:aidog 内部 client_type 仅 `codex_tui` / `claude_code` / `default` 三值(全表枚举确认);openai 协议的请求体风格(OpenAI Chat Completions 格式)归 codex_tui 类,与 ChatGPT-backend 端点无关。

---

## TL;DR

标准 API(`api.openai.com/v1` + API key)视角下,OpenAI 当前 canonical model 分两层:

- **当前 GA(旗舰主力,长期可用)**:`gpt-5.5`、`gpt-5.4`、`gpt-5.4-mini`、`gpt-5.4-nano`(平台导航标 "Latest: GPT-5.5")。
- **deprecated-but-available(已宣布退役,base alias 标准 API 当前仍可调)**:o-series `o3` / `o3-pro` / `o3-mini` / `o4-mini`(子页标 "Default",dated snapshot 标 Deprecated,base alias 整体 2026-10-23 shutdown → 替换 gpt-5.5 / 5.4-mini);gpt-5 初代 `gpt-5` / `gpt-5-mini` / `gpt-5-nano`(2026-12-11 snapshot shutdown)。
- **pro 变体**:`gpt-5.5-pro` / `gpt-5.4-pro` 在 model_price 表(`models.json`)有定价、未出现在 deprecations 退役表 → 推测 GA(未单独验证子页)。

**裁定(2026-07-08 team-lead)**:采 **A 案**(4 项 GA 旗舰,与 PRD 第 28 行一致);o-series 整代 2026-10-23 退役,不进默认 preset(零腐化优先,用户需 o-series 可在 `platform.extra.model_list` 手加)。`gpt-5.5-pro`/`gpt-5.4-pro` 推测 GA 无实证,不补。B 案事实保留供用户日后改向参考。

---

## Findings

### 标准 API GA canonical model 清单(2026-07-08)

数据源:`platform.openai.com/docs/models/<id>` 各子页 + `platform.openai.com/docs/deprecations` 退役表 + 本地 `src-tauri/defaults/models.json`(model_price 表)对齐。

| canonical id | 系列 | 标准 API 状态 | 退役 / 替换 | 来源 |
|---|---|---|---|---|
| `gpt-5.5` | GPT-5.5 | **GA · 当前旗舰**("our newest frontier model") | — | m_gpt-5.5.html;platform docs nav "Latest: GPT-5.5" |
| `gpt-5.4` | GPT-5.4 | **GA** | — | platform docs landing;codex/models.md |
| `gpt-5.4-mini` | GPT-5.4 | **GA** | — | 同上;deprecations 推荐替换目标 |
| `gpt-5.4-nano` | GPT-5.4 | **GA** | — | platform docs landing |
| `gpt-5.5-pro` | GPT-5.5 pro | **推测 GA** | 未在退役表 | models.json 有价;未单独验证子页 |
| `gpt-5.4-pro` | GPT-5.4 pro | **推测 GA** | 未在退役表 | models.json 有价 |
| `o3` | o-series | **Default(仍可调)** | 2026-10-23 shutdown → gpt-5.5 | m_o3.html "Default";deprecations |
| `o3-pro` | o-series | **Default(仍可调)** | 未单独列退役 → 推测随 o3 波次 | m_o3-pro.html "Default" |
| `o3-mini` | o-series | **Default(base alias)** | 2026-10-23 → gpt-5.5;snapshot `o3-mini-2025-01-31` Deprecated | m_o3-mini.html |
| `o4-mini` | o-series | **Default(仍可调)** | 2026-10-23 → gpt-5.4-mini;"succeeded by GPT-5 mini" | m_o4-mini.html |
| `gpt-5` | GPT-5(初代) | **previous model** | snapshot `gpt-5-2025-08-07` 2026-12-11 → gpt-5.5 | m_gpt-5.html "previous model";deprecations |
| `gpt-5-mini` | GPT-5(初代) | previous | snapshot 2026-12-11 → gpt-5.4-mini | deprecations |
| `gpt-5-nano` | GPT-5(初代) | previous | snapshot 2026-12-11 → gpt-5.4-nano | deprecations |

**不进 model_list(已退役或 Codex 专属)**:

| id | 状态 | 原因 |
|---|---|---|
| `gpt-5.2` / `gpt-5.3-codex` | Codex 内 deprecated | codex/models.md 显式 deprecated(且非标准 API 主推) |
| `gpt-5.1-codex` / `-codex-max` / `-chat-latest` / `gpt-5-codex` / `gpt-5-chat-latest` / `gpt-5.2-chat-latest` / `gpt-5.3-chat-latest` | 2026-07-23 / 08-10 shutdown | 已临/已过退役日(今日 2026-07-08) |
| `gpt-4o` / `gpt-4o-mini` / `gpt-4.1` 系列 | 2026-10-23 shutdown | 上一代,替换 gpt-5.5 / 5.4-mini / 5.4-nano |
| `o1` / `o1-pro` / `o3-deep-research` / `o4-mini-deep-research` | 2026-07-23 / 10-23 shutdown | deep-research / o1 已退役或临近 |
| `openai.gpt-5.5` 等 | 非 canonical | Bedrock provider 前缀,不属 openai 协议 |

> 本地 `models.json` 已含全量 canonical(67 GPT + 11 o 系列,含 chat/codex/pro/mini/nano 变体),定价表就绪——model_list 补入无定价缺口。

### deprecated 判定(对应原第 4 项)

- **gpt-4o / o3(非 mini)/ gpt-5(初代)是否下架**:base alias **当前仍可调**(子页标 Default / previous model),但 **dated snapshots 已 Deprecated,整代已宣布退役**(gpt-4o/o3 波次 2026-10-23,gpt-5 初代 2026-12-11)。是否进 model_list 取决于产品意图:
  - 严格「只暴露长期可用 GA」→ 不补。
  - 「暴露标准 API 当前全部 canonical,客户端自行选」→ 可补(标注 deprecated)。

---

## 给 aidog 的建议

endpoints 不动。`models.default.gpt` 保持 `gpt-5.5`。model_list 两案:

### A 案(严格 GA,与现 PRD 一致)

```jsonc
["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"]
```

- 优点:零腐化风险,只含长期可用旗舰。
- 缺点:不含 o-series,客户端探测拿不到标准 API 仍提供的推理模型。

### B 案(标准 API 全 canonical,含 o3/o4 + pro,呼应 team-lead 2026-07-08 方向)

```jsonc
[
  "gpt-5.5", "gpt-5.5-pro",
  "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano",
  "o3", "o3-pro", "o3-mini", "o4-mini"
]
```

- 优点:标准 API 视角完整;`/v1/models` 探测一次拿到 GPT + o 全系。
- 缺点:o-series 整代 2026-10-23 shutdown,届时 model_list 需再清理(月级腐化,同 STATIC_MODEL_IDS 维护模式)。
- 可选追加 gpt-5 初代(`gpt-5`/`-mini`/`-nano`),2026-12-11 退役——更易腐化,不建议。

### 裁定结果(2026-07-08)

**team-lead 裁定 A 案**,PRD 第 28/56 行保持不动。理由:① preset=稳定默认,零腐化优先(o-series 2026-10-23 shutdown,进默认=自造月级腐化债);② 用户需 o-series 可经 `platform.extra.model_list` 覆盖(机制已有);③ openai 协议默认探测重头是 gpt-5.x 旗舰,o-series 非默认必需。`gpt-5.5-pro`/`gpt-5.4-pro` 推测 GA 无实证,preset 不放未验证项。

B 案(含 o3/o4+pro,9 项)事实与理由保留在上方「B 案」小节,供用户日后改向参考。

## Caveats / Not Found

1. **gpt-5.5-pro / gpt-5.4-pro GA 未单独验证子页**:基于本地 `models.json` 有定价 + deprecations 表未列退役推断;`推测:` 状态,implement 前可拉 `platform.openai.com/docs/models/gpt-5.5-pro` 复核。
2. **o3-pro 退役时点**:deprecations 表点名 o3/o3-mini/o4-mini 的 dated snapshot,o3-pro base alias 未单独点名;子页标 Default → 推测随 o3 波次 2026-10-23 退役,但确切日期未在表内找到。
3. **裸 alias vs dated snapshot**:OpenAI 惯例退役 dated snapshot;base alias(如 `gpt-5`、`o3`)当前仍指向可调模型,但属「已宣布退役一代」。本表按 base alias 当前可调性判定。
4. STATIC_MODEL_IDS(`passthrough.rs`)跨 openai+anthropic 两协议静态返回,补 model_list 时需同步评估(见 PRD Technical Notes)。

## 来源 URL(独立官方源 ≥ 3)

1. https://platform.openai.com/docs/models/gpt-5.5 — "GPT-5.5 is our newest frontier model"(GA 旗舰)
2. https://platform.openai.com/docs/models/o3 / /o3-pro / /o3-mini / /o4-mini — o-series base alias = Default(仍可调)
3. https://platform.openai.com/docs/deprecations — gpt-4o / o3 / o4 / gpt-5 初代 完整退役时间表与替换目标
4. https://platform.openai.com/docs/models — 平台导航 "Latest: GPT-5.5"
5. 本地 `src-tauri/defaults/models.json` — model_price 表已含全量 canonical(定价就绪)
6. https://developers.openai.com/codex/models — Codex 视角(参考,非 openai 协议视角)
