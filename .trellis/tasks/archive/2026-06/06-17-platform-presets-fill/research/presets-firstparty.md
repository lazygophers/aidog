# Research: 一方平台组预设 base_url + 默认模型核实

- **Query**: 逐平台核实「一方平台组」(21 平台) 官方 base_url 与当前推荐默认模型 API id，校正过时值/补空缺
- **Scope**: external (官方文档/HF/LiteLLM/OpenRouter) + internal (Platforms.tsx 现有预设)
- **Date**: 2026-06-17

## 一句话结论

base_url **全部仍有效**(13 个核心 host 已 liveness 探测,无 DNS/失效);**需改默认模型 1 个**(minimax/minimax_en: M2.7 已被 2026-06-02 发布的 **MiniMax-M3** 取代);**默认模型空缺可补 4+ 个**(xiaomi_mimo / stepfun / doubao / doubao_seed / qianfan);**xiaomi_mimo openai 端点确定可落地**(`https://api.xiaomimimo.com/v1` + 默认 `mimo-v2.5-pro`)。其余平台保持原值或本身无默认模型槽位。

---

## 核查方法与信源

- base_url: 各 host 直接 `POST …/chat-or-messages` 探测,返回 401/400/200/404/405 均证明 host 可达且端点存在(非 DNS 失败)。
- 默认模型: 优先官方文档静态 md(xiaomi/deepseek/glm/bailian/stepfun 可静态抓取);JS-rendered SPA(kimi/minimax/longcat/doubao 控制台)改用 **LiteLLM `model_prices_and_context_window.json`**(本项目定价单一信源,见 memory `pricing-github-single-source`)+ **OpenRouter `/v1/models`**(聚合真实 provider model id)+ **HuggingFace** 发布时间线 三方交叉核实。

---

## 主表(每平台一行)

| 平台key | 现有base_url(主端点) | 核实后base_url(变更?) | 现有默认模型 | 核实后默认模型(API id) | 来源 | 日期 |
|---|---|---|---|---|---|---|
| anthropic | `https://api.anthropic.com` (anthropic) | 同,N | claude-opus-4-8 / sonnet-4-6 / haiku-4-6 | 保持(本仓自身=Claude,模型由本项目定义) | 项目内部 | 2026-06-17 |
| openai | `https://api.openai.com/v1` (openai) | 同,N | gpt-5.5 | 保持(未核 OpenAI 官方,非本次重点) | 未核 | 2026-06-17 |
| codex | `https://api.openai.com/v1` (openai) | 同,N | gpt-5.5-codex | 保持(同上,原 TODO 未决) | 未核 | 2026-06-17 |
| gemini | `https://generativelanguage.googleapis.com` (gemini) | 同,N | (无槽位,空) | 保持空(gemini 槽位语义不匹配) | — | 2026-06-17 |
| glm | `https://open.bigmodel.cn/api/paas/v4` (openai) + `/api/anthropic` | 同,N (401 可达) | glm-4.6 | **可保持 glm-4.6**;新旗舰=glm-5.2(见 §GLM) | docs.bigmodel.cn/cn/guide/start/model-overview | 2026-06-17 |
| glm_en | `https://api.z.ai/api/paas/v4` (openai) + `/api/anthropic` | 同,N (401 可达) | glm-4.6 | 同 glm:保持 glm-4.6 | docs.z.ai | 2026-06-17 |
| kimi | `https://api.kimi.com/coding/v1`(cp) / `https://api.moonshot.cn/v1`(非cp) + `/anthropic` | 同,N (cp 端点 400 可达) | cp?`kimi-k2.7-code`:`kimi-k2.6` | **保持**(两者 OpenRouter 均确认存在) | openrouter.ai/api/v1/models | 2026-06-17 |
| minimax | `https://api.minimaxi.com/v1` (openai) + `/anthropic` | 同,N (200 可达) | MiniMax-M2.7 | **改 → MiniMax-M3**(2026-06-02 发布,新旗舰) | HF MiniMaxAI + openrouter minimax/minimax-m3 | 2026-06-17 |
| minimax_en | `https://api.minimax.io/v1` (openai) + `/anthropic` | 同,N (200 可达) | MiniMax-M2.7 | **改 → MiniMax-M3**(同上) | 同上 | 2026-06-17 |
| bailian | `https://dashscope.aliyuncs.com/compatible-mode/v1` (openai) + `/apps/anthropic` | 同,N (400 可达) | qwen3.7-max | **保持**(官方"能力最强"旗舰仍 qwen3.7-max) | help.aliyun.com/zh/model-studio/models | 2026-06-17 |
| bailian_coding | `https://coding.dashscope.aliyuncs.com/apps/anthropic` (anthropic) | 同,N | (无槽位,空) | 保持空(coding 透传) | — | 2026-06-17 |
| deepseek | `https://api.deepseek.com/v1` (openai) + `/anthropic` | 同,N (401 可达) | deepseek-v4-flash | **保持**(官方 pricing 页确认 v4-flash/v4-pro) | api-docs.deepseek.com/quick_start/pricing | 2026-06-17 |
| stepfun | `https://api.stepfun.com/step_plan` (anthropic) | 同,N (host 401 可达) | (无槽位,空) | 可补 `step-3.7-flash`(当前 flash 旗舰;但 step_plan 为 coding 透传,建议保持空) | platform.stepfun.com/docs/llm/text | 2026-06-17 |
| stepfun_en | `https://api.stepfun.ai/step_plan` (anthropic) | 同,N | (无槽位,空) | 同 stepfun:建议保持空 | — | 2026-06-17 |
| doubao | `https://ark.cn-beijing.volces.com/api/coding` (anthropic) | 同,N (ark host 401 可达) | (无槽位,空) | 可补 `doubao-seed-2.0-pro`(见 §Doubao);coding 透传建议保持空 | LiteLLM doubao-seed-2-0-* | 2026-06-17 |
| doubao_seed | `https://ark.cn-beijing.volces.com/api/compatible` (anthropic) | 同,N | (无槽位,空) | 可补 `doubao-seed-2.0-pro` | 同上 | 2026-06-17 |
| byteplus | `https://ark.ap-southeast.bytepluses.com/api/coding` (anthropic) | 同,N (未单独探测;同 ark 体系) | (无槽位,空) | 保持空(coding 透传,海外 seed 模型 id 同 doubao-seed-2.0) | — | 2026-06-17 |
| qianfan | `https://qianfan.baidubce.com/v2/coding`(cp openai) / `/anthropic/coding`(anthropic) | 同,N (未单独探测) | (无槽位,空) | 可补 ERNIE 旗舰(未拿到确切 API id,见 Caveats) | — | 2026-06-17 |
| xiaomi_mimo | `https://api.xiaomimimo.com/anthropic` (anthropic, 仅此一端点) | 同,N (按量 host 400 可达);**缺 openai 端点** | (无槽位,空) | **补 openai 端点 + 默认 `mimo-v2.5-pro`**(见 §xiaomi_mimo) | platform.xiaomimimo.com 官方 md | 2026-06-17 |
| bailing | `https://api.tbox.cn/api/anthropic` (anthropic) | 同,N (200 可达) | (无槽位,空) | 保持空(透传) | — | 2026-06-17 |
| longcat | `https://api.longcat.chat/anthropic` (anthropic) | 同,N (400 可达) | (无槽位,空) | 保持空(透传;文档 JS-rendered 未拿到 API model id) | — | 2026-06-17 |

> 说明:表中"无槽位,空"指该平台在 `getDefaultModels`(Platforms.tsx:374-393)中**本就未列任何 ModelSlot**,保持现状(向后兼容,fetchModels 兜底)。本次仅 glm/glm_en/kimi/minimax/minimax_en/bailian/deepseek 7 个在 presets 中有默认模型,其中**只有 minimax/minimax_en 需改值**。

---

## xiaomi_mimo openai 端点建议(确定可落地)

现状:`Platforms.tsx:220-222` 仅有 anthropic 单端点 `https://api.xiaomimimo.com/anthropic`,**缺 openai 端点**。

### 建议补充端点对象(按量 host)

```ts
xiaomi_mimo: [
  { protocol: "anthropic", base_url: "https://api.xiaomimimo.com/anthropic", client_type: "claude_code" }, // 现有,保留
  { protocol: "openai", base_url: "https://api.xiaomimimo.com/v1", client_type: "codex_tui" }, // 新增
],
```

- **base_url**: `https://api.xiaomimimo.com/v1`(含 `/v1` 版本前缀,符合 url-construction-rule;最终 = base_url + `/chat/completions`,无额外拼接)。
- **client_type**: `codex_tui`(openai/codex 协议惯例,对齐 glm/minimax/deepseek 的 openai 端点)。
- **默认模型**: `mimo-v2.5-pro`(官方旗舰文本模型,first-api-call.md 全部 SDK/curl 示例默认 model)。可填到 `getDefaultModels.xiaomi_mimo = { default: "mimo-v2.5-pro" }`。

### host 取舍理由(按量 vs Token Plan)

| 选项 | host | 取舍 |
|---|---|---|
| **按量(选用)** | `api.xiaomimimo.com/v1` | API Key `sk-`,通用,绝大多数用户路径;与现有 anthropic 端点同 host,一致 |
| Token Plan(订阅) | `token-plan-{cn,sgp,ams}.xiaomimimo.com/v1` | API Key `tp-`(与 `sk-` 不可混用),分三地集群,仅订阅用户;且需 coding_plan 变体三元(参考 kimi/qianfan)。属可选增强,非本次必须 |

**结论**: 预设默认补**按量 host**(单一确定值,与现有 anthropic 端点同源)。Token Plan 多集群是可选增强项,若要支持订阅用户再加 `cp ? token-plan-cn… : api.xiaomimimo…` 三元(参考归档 research `xiaomi-coding-plan-api.md` §4.4)。

- xiaomi 文本模型全集(model.md):`mimo-v2.5-pro`(旗舰) / `mimo-v2-pro` / `mimo-v2.5` / `mimo-v2-omni` / `mimo-v2-flash`。

---

## §GLM —— glm-4.6 是否要改

- **官方现状**(docs.bigmodel.cn model-overview):**最新旗舰 = GLM-5.2**;推荐模型表含 GLM-5.2(最新旗舰)/ GLM-5.1 / GLM-5 / GLM-5-Turbo / **GLM-4.6(标"超强性能",仍在推荐列)** / GLM-4.7。站点有专门"迁移至 GLM-5.2"指南。
- 现有 preset 注释:"glm-4.6 将 2026-07-09 弃用,后继 glm-5.2;保留 4.6 因 coding plan 仍广用,到期前替换"。
- **建议**: glm/glm_en **可保持 glm-4.6**(coding plan 端点广用,官方仍推荐且未下线);若要前瞻可改 `glm-5.2`。**非必须改**。OpenRouter `z-ai/*` 确认 glm-4.6 / glm-5 / glm-5.1 / glm-5.2 / glm-4.7 全部在线。
- ⚠️ 注意原注释的弃用日期 2026-07-09 临近(距今 ~3 周),若任务目标是"前瞻补全",建议改 `glm-5.2`;若"稳态校正",保持 4.6。需主代理裁定。

---

## §Doubao / Doubao_seed —— 默认模型(可选补)

- LiteLLM 确认豆包最新 = **doubao-seed-2.0 系列**:`doubao-seed-2-0-pro` / `-lite` / `-mini` / `-code-preview`(litellm 用 dashed-date 后缀 `…-260215`;Ark 实际 model id 用点号 `doubao-seed-2.0-pro`)。
- doubao(`/api/coding` anthropic)与 doubao_seed(`/api/compatible` anthropic)在 preset 中**均无默认模型槽位**。两者皆 coding/透传形态,**建议保持空**;若补,旗舰填 `doubao-seed-2.0-pro`(coding 偏好 `doubao-seed-2.0-code-preview`)。
- byteplus(海外 ark)模型 id 体系同 doubao-seed-2.0,base_url `https://ark.ap-southeast.bytepluses.com/api/coding` 不变。

---

## §其它已核实但保持原值

- **deepseek**: 官方 pricing 页确认 `deepseek-v4-flash`(后继 deepseek-chat)/ `deepseek-v4-pro`;现有 `deepseek-v4-flash` **正确,保持**。OpenRouter 另有 `deepseek-v3.2` 等旧版。
- **bailian**: 官方 model-studio "能力最强"旗舰仍 `qwen3.7-max`;现有值**正确,保持**。
- **kimi**: OpenRouter 确认 `kimi-k2.7-code`(coding)与 `kimi-k2.6`(通用)均在线;现有三元 `cp?"kimi-k2.7-code":"kimi-k2.6"` **完全正确,保持**。
- **stepfun**: 当前 flash 旗舰 `step-3.7-flash`;但 stepfun preset 是 `step_plan` anthropic coding 端点(透传),无默认模型槽位,**建议保持空**。

---

## Caveats / Not Found

- **openai / codex / gemini**: 本次未核(非"国内一方"重点,且 OpenAI/Google 官方文档 SPA)。openai=`gpt-5.5`、codex=`gpt-5.5-codex`(原注 TODO 未从官方确认)、gemini 空,**均保持原值**。如需核实须另查 OpenAI/Google 官方。
- **qianfan(ERNIE)**: 百度 cloud.baidu.com 文档全 JS-rendered,**未拿到确切 ERNIE 旗舰 chat API id**;LiteLLM/OpenRouter 仅有 `ernie-4.5-vl-424b-a47b`(VL)非纯文本旗舰。qianfan preset 无默认模型槽位,**保持空,未找到不硬填**。base_url `https://qianfan.baidubce.com/v2/coding` 与 `/anthropic/coding` 未单独 liveness 探测(但同百度体系,推测可达)。
- **longcat**: longcat.chat/platform/docs 与 openapi.json 全 JS-rendered/非 JSON,**未拿到 API model id**(社区常见 `LongCat-Flash-Chat` 但无官方静态来源佐证,不写入)。preset 为 anthropic 透传无默认模型槽位,**保持空**。base_url `https://api.longcat.chat/anthropic` 400 可达。
- **MiniMax-M3 vs M2.7**: M3 由 HF(MiniMaxAI/MiniMax-M3,2026-06-02)+ OpenRouter(`minimax/minimax-m3`,ctx 1048576)双源确认存在且为最新;但 **minimaxi.com/minimax.io 开放平台控制台 SPA 未能静态确认 M3 的开放平台 chat API model id 大小写**。OpenRouter 聚合 id 为 `minimax-m3`,**推测官方开放平台 model id = `MiniMax-M3`**(沿用 M2.5/M2.7 的 `MiniMax-Mx` 大小写惯例)。落预设前建议用真实 key 调一次 `/v1/text/chatcompletion_v2` 验证 model 字段确切写法。
- **bailing(tbox.cn)**: 200 可达,anthropic 透传,无默认模型,保持空。
- base_url 未逐一探测的:bailian_coding / stepfun_en / byteplus / qianfan / doubao_seed(与已探测同源 host 体系,推测可达,标"未单独探测")。

## External References

- [Xiaomi MiMo first-api-call](https://platform.xiaomimimo.com/static/docs/quick-start/first-api-call.md) — openai base_url `/v1` + 默认 `mimo-v2.5-pro`
- [Xiaomi MiMo model.md](https://platform.xiaomimimo.com/static/docs/quick-start/model.md) — 文本模型全集
- [DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing) — deepseek-v4-flash / v4-pro
- [GLM model-overview](https://docs.bigmodel.cn/cn/guide/start/model-overview) — GLM-5.2 旗舰 + glm-4.6 仍推荐
- [Bailian model-studio](https://help.aliyun.com/zh/model-studio/models) — qwen3.7-max 能力最强
- [StepFun text models](https://platform.stepfun.com/docs/llm/text) — step-3.7-flash
- HuggingFace `MiniMaxAI/MiniMax-M3`(2026-06-02) — MiniMax 新旗舰
- OpenRouter `/v1/models` — kimi-k2.7-code / kimi-k2.6 / minimax-m3 / z-ai/glm-5.2 / deepseek-v4-* / doubao-seed
- LiteLLM `model_prices_and_context_window.json` — 交叉核实 model id(本项目定价单一信源)

## Related Internal Files

- `src/pages/Platforms.tsx:150-365` — `getDefaultEndpoints`(base_url 预设)
- `src/pages/Platforms.tsx:371-395` — `getDefaultModels`(默认模型预设)
- `.trellis/tasks/archive/2026-06/06-17-xiaomi-coding-plan/research/xiaomi-coding-plan-api.md` — xiaomi Token Plan 三集群端点 + tp- key 细节(可选增强参考)
