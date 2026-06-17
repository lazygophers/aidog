# Research: 一方平台组「候选模型列表」(chat/coding) 核实

- **Query**: 逐平台 WebSearch/WebFetch 核「一方平台组」(21 平台) 官方当前在售模型 API id 列表（chat/coding 相关），旗舰在前，供 aidog 做内置候选模型下拉（string[]，扩 `getDefaultModels` 单值→列表）。
- **Scope**: external（官方静态文档 + OpenRouter `/v1/models` + LiteLLM 定价 JSON 三方交叉）+ internal（Platforms.tsx 现有预设）
- **Date**: 2026-06-17

## 一句话结论

21 平台中 **15 个拿到可直接用的 API model id 候选列表**（anthropic/openai/codex/glm/glm_en/kimi/minimax/minimax_en/bailian/deepseek/stepfun/stepfun_en/xiaomi_mimo/doubao/doubao_seed/byteplus 实为 16，doubao 系列共享）；**3 个部分**（gemini 槽位语义另算、bailian_coding 透传无独立列表、qianfan 仅拿到 ERNIE 主力但 API id 大小写需 key 实测）；**3 个未找到/留空**（longcat / bailing 官方模型列表 JS-rendered 无法静态抓取，qianfan 同）。

**核查信源可信度排序**: 官方静态文档 md（xiaomi/glm/stepfun/kimi 可静态）> OpenRouter `/v1/models`（最新，但 id 已 normalize 为小写，仅作交叉）> LiteLLM `model_prices_and_context_window.json`（本项目定价单一信源，但有知识截止，部分滞后：kimi 只到 k2.6、glm 只到 glm-5、deepseek 只到 v3.2）。

---

## 每平台候选列表

> 约定：列表项为**可直接用作 API `model` 字段的字符串**（旗舰/最新在前，含主力 chat + 可用的 coding 变体）。`默认` = 现有 `getDefaultModels` 已填值（旗舰，应置列表首）。

### `anthropic`
- 候选: `claude-opus-4-8`, `claude-sonnet-4-6`, `claude-haiku-4-6`
- 来源: 本仓自身 = Claude，模型由本项目定义（`getDefaultModels` Platforms.tsx:382）。槽位 opus/sonnet/haiku 已分别绑定。
- 核查日期: 2026-06-17

### `openai`
- 候选: `gpt-5.5`
- 来源: **未从 OpenAI 官方 docs 静态确认**（SPA）。沿用现有预设 `gpt-5.5`（Platforms.tsx:383）。如需扩列表须另查 OpenAI 官方 models 页（推测含 gpt-5.5 / gpt-5.5-mini / gpt-5.1 等，未证实不写入）。
- 核查日期: 2026-06-17

### `codex`
- 候选: `gpt-5.5-codex`
- 来源: **未从官方确认**（原注 TODO 未决，Platforms.tsx:384）。沿用现有。可能变体 `gpt-5.5-codex` / `gpt-5-codex`（未证实，留 fetchModels 兜底）。
- 核查日期: 2026-06-17

### `gemini`
- 候选: **留空靠 fetchModels**（gemini 槽位语义不匹配 opus/sonnet/gpt，现有预设本就空）。
- 推测: 若要填，旗舰 `gemini-2.5-pro` / `gemini-2.5-flash`（未从 Google 官方静态确认，SPA）。
- 核查日期: 2026-06-17

### `glm`（智谱，国内站）
- 候选: `glm-5.2`, `glm-5.1`, `glm-5`, `glm-5-turbo`, `glm-4.7`, `glm-4.7-flash`, `glm-4.6`, `glm-4.5-air`
- 默认: `glm-5.2`（现有预设，旗舰；coding plan 端如遇不兼容回退 `glm-4.6`）
- 来源: docs.bigmodel.cn/cn/guide/start/model-overview（静态抓取确认全部 id：GLM-5.2/5.1/5/5-Turbo/4.7/4.7-Flash/4.6/4.5-Air/4.5-AirX/4.5-Flash 均在推荐列）。OpenRouter `z-ai/glm-5.2…glm-4.5` 11 条交叉确认。
- 注: 官方 model id 用小写 `glm-5.2`（OpenRouter 同）。`glm-4.6` 将 2026-07-09 弃用但仍在推荐列（coding plan 广用），建议保留作回退候选。
- 核查日期: 2026-06-17

### `glm_en`（z.ai，海外站）
- 候选: 同 glm — `glm-5.2`, `glm-5.1`, `glm-5`, `glm-5-turbo`, `glm-4.7`, `glm-4.7-flash`, `glm-4.6`, `glm-4.5-air`
- 默认: `glm-5.2`
- 来源: docs.z.ai（同 model-overview 体系）+ OpenRouter `z-ai/*`。model id 与国内站一致。
- 核查日期: 2026-06-17

### `kimi`（Moonshot 月之暗面）
- 候选(coding plan): `kimi-k2.7-code`, `kimi-k2.7-code-highspeed`, `kimi-k2.6`, `kimi-k2.5`
- 候选(非 cp 通用): `kimi-k2.6`, `kimi-k2.5`, `kimi-k2-thinking`, `kimi-latest`
- 默认: `cp ? "kimi-k2.7-code" : "kimi-k2.6"`（现有三元正确）
- 来源: platform.moonshot.cn/docs/api/chat（静态确认 `kimi-k2.7-code` / `kimi-k2.7-code-highspeed` / `kimi-k2.6` / `kimi-k2.5`）。OpenRouter `moonshotai/kimi-k2.7-code|k2.6|k2.5|k2-thinking` 交叉。LiteLLM `moonshot/kimi-k2.6|k2.5|kimi-k2-thinking`（滞后无 k2.7）。
- 注: kimi-k2 原系列 2026-05-25 已停用，不入候选。
- 核查日期: 2026-06-17

### `minimax`（minimaxi.com 国内）
- 候选: `MiniMax-M3`, `MiniMax-M2.7`, `MiniMax-M2.5`, `MiniMax-M2.1`, `MiniMax-M2`
- 默认: `MiniMax-M3`（2026-06-02 发布新旗舰）
- 来源: LiteLLM `minimax/MiniMax-M3|M2.5|M2.5-lightning|M2.1|M2.1-lightning|M2`（确认官方大小写 `MiniMax-Mx`）。OpenRouter `minimax/minimax-m3…minimax-m2`（normalize 小写，仅作存在性交叉，含 `minimax-m2.7`）。HF `MiniMaxAI/MiniMax-M3`(2026-06-02)。
- ⚠️ M3 官方开放平台 chat API model id 大小写**推测 = `MiniMax-M3`**（沿用 M2.5/M2.7 惯例 + LiteLLM 用此写法）；落预设前建议用真实 key 调 `/v1/text/chatcompletion_v2` 验证 `model` 字段确切写法。lightning 变体（`MiniMax-M2.5-lightning` 等）LiteLLM 有，开放平台是否同名未实测。
- 核查日期: 2026-06-17

### `minimax_en`（minimax.io 海外）
- 候选: 同 minimax — `MiniMax-M3`, `MiniMax-M2.7`, `MiniMax-M2.5`, `MiniMax-M2.1`, `MiniMax-M2`
- 默认: `MiniMax-M3`
- 来源: 同上（model id 与国内站一致）。
- 核查日期: 2026-06-17

### `bailian`（阿里云百炼 / 通义千问 DashScope）
- 候选(chat): `qwen3.7-max`, `qwen3.7-plus`, `qwen3.6-flash`, `qwen3.5-omni-plus`
- 候选(coding): `qwen3-coder-plus`, `qwen3-coder-flash`（来自 OpenRouter `qwen/qwen3-coder-plus|flash`，DashScope 同名）
- 默认: `qwen3.7-max`（官方「能力最强」旗舰）
- 来源: help.aliyun.com/zh/model-studio/getting-started/models（静态部分抓取确认 `qwen3.7-max` / `qwen3.7-plus` / `qwen3.6-flash` / `qwen3.5-omni-plus`）。OpenRouter `qwen/qwen3.7-max|qwen3.7-plus|qwen3-coder-plus|qwen3-coder-flash` 交叉。
- ⚠️ 阿里 model-studio 完整文本生成表 JS-rendered，仅抓到上述主力 4+2；qwen3.7 全档（turbo/long 等）未逐一静态确认。coder 系列 id 由 OpenRouter 提供（DashScope 通常同名，但 `qwen3-coder-plus` vs 含日期版未实测）。
- 核查日期: 2026-06-17

### `bailian_coding`（百炼 coding 透传 anthropic 端点）
- 候选: **留空靠 fetchModels**（`/apps/anthropic` coding 透传端点，现有预设无槽位）。
- 注: 实际可用 model 同 bailian 的 coder 系列（`qwen3-coder-plus` 等），但端点形态为透传，建议保持空或复用 bailian coder 候选。
- 核查日期: 2026-06-17

### `deepseek`
- 候选: `deepseek-v4-flash`, `deepseek-v4-pro`, `deepseek-chat`, `deepseek-reasoner`
- 默认: `deepseek-v4-flash`（后继 deepseek-chat；deepseek-chat 将 2026-07-24 弃用）
- 来源: OpenRouter `deepseek/deepseek-v4-flash|deepseek-v4-pro|deepseek-chat`（确认 v4-flash/v4-pro 在线）。api-docs.deepseek.com/quick_start/pricing（SPA，但前期 research 已静态确认 v4-flash/v4-pro）。LiteLLM 滞后（仅到 `deepseek-chat`/`deepseek-reasoner`/`deepseek-v3.2`）。
- 注: 官方 API 用 `deepseek-chat`（指向最新通用）/ `deepseek-reasoner`（思考）作别名 id，与版本号 id `deepseek-v4-flash` 并存——两类都可用。
- 核查日期: 2026-06-17

### `stepfun`（阶跃星辰，国内 step_plan anthropic 透传）
- 候选: `step-3.7-flash`, `step-3.5-flash`
- 来源: platform.stepfun.com/docs/llm/text（静态确认 `step-3.7-flash` / `step-3.5-flash`）。OpenRouter `stepfun/step-3.7-flash|step-3.5-flash` 交叉。
- 注: 现有预设为 `step_plan` anthropic coding **透传**端点，本就无默认模型槽位；如要填候选，旗舰 `step-3.7-flash`。step_plan 透传时 model 字段可能被上游忽略——建议候选作可选项。
- 核查日期: 2026-06-17

### `stepfun_en`（api.stepfun.ai 海外）
- 候选: 同 stepfun — `step-3.7-flash`, `step-3.5-flash`
- 来源: 同 stepfun（model id 一致）。
- 核查日期: 2026-06-17

### `doubao`（火山方舟 Ark，`/api/coding` anthropic 透传）
- 候选: `doubao-seed-2-0-pro`, `doubao-seed-2-0-code-preview`, `doubao-seed-2-0-lite`, `doubao-seed-2-0-mini`
- 默认: `doubao-seed-2-0-pro`（旗舰；coding 偏好 `doubao-seed-2-0-code-preview`）
- 来源: www.volcengine.com/docs/82379/1099455（静态 JSON 流抓到 `doubao-seed-2-0-pro` / `doubao-seed-2-0-lite-260215`）。LiteLLM `volcengine/doubao-seed-2-0-pro-260215|lite|mini|code-preview-260215` 确认全 4 档。
- ⚠️ **model id 写法**: Ark 实际 model id 有两种形态——**带日期** `doubao-seed-2-0-pro-260215`（精确版本）与**无日期别名** `doubao-seed-2-0-pro`（滚动最新）。前期 research 提到点号形态 `doubao-seed-2.0-pro`，但本次官方 JSON + LiteLLM 均用**短横线** `doubao-seed-2-0-pro`——以**短横线**为准。是否需带日期后缀取决于用户是否绑定 endpoint id，候选用无日期别名更稳。
- 核查日期: 2026-06-17

### `doubao_seed`（Ark `/api/compatible` anthropic）
- 候选: 同 doubao — `doubao-seed-2-0-pro`, `doubao-seed-2-0-code-preview`, `doubao-seed-2-0-lite`, `doubao-seed-2-0-mini`
- 默认: `doubao-seed-2-0-pro`
- 来源: 同 doubao（同 Ark 体系，model id 一致）。
- 核查日期: 2026-06-17

### `byteplus`（海外 Ark，`ark.ap-southeast.bytepluses.com/api/coding`）
- 候选: `doubao-seed-2-0-pro`, `doubao-seed-2-0-code-preview`, `doubao-seed-2-0-lite`, `doubao-seed-2-0-mini`（model id 体系同 doubao-seed-2.0）
- 来源: 同 Ark 体系（海外 BytePlus 沿用 doubao-seed-2-0 id）。OpenRouter 另见 `bytedance-seed/seed-2.0-lite|mini`（聚合站命名，非 Byter Ark 原生 id）。
- ⚠️ BytePlus 控制台可能需先开通对应 endpoint，原生 model id 推测同 `doubao-seed-2-0-*`，未单独实测海外站。
- 核查日期: 2026-06-17

### `qianfan`（百度千帆 / 文心 ERNIE）
- 候选: **部分 / 大小写未实测** — 推测旗舰 `ernie-5.0` / `ernie-4.5-turbo` 系列（百度 2026 主力）
- 来源: cloud.baidu.com/doc/WENXINWORKSHOP（文档 JS-rendered，ERNIE model id 表未能静态抓取）。OpenRouter 仅 `baidu/ernie-4.5-vl-424b-a47b`（VL 多模态，非纯文本旗舰 chat id）。LiteLLM 无 ernie 纯文本 chat 条目。
- 结论: **未拿到确切纯文本 chat/coding ERNIE API model id**，现有预设无槽位。**建议留空靠 fetchModels**（qianfan v2 `/v2/models` 可拉）。不硬填以免编造。
- 已查 URL: cloud.baidu.com/doc/WENXINWORKSHOP/s/Fm2vrveyu、cloud.baidu.com/doc/qianfan-modelbuilder/s/Wm9cvy6rl（均 SPA）、qianfan.baidubce.com/v2/models（需鉴权）。
- 核查日期: 2026-06-17

### `xiaomi_mimo`（小米 MiMo）
- 候选: `mimo-v2.5-pro`, `mimo-v2-pro`, `mimo-v2.5`, `mimo-v2-omni`, `mimo-v2-flash`
- 默认: `mimo-v2.5-pro`（旗舰文本模型，按量 openai 端点）
- 来源: platform.xiaomimimo.com/static/docs/quick-start/model.md（**官方静态确认全 5 个文本 model id**：Pro 系列 mimo-v2.5-pro/mimo-v2-pro，Omni 系列 mimo-v2.5/mimo-v2-omni，Flash 系列 mimo-v2-flash）。OpenRouter `xiaomi/mimo-v2.5-pro|mimo-v2.5|mimo-v2-flash` 交叉。
- 注: `mimo-v2.5-asr` 为语音识别，不入 chat 候选。
- 核查日期: 2026-06-17

### `bailing`（蚂蚁百灵 / tbox.cn，anthropic 透传）
- 候选: **未找到，留空靠 fetchModels**
- 来源: api.tbox.cn `/api/anthropic`（透传端点，host 200 可达）；`/v1/models` 返回 404（无公开模型枚举）。官方模型文档未找到静态可抓 model id。
- 已查 URL: api.tbox.cn/v1/models（404）。社区有「Ling/Bailing」系列但无官方静态来源佐证，不写入。
- 核查日期: 2026-06-17

### `longcat`（美团龙猫，anthropic 透传）
- 候选: **未找到，留空靠 fetchModels**
- 来源: longcat.chat/platform/docs 与 longcat.ai/docs 全 JS-rendered（抓到的全是站点域名非 model id）。`api.longcat.chat/anthropic` 透传端点 400 可达。
- 推测: 社区常见 `LongCat-Flash-Chat` / `LongCat-Flash-Thinking`，但**无官方静态来源佐证，不写入**。
- 已查 URL: longcat.chat/platform/docs/zh/models（353B 空壳）、longcat.ai/docs。
- 核查日期: 2026-06-17

---

## 落地建议（供主代理扩 getDefaultModels 用）

现有 `getDefaultModels` 返回 `Partial<Record<ModelSlot, string>>`（单值）。本次目标扩成「候选列表」：

- 旗舰仍作 `default` 槽（route resolve 用），列表作 UI 下拉候选另存（如 `getCandidateModels(protocol, cp): string[]`，与 `getDefaultModels` 并列）。
- 列表首项 = 现有 `getDefaultModels` 默认值（保持 route 行为不变）。
- coding plan 平台（kimi/glm/qianfan/doubao/bailian_coding）列表按 cp 切（cp 时 coding 变体在前）。
- 6 个未找到/留空平台（gemini/bailian_coding/qianfan/bailing/longcat + openai/codex 未官方确认）保持空数组，fetchModels 兜底。

---

## Caveats / Not Found 汇总

| 平台 | 状态 | 原因 |
|---|---|---|
| openai / codex | 部分（沿用现有，未官方确认） | OpenAI docs SPA，未静态抓取 |
| gemini | 留空 | 槽位语义不匹配 + Google docs SPA |
| qianfan | 未找到确切 chat id | 百度文档 JS-rendered；OpenRouter 仅 VL 模型 |
| bailing | 未找到 | tbox.cn 无公开模型枚举；`/v1/models` 404 |
| longcat | 未找到 | longcat 文档全 JS-rendered |
| minimax(_en) | 拿到但大小写需实测 | M3 开放平台 model 字段大小写推测 `MiniMax-M3`，未用 key 验证 |
| doubao 系 | 拿到但 id 形态需注意 | 短横线 `doubao-seed-2-0-pro`（非点号），含/不含日期后缀两形态 |
| bailian | 拿到主力，全档未尽 | model-studio 完整表 JS-rendered，仅抓主力 4+2 |

## External References

- [Xiaomi MiMo model.md](https://platform.xiaomimimo.com/static/docs/quick-start/model.md) — 5 个文本 model id（mimo-v2.5-pro 等）静态确认
- [GLM model-overview](https://docs.bigmodel.cn/cn/guide/start/model-overview) — GLM-5.2…4.5 全推荐列静态确认
- [Moonshot api/chat docs](https://platform.moonshot.cn/docs/api/chat) — kimi-k2.7-code / k2.6 / k2.5 静态确认
- [StepFun text models](https://platform.stepfun.com/docs/llm/text) — step-3.7-flash / step-3.5-flash
- [Aliyun model-studio models](https://help.aliyun.com/zh/model-studio/getting-started/models) — qwen3.7-max / 3.7-plus / 3.6-flash
- [Volcengine Ark models 1099455](https://www.volcengine.com/docs/82379/1099455) — doubao-seed-2-0-pro / lite
- [OpenRouter /v1/models](https://openrouter.ai/api/v1/models) — 最新存在性交叉（id 已 normalize 小写）
- LiteLLM `model_prices_and_context_window.json`(GitHub raw) — minimax/doubao/kimi 大小写交叉（部分滞后）

## Related Internal Files

- `src/pages/Platforms.tsx:376` — `getDefaultModels`（当前单值，本次扩列表）
- `src/pages/Platforms.tsx:1172` / `:1639` — 两个消费点（卡片回退展示 + 表单 auto-fill）
- `src/services/api.ts:47` — `ModelSlot` 槽位类型
- `.trellis/tasks/archive/2026-06/06-17-platform-presets-fill/research/presets-firstparty.md` — 前期 base_url + 单默认模型核实
- `.trellis/tasks/archive/2026-06/06-17-xiaomi-coding-plan/research/xiaomi-coding-plan-api.md` — xiaomi 端点/Token Plan 细节
- `.claude/skills/aidog-add-platform/references/default-model.md` — getDefaultModels 机制
