# Research: 智谱 GLM 全模型 + 端点审计

- **Query**: 查智谱 AI（BigModel / z.ai）全部官方 GLM 文本模型 + 端点，判现 aidog preset 遗漏
- **Scope**: external（官方文档）+ internal（preset 比对）
- **Date**: 2026-07-09
- **数据源**:
  - 国内: `https://docs.bigmodel.cn/llms-full.txt`（39578 行快照，2026-07-09）
  - 国际: `https://docs.z.ai/llms-full.txt`（18421 行快照，2026-07-09）

---

## 1. 全文本模型清单（API id 维度）

> 仅含 chat 文本模型。排除 CogVideoX / CogView / GLM-Image / GLM-Embedding / GLM-OCR / GLM-TTS / GLM-ASR / GLM-Realtime / GLM-Voice（非文本 chat 或多模态）。
> 视觉理解模型（GLM-5V-Turbo / GLM-4.6V / GLM-4.5V 等）亦不在文本 preset 范围。

### 1.1 国内 BigModel（open.bigmodel.cn）

权威两表：模型一览表（`bigmodel-full.txt:33399-33412`）+ `max_tokens` 表（`bigmodel-full.txt:32937-32962`）。

| API model id | 系列 | 上下文 | 默认/最大输出 | 状态 | 用途 | 文档 URL |
|---|---|---|---|---|---|---|
| `glm-5.2` | GLM-5 | **1M** | 65536 / 131072 | 在售旗舰 | 长程工程、Coding SOTA；Coding Plan 支持 | https://docs.bigmodel.cn/cn/guide/models/text/glm-5.2 |
| `glm-5.1` | GLM-5 | 200K | 65536 / 131072 | **历史**（调用自动切到 5.2，见 `bigmodel-full.txt:9603`） | — | https://docs.bigmodel.cn/cn/guide/models/text/glm-5.1 |
| `glm-5` | GLM-5 | 200K | 65536 / 131072 | **历史**（自动切到 5.2） | — | https://docs.bigmodel.cn/cn/guide/models/text/glm-5 |
| `glm-5-turbo` | GLM-5 | 200K | 65536 / 131072 | 在售 | 龙虾/OpenClaw 任务优化；Coding Plan 支持 | https://docs.bigmodel.cn/cn/guide/models/text/glm-5-turbo |
| `glm-4.7` | GLM-4.7 | 200K | 65536 / 131072 | 在售旗舰 | Agentic Coding、通用对话；Coding Plan 支持 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7 |
| `glm-4.7-flashx` | GLM-4.7 | 200K | 65536 / 131072 | 在售 | 轻量高速版（同 4.7 基座，对标国产前端/写作） | https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7 （Tab: GLM-4.7-FlashX） |
| `glm-4.6` | GLM-4.6 | 200K | 65536 / 131072 | 在售 | 高级编码 + 工具调用 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4.6 |
| `glm-4.5-air` | GLM-4.5 | 128K | 65536 / 98304 | 在售（高性价比） | 106B/12B MoE，推理+智能体稳定 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4.5 |
| `glm-4.5-airx` | GLM-4.5 | 128K | 65536 / 98304 | 在售 | Air 极速版（低延迟） | https://docs.bigmodel.cn/cn/guide/models/text/glm-4.5 （Tab: GLM-4.5-AirX） |
| `glm-4.5` | GLM-4.5 | 128K | 65536 / 98304 | **即将下线**（`bigmodel-full.txt:23438`） | 355B/32B MoE 旗舰，已建议迁 4.7 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4.5 |
| `glm-4.5-x` | GLM-4.5 | 128K | 65536 / 98304 | **即将下线** | — | 同上 |
| `glm-4.5-flash` | GLM-4.5 | 128K | 65536 / 98304 | 免费，**即将下线** | 普惠 + 深度思考 | https://docs.bigmodel.cn/cn/guide/models/free/glm-4.5-flash |
| `glm-4.7-flash` | GLM-4.7 | 200K | 65536 / 131072 | **免费**（2026-01-19 上线，`bigmodel-full.txt:39394`） | 延续 4.7 基座普惠版 | https://docs.bigmodel.cn/cn/guide/models/free/glm-4.7-flash |
| `glm-4-long` | GLM-4 | **1M** | 动态 / 4095（`bigmodel-full.txt:32962` 注：表内标 4K，资源包表标 4095） | 在售（长上下文场景） | 1M 长文本/记忆型任务，输出仅 4K | https://docs.bigmodel.cn/cn/guide/models/text/glm-4-long |
| `glm-4-flashx-250414` | GLM-4 | 128K | 16384 / 16384 | 在售（免费配额） | Flash 增强高速 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4 |
| `glm-4-flash-250414` | GLM-4 | 128K | 32768 / 32768 | 免费 | 多语言理解 + 工具调用 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4 |
| `glm-4-air-250414` | GLM-4 | — | 16384 / 16384 | 在售 | — | 同 glm-4 系列 |
| `glm-4-plus` / `glm-4-air` / `glm-4-airx` / `glm-4-flash` / `glm-4-flashx` | GLM-4 (legacy) | — | 动态 / 4095 | 老 4 系列（`bigmodel-full.txt:32956-32960`） | 已被 250414 替代 | https://docs.bigmodel.cn/cn/guide/models/text/glm-4 |

> 注：`max_tokens` 表（`bigmodel-full.txt:32937+`）是 API 真实可接受的 id 字面值；与上面 model 一览表完全一致，可作权威。

### 1.2 国际 z.ai（api.z.ai）

权威表：`zai-full.txt:13415-13444`（Models, Agents and Tools）+ `zai-full.txt:13080-13100`（max_tokens 表）。

| API model id | 与国内差异 | 上下文 | 文档 URL |
|---|---|---|---|
| `glm-5.2` | 同 | 1M | https://docs.z.ai/guides/llm/glm-5.2 |
| `glm-5.1` | 同 | 200K | https://docs.z.ai/guides/llm/glm-5.1 |
| `glm-5` | 同 | 200K | https://docs.z.ai/guides/llm/glm-5 |
| `glm-5-turbo` | 同 | 200K | https://docs.z.ai/guides/llm/glm-5-turbo |
| `glm-4.7` | 同 | 200K | https://docs.z.ai/guides/llm/glm-4.7 |
| `glm-4.7-flashx` | 同 | 200K | https://docs.z.ai/guides/llm/glm-4.7 |
| `glm-4.6` | 同 | 200K | https://docs.z.ai/guides/llm/glm-4.6 |
| `glm-4.5` | 同（即将下线） | 128K | https://docs.z.ai/guides/llm/glm-4.5 |
| `glm-4.5-x` | 同（即将下线） | 128K | 同上 |
| `glm-4.5-air` | 同 | 128K | 同上 |
| `glm-4.5-airx` | 同 | 128K | 同上 |
| `glm-4-32b-0414-128k` | **z.ai 独有**（国内 docs 无此页） | 128K | https://docs.z.ai/guides/llm/glm-4-32b-0414-128k |
| `glm-4.7-flash` | 同（免费） | 200K | https://docs.z.ai/guides/llm/glm-4.7 |
| `glm-4.5-flash` | 同（免费） | 200K | https://docs.z.ai/guides/llm/glm-4.5 |

z.ai 没有 `glm-4-long` 文档页，也未在文本模型一览出现。

---

## 2. 全端点清单

### 2.1 普通按量计费端点（默认分支）

| 区域 | 协议 | base_url | 文档 |
|---|---|---|---|
| 国内 | OpenAI 兼容 | `https://open.bigmodel.cn/api/paas/v4` | `bigmodel-full.txt:476,512,888,1338` |
| 国内 | Anthropic 兼容 | `https://open.bigmodel.cn/api/anthropic` | `bigmodel-full.txt:6674` |
| 国际 | OpenAI 兼容 | `https://api.z.ai/api/paas/v4` | `zai-full.txt:171,202,346,379` |
| 国际 | Anthropic 兼容 | `https://api.z.ai/api/anthropic` | `zai-full.txt:1037,1060` |

### 2.2 GLM Coding Plan 专用端点

Coding Plan 走**独立 base_url**（仅 OpenAI 协议差异；Anthropic 协议与普通端点相同）。Coding Plan 仅支持 3 个模型：`glm-5.2` / `glm-5-turbo` / `glm-4.7`（`zai-full.txt:1043`、`bigmodel-full.txt:6445,9602`）。

| 区域 | 协议 | coding_plan base_url | 文档 |
|---|---|---|---|
| 国内 | Anthropic (Claude Code/Goose) | `https://open.bigmodel.cn/api/anthropic` （同普通，鉴权 API Key 自动识别套餐） | `bigmodel-full.txt:6674,6714` |
| 国内 | OpenAI 兼容 (Codex TUI / Cherry Studio / 其他) | `https://open.bigmodel.cn/api/coding/paas/v4` （Cherry Studio 文档带尾斜杠） | `bigmodel-full.txt:6675-6676` |
| 国际 | Anthropic (Claude Code/Goose) | `https://api.z.ai/api/anthropic` （同普通） | `zai-full.txt:1037,1060` |
| 国际 | OpenAI 兼容 (其他工具) | `https://api.z.ai/api/coding/paas/v4` | `zai-full.txt:1038,1061,1161` |

> **关键**: Coding Plan 无专用模型变体（无 `glm-coding-*` id）。区别仅是端点 URL，且仅 OpenAI 协议端点有 `/coding/` 路径段。Anthropic 协议端点同 URL，靠 API Key 鉴权识别套餐。
>
> 用普通 `/api/paas/v4` 调套餐会扣账户余额或报 1113（`bigmodel-full.txt:6670-6677`）。

### 2.3 GLM-5.2 1M 上下文

`glm-5.2[1m]` 是模型 id 后缀而非独立端点（`bigmodel-full.txt:6772-6780`）。需配 `CLAUDE_CODE_MAX_COMPRESSION_WINDOW`。

---

## 3. 现 preset 比对

### 3.1 现 `model_list.default`（两协议同）

```
glm-5.2, glm-5.1, glm-5, glm-5-turbo, glm-4.7, glm-4.7-flash, glm-4.6, glm-4.5-air
```

源: `src-tauri/defaults/platform-presets.json:236-245`（glm）与 `:295-306`（glm_en，完全相同）。

### 3.2 遗漏分析（按"应否进 default"分类）

| model id | 是否遗漏 | 建议 | 理由 |
|---|---|---|---|
| `glm-4.7-flashx` | **遗漏** | 加入 | 在售、文档单独列 Tab；同基座轻量高速版，对照 4.7 用户可选；与 `glm-4.7-flash`(免费) 区分明显 |
| `glm-4.5-airx` | 遗漏 | 可加 | 在售，Air 极速版；与已有 `glm-4.5-air` 配对，符合"轻量+高速"双档惯例 |
| `glm-4.5` | 遗漏 | **不加** | 官方明示「即将下线，建议选 GLM-4.7」（`bigmodel-full.txt:23438`） |
| `glm-4.5-x` | 遗漏 | **不加** | 同上，即将下线 |
| `glm-4.5-flash` | 遗漏 | 不加 | 免费 + 即将下线；且 default 已有 `glm-4.7-flash` 免费占位 |
| `glm-4-long` | 遗漏 | 不加 | 输出仅 4K，与代理/Coding 场景不匹配；niche |
| `glm-4-flashx-250414` / `glm-4-flash-250414` / `glm-4-air-250414` | 遗漏 | 不加 | GLM-4 老 250414 系列，能力天花板低；不属于 GLM 5/4.7 主线 |
| `glm-4-plus` 等 legacy | 遗漏 | 不加 | max_tokens 仅 4095，已淘汰 |
| `glm-4-32b-0414-128k` | 遗漏 | **可选加（仅 glm_en）** | z.ai 独有；128K 输出 16K，开源 32B 高性价比；但国内端点不可用，会致两协议 list 不一致 |

### 3.3 可疑项核验

| preset 内现有 id | 核验结果 |
|---|---|
| `glm-5.2` | 在售旗舰，OK |
| `glm-5.1` | 历史 id，调用自动迁 5.2（`bigmodel-full.txt:9603`）；保留无害（向后兼容用户配置） |
| `glm-5` | 同上 |
| `glm-5-turbo` | 在售，OK |
| `glm-4.7` | 在售，OK |
| `glm-4.7-flash` | **免费模型**（`free/glm-4.7-flash`），非 `glm-4.7-flashx`；preset 现值是免费版，OK 但需意识到这是免费普惠版而非轻量高速版 |
| `glm-4.6` | 在售，OK |
| `glm-4.5-air` | 在售，OK（注意：4.5 系列 `glm-4.5` / `glm-4.5-x` 即将下线，但 `glm-4.5-air` / `glm-4.5-airx` 在售保留） |

### 3.4 现 endpoints 比对

现 preset（glm 与 glm_en 一致）：两 endpoint 均 `coding_plan: false`，对应「普通按量」分支。

| 协议 | 现值 | 是否正确 |
|---|---|---|
| openai | `https://open.bigmodel.cn/api/paas/v4`（glm）/ `https://api.z.ai/api/paas/v4`（glm_en） | 正确（普通分支） |
| anthropic | `https://open.bigmodel.cn/api/anthropic`（glm）/ `https://api.z.ai/api/anthropic`（glm_en） | 正确（普通分支；同时也是 coding plan 分支，因 Anthropic 端点共用） |

**缺失**: `coding_plan: true` 的 OpenAI 分支：
- 国内: `https://open.bigmodel.cn/api/coding/paas/v4`
- 国际: `https://api.z.ai/api/coding/paas/v4`

> 但 per CLAUDE.md 约定（2026-07-08 起 preset JSON 默认不带 `coding_plan` 子分支，机制保留 via `platform.extra`），这是**有意为之**。用户级 `platform.extra` 可手工启用 cp 端点。Anthropic 协议同 URL 故无需额外分支。

---

## 4. 推荐 final 清单

### 4.1 `model_list.default`

**推荐（保守，仅加 1 个高价值项）**:

```json
[
  "glm-5.2",
  "glm-5.1",
  "glm-5",
  "glm-5-turbo",
  "glm-4.7",
  "glm-4.7-flashx",      // 新增：在售，4.7 轻量高速版（与免费 flash 区分）
  "glm-4.7-flash",       // 保留：免费普惠版
  "glm-4.6",
  "glm-4.5-air",
  "glm-4.5-airx"         // 可选新增：与 4.5-air 配对，极速版
]
```

**glm 与 glm_en 是否应一致？**

- 默认推荐：**一致**（便于维护、用户跨区域切换体验同）
- 若要差异化：仅 `glm_en` 加 `glm-4-32b-0414-128k`（z.ai 独有，开源 32B 高性价比，128K 上下文）；国内端点调用此 id 会失败。
- 决策: 维持一致更稳妥；32B 老模型（非 5/4.7 主线）可暂不加。

### 4.2 `endpoints.default`

**维持现状**（普通分支），不引入 coding_plan 子分支（遵循 2026-07-08 去重决策）：

```json
[
  {"protocol": "openai",     "base_url": "https://open.bigmodel.cn/api/paas/v4",   "client_type": "codex_tui",   "coding_plan": false},
  {"protocol": "anthropic",  "base_url": "https://open.bigmodel.cn/api/anthropic", "client_type": "claude_code", "coding_plan": false}
]
```

**Coding Plan 用户**: 走 `platform.extra` 手工配 `https://open.bigmodel.cn/api/coding/paas/v4`（OpenAI 协议）；Anthropic 协议无需改 URL。如未来要内置 cp 分支，文档依据已就位（见 §2.2）。

### 4.3 `models.default.default`

维持 `glm-5.2`。

---

## 5. 引用清单

### 国内 BigModel
- 模型一览表: https://docs.bigmodel.cn/cn/guide/models/text （`bigmodel-full.txt:33399-33412`）
- `max_tokens` 表: https://docs.bigmodel.cn/cn/guide/overview/concept-param （`bigmodel-full.txt:32937-32962`）
- GLM-5.2: https://docs.bigmodel.cn/cn/guide/models/text/glm-5.2
- GLM-5.1: https://docs.bigmodel.cn/cn/guide/models/text/glm-5.1
- GLM-5: https://docs.bigmodel.cn/cn/guide/models/text/glm-5
- GLM-5-Turbo: https://docs.bigmodel.cn/cn/guide/models/text/glm-5-turbo
- GLM-4.7: https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7
- GLM-4.6: https://docs.bigmodel.cn/cn/guide/models/text/glm-4.6
- GLM-4.5 系列: https://docs.bigmodel.cn/cn/guide/models/text/glm-4.5 （含 Air/AirX/X/Flash Tabs）
- GLM-4.7-Flash 免费: https://docs.bigmodel.cn/cn/guide/models/free/glm-4.7-flash
- GLM-4.5-Flash 免费: https://docs.bigmodel.cn/cn/guide/models/free/glm-4.5-flash
- GLM-4-Long: https://docs.bigmodel.cn/cn/guide/models/text/glm-4-long
- GLM-4 系列: https://docs.bigmodel.cn/cn/guide/models/text/glm-4
- 普通 base_url: https://docs.bigmodel.cn/cn/guide/start/quick-start （`bigmodel-full.txt:476,512`）
- Coding Plan base_url: https://docs.bigmodel.cn/cn/coding-plan/faq （`bigmodel-full.txt:6674-6677`）
- Coding Plan 支持模型: https://docs.bigmodel.cn/cn/coding-plan/latest-model （`bigmodel-full.txt:6445,9602`）
- Coding Plan 快速开始: https://docs.bigmodel.cn/cn/coding-plan/quick-start
- 迁移到 GLM-5.2: https://docs.bigmodel.cn/cn/guide/start/migrate-to-glm-new

### 国际 z.ai
- 模型一览表: https://docs.z.ai/guides/overview/overview （`zai-full.txt:13415-13444`）
- `max_tokens` 表: https://docs.z.ai/guides/overview/concept-param （`zai-full.txt:13080-13100`）
- GLM-5.2: https://docs.z.ai/guides/llm/glm-5.2
- GLM-5.1: https://docs.z.ai/guides/llm/glm-5.1
- GLM-5: https://docs.z.ai/guides/llm/glm-5
- GLM-5-Turbo: https://docs.z.ai/guides/llm/glm-5-turbo
- GLM-4.7: https://docs.z.ai/guides/llm/glm-4.7
- GLM-4.6: https://docs.z.ai/guides/llm/glm-4.6
- GLM-4.5 系列: https://docs.z.ai/guides/llm/glm-4.5
- GLM-4-32B-0414-128K: https://docs.z.ai/guides/llm/glm-4-32b-0414-128k （**z.ai 独有**）
- 普通 + Coding Plan 端点: https://docs.z.ai/devpack/faq （`zai-full.txt:1037-1038,1060-1061`）
- Coding Plan 支持模型（3 个）: https://docs.z.ai/devpack/overview （`zai-full.txt:1043`）
- 切模型指南: https://docs.z.ai/devpack/latest-model
- 定价: https://docs.z.ai/guides/overview/pricing

### 内部（aidog preset）
- `src-tauri/defaults/platform-presets.json:212-273`（glm）
- `src-tauri/defaults/platform-presets.json:274-340`（glm_en）

---

## 6. Caveats / 未决

- **Mintlify SPA 文档**: 通过官方 `llms-full.txt` 抓取（2026-07-09），等价于官方渲染后正文。所有 model id 与 `max_tokens` 表内字面值严格对齐，未做臆测。
- **GLM-5.2 1M 上下文**: 通过模型 id 后缀 `glm-5.2[1m]` 启用，非独立 model id；preset `model_list` 不需要单独列项（用户用 `extra` 或直接手填）。详见 `bigmodel-full.txt:6772-6780`。
- **`glm-4.7-flash` vs `glm-4.7-flashx`**: 现 preset 已有前者（免费普惠版）；后者（轻量高速版，付费）未列入。两者能力档不同，建议同时提供。
- **Coding Plan 端点是否进 preset 默认**: 按 CLAUDE.md 2026-07-08 决策，preset JSON 默认不带 cp 子分支。本文档仅提供端点真值，不改架构决策。
- **z.ai `glm-4-32b-0414-128k`**: 国内 docs.bigmodel.cn 无此页，国内端点调此 id 行为未知（推测: 报 model not found，需 `需要:` 用户实测）。建议默认不加。
- **下线模型保留策略**: preset 保留 `glm-5.1`/`glm-5` 历史 id（调用自动迁 5.2），为用户既有配置兼容；新增 `glm-4.5`/`glm-4.5-x` 等下线项则不建议。
