import type { Protocol, PlatformEndpoint, ModelSlot, ClientType } from "../../services/api";
import { PROTOCOLS } from "./constants";

/** 根据端点协议返回推荐的默认客户端类型 */
export function defaultClientForProtocol(protocol: Protocol): ClientType {
  switch (protocol) {
    case "anthropic": return "claude_code";
    case "openai": return "codex_tui";
    default: return "default";
  }
}

/** 根据 ProtocolOption 生成默认端点（含 coding_plan 标记）
 *  数据来源：cc-switch 各平台官方配置 */
export function getDefaultEndpoints(protocol: Protocol, codingPlan?: boolean): PlatformEndpoint[] {
  const cp = !!codingPlan;
  const base: Partial<Record<Protocol, PlatformEndpoint[]>> = {
    // ── 官方 ──
    anthropic: [
      { protocol: "anthropic", base_url: "https://api.anthropic.com", client_type: "claude_code" },
    ],
    openai: [
      { protocol: "openai", base_url: "https://api.openai.com/v1", client_type: "codex_tui" },
    ],
    codex: [
      { protocol: "openai", base_url: "https://api.openai.com/v1", client_type: "codex_tui" },
    ],
    gemini: [
      { protocol: "gemini", base_url: "https://generativelanguage.googleapis.com" },
    ],

    // ── 国内官方 ──
    // GLM(智谱)：openai 与 anthropic 端点同处 open.bigmodel.cn，同一把 key 通用（不像 Kimi 按 host
    // 拆 coding/常规）。coding plan 时两端点均标 coding_plan=true，使 anthropic(Claude Code)入站走
    // anthropic coding 端点原协议直发，不再被转换成 openai 走 /api/coding/paas/v4。
    glm: [
      { protocol: "openai", base_url: cp ? "https://open.bigmodel.cn/api/coding/paas/v4" : "https://open.bigmodel.cn/api/paas/v4", client_type: "codex_tui", coding_plan: cp },
      { protocol: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", client_type: "claude_code", coding_plan: cp },
    ],
    glm_en: [
      { protocol: "openai", base_url: "https://api.z.ai/api/paas/v4", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://api.z.ai/api/anthropic", client_type: "claude_code" },
    ],
    // coding plan：key 仅对 coding host(api.kimi.com/coding)有效，且该 host 无 anthropic 端点
    // (api.kimi.com/coding/anthropic → 404)；常规 anthropic 端点(api.moonshot.cn/anthropic)需另一把
    // 常规 key，用 coding key 打过去 401。故 cp 时只给唯一可用的 openai coding 端点(anthropic 入站
    // 经 convert_request 转 openai)。非 cp 时常规 key 在两个 host 通用，保留双端点。
    kimi: cp ? [
      { protocol: "openai", base_url: "https://api.kimi.com/coding/v1", client_type: "claude_code", coding_plan: true },
    ] : [
      { protocol: "openai", base_url: "https://api.moonshot.cn/v1", client_type: "claude_code" },
      { protocol: "anthropic", base_url: "https://api.moonshot.cn/anthropic", client_type: "claude_code" },
    ],
    // MiniMax(海螺)：coding plan 与常规共用同一 host(api.minimaxi.com / api.minimax.io)与 key，
    // 配额查询走 /v1/api/openplatform/coding_plan/remains(quota.rs)。cp 时仅在端点标 coding_plan=true，
    // base_url / client_type 不变（无独立 coding host）。
    minimax: [
      { protocol: "openai", base_url: "https://api.minimaxi.com/v1", client_type: "codex_tui", coding_plan: cp },
      { protocol: "anthropic", base_url: "https://api.minimaxi.com/anthropic", client_type: "claude_code", coding_plan: cp },
    ],
    minimax_en: [
      { protocol: "openai", base_url: "https://api.minimax.io/v1", client_type: "codex_tui", coding_plan: cp },
      { protocol: "anthropic", base_url: "https://api.minimax.io/anthropic", client_type: "claude_code", coding_plan: cp },
    ],
    bailian: [
      { protocol: "openai", base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://dashscope.aliyuncs.com/apps/anthropic", client_type: "claude_code" },
    ],
    bailian_coding: [
      { protocol: "anthropic", base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic", client_type: "claude_code" },
    ],
    deepseek: [
      { protocol: "openai", base_url: "https://api.deepseek.com/v1", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://api.deepseek.com/anthropic", client_type: "claude_code" },
    ],
    stepfun: [
      { protocol: "anthropic", base_url: "https://api.stepfun.com/step_plan", client_type: "claude_code" },
    ],
    stepfun_en: [
      { protocol: "anthropic", base_url: "https://api.stepfun.ai/step_plan", client_type: "claude_code" },
    ],
    doubao: [
      { protocol: "anthropic", base_url: "https://ark.cn-beijing.volces.com/api/coding", client_type: "claude_code" },
      // 标准 OpenAI(Chat Completions) 兼容端点：base_url 含 /v3（火山 coding plan 的 openai 侧均 v3 结尾），
      // proxy 拼 /chat/completions。供非 Codex 的 openai 客户端使用。与 openai_responses 同 base_url、协议不同。
      { protocol: "openai", base_url: "https://ark.cn-beijing.volces.com/api/coding/v3", client_type: "codex_tui" },
      // OpenAI Responses 兼容端点（Codex）：base_url 含 /v3，proxy 拼 path；hosts 派生后 /api/coding/v3
      // 比 /api/coding 更长 → 粘贴 v3 URL 最长子串胜出命中 Responses 端点。
      { protocol: "openai_responses", base_url: "https://ark.cn-beijing.volces.com/api/coding/v3", client_type: "codex_tui" },
      // Agent Plan（火山新套餐，区别 Coding Plan 的 /api/coding）：anthropic 兼容端点 /api/plan。
      // hosts 派生后 /api/plan 与 /api/coding 等长，靠用户粘入的 base_url path 命中对应端点。
      { protocol: "anthropic", base_url: "https://ark.cn-beijing.volces.com/api/plan", client_type: "claude_code" },
      // Agent Plan openai 侧：标准 Chat Completions（/api/plan/v3）+ Responses（同 base_url，协议不同）。
      { protocol: "openai", base_url: "https://ark.cn-beijing.volces.com/api/plan/v3", client_type: "codex_tui" },
      { protocol: "openai_responses", base_url: "https://ark.cn-beijing.volces.com/api/plan/v3", client_type: "codex_tui" },
    ],
    byteplus: [
      { protocol: "anthropic", base_url: "https://ark.ap-southeast.bytepluses.com/api/coding", client_type: "claude_code" },
    ],
    qianfan: cp ? [
      { protocol: "openai", base_url: "https://qianfan.baidubce.com/v2/coding", client_type: "codex_tui", coding_plan: true },
      { protocol: "anthropic", base_url: "https://qianfan.baidubce.com/anthropic/coding", client_type: "claude_code", coding_plan: true },
    ] : [
      { protocol: "anthropic", base_url: "https://qianfan.baidubce.com/anthropic/coding", client_type: "claude_code" },
    ],
    xiaomi_mimo: cp ? [
      { protocol: "anthropic", base_url: "https://token-plan-cn.xiaomimimo.com/anthropic", client_type: "claude_code", coding_plan: true },
      { protocol: "openai", base_url: "https://token-plan-cn.xiaomimimo.com/v1", client_type: "codex_tui", coding_plan: true },
    ] : [
      { protocol: "anthropic", base_url: "https://api.xiaomimimo.com/anthropic", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://api.xiaomimimo.com/v1", client_type: "codex_tui" },
    ],
    bailing: [
      { protocol: "anthropic", base_url: "https://api.tbox.cn/api/anthropic", client_type: "claude_code" },
    ],
    longcat: [
      { protocol: "anthropic", base_url: "https://api.longcat.chat/anthropic", client_type: "claude_code" },
      // openai 端点实证 (2026-07-03 curl)：/openai/v1 返 401 鉴权墙确认存在；/v1/* 全 404 openresty 兜底已证伪。
      { protocol: "openai", base_url: "https://api.longcat.chat/openai/v1", client_type: "codex_tui" },
    ],
    // 商汤 SenseNova（日日新）Token Plan 公测全免费：token.sensenova.cn 仅暴露 4 个 LLM 端点，
    // usage/quota/balance 全 404（无 API-Key 配额接口，同 xiaomi_mimo token-plan），quota.rs 不加 case。
    // base_url 铁律：openai 端点含 /v1（proxy 拼 /chat/completions）；anthropic 端点裸 host
    // （https://token.sensenova.cn，proxy 拼 /v1/messages；配 /v1 会 /v1/v1/messages 404）。
    // responses 端点 /v1/responses 探测返回 404 NOT_FOUND（非 401）→ 不支持，未配。
    sensenova: [
      { protocol: "openai", base_url: "https://token.sensenova.cn/v1", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://token.sensenova.cn", client_type: "claude_code" },
    ],

    // ── 聚合平台 ──
    openrouter: [
      { protocol: "anthropic", base_url: "https://openrouter.ai/api", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://openrouter.ai/api/v1", client_type: "codex_tui" },
      { protocol: "gemini", base_url: "https://openrouter.ai/api" },
    ],
    siliconflow: [
      { protocol: "anthropic", base_url: "https://api.siliconflow.cn", client_type: "claude_code" },
    ],
    siliconflow_en: [
      { protocol: "anthropic", base_url: "https://api.siliconflow.com", client_type: "claude_code" },
    ],
    aihubmix: [
      { protocol: "anthropic", base_url: "https://aihubmix.com", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://aihubmix.com/v1", client_type: "codex_tui" },
    ],
    dmxapi: [
      { protocol: "anthropic", base_url: "https://www.dmxapi.cn", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://www.dmxapi.cn/v1", client_type: "codex_tui" },
    ],
    modelscope: [
      { protocol: "anthropic", base_url: "https://api-inference.modelscope.cn", client_type: "claude_code" },
    ],
    shengsuanyun: [
      { protocol: "anthropic", base_url: "https://router.shengsuanyun.com/api", client_type: "claude_code" },
    ],
    atlascloud: [
      { protocol: "anthropic", base_url: "https://api.atlascloud.ai", client_type: "claude_code" },
    ],
    novita: [
      { protocol: "anthropic", base_url: "https://api.novita.ai/anthropic", client_type: "claude_code" },
    ],
    therouter: [
      { protocol: "anthropic", base_url: "https://api.therouter.ai", client_type: "claude_code" },
    ],
    cherryin: [
      { protocol: "anthropic", base_url: "https://open.cherryin.net", client_type: "claude_code" },
    ],

    // ── 第三方平台 ──
    packycode: [
      { protocol: "anthropic", base_url: "https://www.packyapi.com", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://www.packyapi.com/v1", client_type: "codex_tui" },
      { protocol: "gemini", base_url: "https://www.packyapi.com" },
    ],
    cubence: [
      { protocol: "anthropic", base_url: "https://api.cubence.com", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://api.cubence.com/v1", client_type: "codex_tui" },
      { protocol: "gemini", base_url: "https://api.cubence.com" },
    ],
    aigocode: [
      { protocol: "anthropic", base_url: "https://api.aigocode.com", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://api.aigocode.com/v1", client_type: "codex_tui" },
      { protocol: "gemini", base_url: "https://api.aigocode.com" },
    ],
    rightcode: [
      { protocol: "anthropic", base_url: "https://www.right.codes/claude", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://right.codes/codex/v1", client_type: "codex_tui" },
    ],
    aicodemirror: [
      { protocol: "anthropic", base_url: "https://api.aicodemirror.com/api/claudecode", client_type: "claude_code" },
      { protocol: "openai", base_url: "https://api.aicodemirror.com/api/codex/backend-api/codex", client_type: "codex_tui" },
      { protocol: "gemini", base_url: "https://api.aicodemirror.com/api/gemini" },
    ],
    nvidia: [
      { protocol: "openai", base_url: "https://integrate.api.nvidia.com/v1", client_type: "codex_tui" },
    ],
    pateway: [
      { protocol: "anthropic", base_url: "https://api.pateway.ai", client_type: "claude_code" },
    ],
    ccsub: [
      { protocol: "anthropic", base_url: "https://www.ccsub.net", client_type: "claude_code" },
    ],
    apikeyfun: [
      { protocol: "anthropic", base_url: "https://api.apikey.fun", client_type: "claude_code" },
    ],
    apinebula: [
      { protocol: "anthropic", base_url: "https://apinebula.com", client_type: "claude_code" },
    ],
    sudocode: [
      { protocol: "anthropic", base_url: "https://sudocode.us", client_type: "claude_code" },
    ],
    claudeapi: [
      { protocol: "anthropic", base_url: "https://gw.claudeapi.com", client_type: "claude_code" },
    ],
    claudecn: [
      { protocol: "anthropic", base_url: "https://claudecn.top", client_type: "claude_code" },
    ],
    runapi: [
      { protocol: "anthropic", base_url: "https://runapi.co", client_type: "claude_code" },
    ],
    relaxycode: [
      { protocol: "anthropic", base_url: "https://www.relaxycode.com", client_type: "claude_code" },
    ],
    crazyrouter: [
      { protocol: "anthropic", base_url: "https://cn.crazyrouter.com", client_type: "claude_code" },
    ],
    sssaicode: [
      { protocol: "anthropic", base_url: "https://node-hk.sssaicodeapi.com/api", client_type: "claude_code" },
    ],
    compshare: [
      { protocol: "anthropic", base_url: "https://api.modelverse.cn", client_type: "claude_code" },
    ],
    compshare_coding: [
      { protocol: "anthropic", base_url: "https://cp.compshare.cn", client_type: "claude_code" },
    ],
    micu: [
      { protocol: "anthropic", base_url: "https://www.micuapi.ai", client_type: "claude_code" },
    ],
    ctok: [
      { protocol: "anthropic", base_url: "https://api.ctok.ai", client_type: "claude_code" },
    ],
    eflowcode: [
      { protocol: "anthropic", base_url: "https://e-flowcode.cc", client_type: "claude_code" },
    ],
    lemondata: [
      { protocol: "anthropic", base_url: "https://api.lemondata.cc", client_type: "claude_code" },
    ],
    pipellm: [
      { protocol: "anthropic", base_url: "https://cc-api.pipellm.ai", client_type: "claude_code" },
    ],
    // opencode zen/go：codex_tui OpenAI 兼容路由根（区别 opencode_zen 的 /zen/v1）。
    // base_url 必须含 /v1 前缀，proxy build_models_url 拼 /models、provider_api_path 拼 /chat/completions；
    // 缺 /v1 则 /zen/go/models + /zen/go/chat/completions 双 404（request_id a88d75c5）。
    opencode: [
      { protocol: "openai", base_url: "https://opencode.ai/zen/go/v1", client_type: "codex_tui" },
    ],
    // OpenCode Zen 免费版：标准 OpenAI 兼容端点，免费模型靠 catalog 定价 0（big-pickle/glm-4.7-free 等）。
    // base_url 含 /v1 前缀，proxy 拼 /chat/completions；/v1/models 无 auth 可列模型。
    // api_key 用户自填；留空时 proxy 端 resolve_opencode_zen_key 兜底 $opencode（匿名免费，共享限频）。
    opencode_zen: [
      { protocol: "openai", base_url: "https://opencode.ai/zen/v1", client_type: "default" },
    ],
    // ── 中转平台 ──
    newapi: [
      { protocol: "openai", base_url: "https://your-newapi-instance.com/v1", client_type: "codex_tui" },
    ],

    // ── 订阅透传（纯透传，base_url 填 host 根，客户端原始 path 直接拼接）──
    claude_code: [
      { protocol: "anthropic", base_url: "https://api.anthropic.com", client_type: "default" },
    ],
  };
  return (base[protocol] || []).map(ep => ({ ...ep }));
}

/** 从 getDefaultEndpoints 派生 URL 子串（host + path），注入 PROTOCOLS 供智能识别 base_url 优先匹配。
 *  按 preset.codingPlan 取对应 cp 分支，避免 coding plan 与普通版互相误匹配。
 *  取 host+pathname（非仅 hostname）：同 host 分裂（如 glm open.bigmodel.cn 普通 /api/paas/v4 vs
 *  coding /api/coding/paas/v4）靠 path 子串区分；不同 host（xiaomi_mimo token-plan-cn vs api）靠 host 区分。
 *  matchPlatform 最长串胜出 → 最特异 preset 命中。单一事实源：base_url 改动只动 getDefaultEndpoints。 */
for (const p of PROTOCOLS) {
  const hosts = new Set<string>();
  for (const ep of getDefaultEndpoints(p.value, !!p.codingPlan)) {
    try {
      const u = new URL(ep.base_url);
      const host = u.host.replace(/^www\./, "").toLowerCase();
      // host + path（去尾斜杠），path 为空则仅 host。含 path 让同 host 分裂可区分。
      const path = u.pathname.replace(/\/+$/, "").toLowerCase();
      const sub = path && path !== "/" ? host + path : host;
      if (host) hosts.add(sub);
    } catch { /* 非法 URL 跳过 */ }
  }
  if (hosts.size) p.hosts = [...hosts];
}


/** 主流平台预设默认模型（按 PlatformModels 槽位语义归类）。
 *  与 getDefaultEndpoints 同址同模式：纯前端预设，落 CreatePlatform.models。
 *  仅覆盖主流平台，其余留空（向后兼容，未覆盖平台 models 保持全空）。
 *  模型名取各平台当前主力型号；不确定的不硬填（避免过时/编造）。 */
export function getDefaultModels(protocol: Protocol, codingPlan?: boolean): Partial<Record<ModelSlot, string>> {
  // 默认模型截至 2026-06 核对官方发布说明；上游模型迭代月级，过时由 fetchModels 拉取覆盖；维护说明见 .claude/skills/aidog-add-platform/references/default-model.md
  const cp = !!codingPlan;
  const presets: Partial<Record<Protocol, Partial<Record<ModelSlot, string>>>> = {
    // ── 官方 ──
    anthropic: { default: "claude-opus-4-8", opus: "claude-opus-4-8", sonnet: "claude-sonnet-4-6", haiku: "claude-haiku-4-5" },
    openai: { gpt: "gpt-5.5" },
    codex: { gpt: "gpt-5.5-codex" }, // TODO 核对 codex 变体确切 API id，截至2026-06 未从官方 docs 确认
    // gemini: 槽位语义不匹配（无 opus/sonnet/gpt 对应），留空待用户填或拉取

    // ── 国内官方 ──
    // glm-4.6 已被 glm-5.2 取代（2026-06 官方新旗舰）；coding plan 端如遇不兼容回退 4.6
    glm: { default: "glm-5.2" },
    glm_en: { default: "glm-5.2" },
    // kimi-k2 原系列 2026-05-25 已停用
    kimi: { default: cp ? "kimi-k2.7-code" : "kimi-k2.6" },
    minimax: { default: "MiniMax-M3" },
    minimax_en: { default: "MiniMax-M3" },
    // 百炼（通义千问）；qwen3-max 已被 qwen3.7-max(2026-05-20) 取代；coding plan 对齐 lists cp 首项
    bailian: { default: cp ? "qwen3-coder-plus" : "qwen3.7-max" },
    // deepseek-chat 将 2026-07-24 弃用，v4-flash 为后继
    deepseek: { default: "deepseek-v4-flash" },
    // 阶跃星辰（research:94 静态确认旗舰；step-3.7-flash 国际/国内同型号）
    stepfun: { default: "step-3.7-flash" },
    stepfun_en: { default: "step-3.7-flash" },
    // 火山引擎（ark）coding 端点旗舰；model id 格式（点 vs 横线）待核实，暂随仓库现行短横线惯例
    doubao: { default: "doubao-seed-2-0-code" },
    // BytePlus 国际 doubao 旗舰（research:106；lists:531 首项一致）
    byteplus: { default: "doubao-seed-2-0-pro" },
    // 小米 MiMo 旗舰文本模型（按量 openai 端点）
    xiaomi_mimo: { default: "mimo-v2.5-pro" },
    // OpenCode Zen 免费旗舰（catalog 定价 0）；其余免费模型靠 fetchModels /v1/models 拉取
    opencode_zen: { default: "big-pickle" },
    // 商汤 SenseNova Token Plan 公测全免费；flash-lite 为 chat + 图像理解旗舰（256K）
    sensenova: { default: "sensenova-6.7-flash-lite" },
  };
  return { ...(presets[protocol] || {}) };
}

/** 平台内置候选模型列表（供模型槽位下拉冷启动兜底）。
 *  未刷新（fetchModels）时槽位下拉展示此列表；刷新成功后改用接口 available_models。
 *  数据来自 .trellis/tasks/06-17-platform-model-list/research/*.md（一方/聚合/第三方三组核查）。
 *  约定：列表有序，旗舰/默认在前，**首项 = getDefaultModels 默认值**（保 route resolve 行为）。
 *  查不到官方列表的平台返回 []（完全靠 fetchModels 兜底，不编造）。
 *  模型名月级腐化，运行时必靠 fetchModels 拉取真实可用集。 */
export function getDefaultModelList(protocol: Protocol, codingPlan?: boolean): string[] {
  // 截至 2026-06-17 核对官方（信源见 research/models-{firstparty,aggregator,thirdparty}.md）
  const cp = !!codingPlan;

  // Claude 旗舰候选列表（第三方/中转组 22 平台共用，连字符 API id）
  const CLAUDE_FLAGSHIP = [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5",
  ];

  const lists: Partial<Record<Protocol, string[]>> = {
    // ── 一方官方（research/models-firstparty.md）──
    anthropic: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5"],
    openai: ["gpt-5.5"], // OpenAI docs SPA 未官方确认，沿用现有
    codex: ["gpt-5.5-codex"], // 未官方确认，留 fetchModels 兜底
    // gemini: 槽位语义不匹配，留空靠 fetchModels
    glm: ["glm-5.2", "glm-5.1", "glm-5", "glm-5-turbo", "glm-4.7", "glm-4.7-flash", "glm-4.6", "glm-4.5-air"],
    glm_en: ["glm-5.2", "glm-5.1", "glm-5", "glm-5-turbo", "glm-4.7", "glm-4.7-flash", "glm-4.6", "glm-4.5-air"],
    kimi: cp
      ? ["kimi-k2.7-code", "kimi-k2.7-code-highspeed", "kimi-k2.6", "kimi-k2.5"]
      : ["kimi-k2.6", "kimi-k2.5", "kimi-k2-thinking", "kimi-latest"],
    minimax: ["MiniMax-M3", "MiniMax-M2.7", "MiniMax-M2.5", "MiniMax-M2.1", "MiniMax-M2"], // M3 大小写推测，见 research
    minimax_en: ["MiniMax-M3", "MiniMax-M2.7", "MiniMax-M2.5", "MiniMax-M2.1", "MiniMax-M2"],
    bailian: cp
      ? ["qwen3-coder-plus", "qwen3-coder-flash", "qwen3.7-max", "qwen3.7-plus", "qwen3.6-flash"]
      : ["qwen3.7-max", "qwen3.7-plus", "qwen3.6-flash", "qwen3.5-omni-plus", "qwen3-coder-plus", "qwen3-coder-flash"],
    // bailian_coding: 透传端点无独立列表，留空靠 fetchModels
    deepseek: ["deepseek-v4-flash", "deepseek-v4-pro", "deepseek-chat", "deepseek-reasoner"],
    stepfun: ["step-3.7-flash", "step-3.5-flash"],
    stepfun_en: ["step-3.7-flash", "step-3.5-flash"],
    // 火山引擎 ark 官方 + 托管多厂商型号。doubao 自有型号沿用仓库短横线惯例（点 vs 横线待核实）；
    // 跨厂商型号（minimax/glm/deepseek/kimi）保留各厂商原生命名。首项 = getDefaultModels 默认值。
    doubao: ["doubao-seed-2-0-code", "doubao-seed-2-0-pro", "doubao-seed-2-0-lite", "doubao-seed-code", "minimax-m2.7", "minimax-m3", "glm-5.2", "deepseek-v4-flash", "deepseek-v4-pro", "kimi-k2.6", "kimi-k2.7-code"],
    byteplus: ["doubao-seed-2-0-pro", "doubao-seed-2-0-code-preview", "doubao-seed-2-0-lite", "doubao-seed-2-0-mini"],
    // qianfan: 百度文档 JS-rendered 未拿到确切 chat id，留空靠 fetchModels
    xiaomi_mimo: ["mimo-v2.5-pro", "mimo-v2-pro", "mimo-v2.5", "mimo-v2-omni", "mimo-v2-flash"],
    // 商汤 SenseNova Token Plan：chat + 推理 + 图像生成三模型（公测全免费）。首项 = getDefaultModels 默认值。
    sensenova: ["sensenova-6.7-flash-lite", "deepseek-v4-flash", "sensenova-u1-fast"],
    // OpenCode Zen 免费版：catalog 定价 0 的免费模型冷启动占位（其余靠 fetchModels /v1/models 拉取）。
    // 首项 = getDefaultModels 默认值（big-pickle）。
    opencode_zen: ["big-pickle", "glm-4.7-free"],
    // bailing / longcat: 官方模型文档无静态来源，留空靠 fetchModels

    // ── 聚合平台（research/models-aggregator.md，fetchModels 为主源，列表仅冷启动占位）──
    openrouter: [
      "anthropic/claude-opus-4.8", "anthropic/claude-sonnet-4.6", "anthropic/claude-opus-4.5",
      "openai/gpt-5.5", "openai/gpt-5.5-pro", "openai/gpt-5.3-codex",
      "google/gemini-3.5-flash", "google/gemini-3.1-pro-preview",
      "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash",
      "qwen/qwen3.7-max", "z-ai/glm-5.2", "moonshotai/kimi-k2.7-code", "x-ai/grok-4.3", "minimax/minimax-m3",
    ],
    // siliconflow / siliconflow_en: 公开端点需鉴权 + 文档腐化，留空靠 fetchModels
    aihubmix: [
      "claude-opus-4-8", "claude-sonnet-4-6", "claude-sonnet-4-5",
      "gpt-5.5", "gpt-5.5-pro", "gpt-5.3-codex",
      "gemini-3.5-flash", "gemini-3.1-pro-preview",
      "deepseek-v4-pro", "deepseek-v4-flash", "qwen3.7-max", "glm-5.2", "kimi-k2.7-code", "grok-4.3",
    ],
    dmxapi: [
      "claude-opus-4-8", "claude-sonnet-4-6", "claude-opus-4-5-20251101",
      "deepseek-v4-pro", "deepseek-v4-flash",
      "gpt-5.5", "gpt-5.3-codex", "gemini-3.5-flash", "gemini-3.1-pro-preview",
      "glm-5.2", "kimi-k2.7-code",
    ],
    modelscope: [
      "deepseek-ai/DeepSeek-V4-Pro", "deepseek-ai/DeepSeek-V4-Flash", "deepseek-ai/DeepSeek-V3.2",
      "Qwen/Qwen3.5-397B-A17B", "Qwen/Qwen3.5-122B-A10B", "Qwen/Qwen3-Coder-30B-A3B-Instruct",
      "ZhipuAI/GLM-5.2", "ZhipuAI/GLM-5.1", "ZhipuAI/GLM-5",
      "moonshotai/Kimi-K2.5", "MiniMax/MiniMax-M3", "MiniMax/MiniMax-M2.7",
    ],
    shengsuanyun: [
      "anthropic/claude-opus-4.8", "anthropic/claude-sonnet-4.6", "anthropic/claude-opus-4.5",
      "openai/gpt-5.5", "openai/gpt-5.3-codex",
      "google/gemini-3.5-flash", "google/gemini-3.1-pro-preview",
      "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash",
      "ali/qwen3.7-max", "bigmodel/glm-5.2", "moonshot/kimi-k2.7-code", "x-ai/grok-4",
    ],
    atlascloud: [
      "deepseek-ai/DeepSeek-V3.2-Exp", "deepseek-ai/DeepSeek-V3.1-Terminus", "deepseek-ai/DeepSeek-V3-0324",
      "zai-org/GLM-4.6", "Qwen/Qwen3-235B-A22B-Instruct-2507", "Qwen/Qwen3-Coder",
      "Qwen/Qwen3-Next-80B-A3B-Instruct", "Qwen/Qwen3-VL-235B-A22B-Instruct",
      "moonshotai/Kimi-K2-Thinking", "moonshotai/Kimi-K2-Instruct-0905", "MiniMaxAI/MiniMax-M2",
    ],
    novita: [
      "zai-org/glm-5.2", "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash",
      "qwen/qwen3.7-max", "moonshotai/kimi-k2.7-code", "minimax/minimax-m3",
      "zai-org/glm-5.1", "qwen/qwen3.6-plus", "moonshotai/kimi-k2.6", "minimax/minimax-m2.7", "deepseek/deepseek-v3.2",
    ],
    // therouter: 全端点 404/需鉴权/SPA，留空靠 fetchModels
    cherryin: [
      "anthropic/claude-opus-4.8", "anthropic/claude-sonnet-4.6", "anthropic/claude-opus-4.5",
      "openai/gpt-5.5", "openai/gpt-5.3-codex",
      "google/gemini-3.5-flash", "google/gemini-3-pro-preview",
      "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash", "deepseek/deepseek-v3.2",
      "agent/glm-5.2", "moonshotai/kimi-k2.7-code", "grok-4",
    ],
    nvidia: [
      "nvidia/nemotron-3-ultra-550b-a55b", "nvidia/nemotron-3-super-120b-a12b",
      "nvidia/llama-3.3-nemotron-super-49b-v1.5", "deepseek/deepseek-v3.2",
      "qwen/qwen3.5-397b-a17b", "qwen/qwen3-next-80b-a3b-instruct",
      "z-ai/glm-5.1", "moonshotai/kimi-k2.6", "minimaxai/minimax-m3",
      "meta/llama-4-maverick-17b-128e-instruct", "meta/llama-3.3-70b-instruct", "openai/gpt-oss-120b",
    ],

    // ── 第三方/中转（research/models-thirdparty.md，纯 Claude Code 中转 → Claude 旗舰列表）──
    packycode: CLAUDE_FLAGSHIP,
    cubence: CLAUDE_FLAGSHIP,
    aigocode: CLAUDE_FLAGSHIP,
    rightcode: CLAUDE_FLAGSHIP,
    aicodemirror: CLAUDE_FLAGSHIP,
    pateway: CLAUDE_FLAGSHIP,
    ccsub: CLAUDE_FLAGSHIP,
    apikeyfun: CLAUDE_FLAGSHIP,
    apinebula: CLAUDE_FLAGSHIP,
    sudocode: CLAUDE_FLAGSHIP,
    claudeapi: CLAUDE_FLAGSHIP,
    claudecn: CLAUDE_FLAGSHIP,
    runapi: CLAUDE_FLAGSHIP,
    relaxycode: CLAUDE_FLAGSHIP,
    crazyrouter: CLAUDE_FLAGSHIP,
    sssaicode: CLAUDE_FLAGSHIP,
    compshare_coding: CLAUDE_FLAGSHIP,
    micu: CLAUDE_FLAGSHIP,
    ctok: CLAUDE_FLAGSHIP,
    eflowcode: CLAUDE_FLAGSHIP,
    lemondata: CLAUDE_FLAGSHIP,
    pipellm: CLAUDE_FLAGSHIP,
    claude_code: CLAUDE_FLAGSHIP,
    // compshare / opencode / newapi: 自有/聚合或非 Claude 专用，留空靠 fetchModels
  };
  return lists[protocol] ? [...lists[protocol]!] : [];
}
