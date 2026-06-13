import React, { useState, useEffect, useRef, useMemo, memo } from "react";
import { useTranslation } from "react-i18next";
import { platformApi, settingsApi, modelTestApi, quotaApi, parseMockConfig, serializeMockConfig, parseNewApiConfig, serializeNewApiConfig, onProxyLogUpdated, DEFAULT_MOCK_CONFIG, DEFAULT_NEWAPI_CONFIG, type Platform, type Protocol, type ModelSlot, type PlatformEndpoint, type ClientType, type PlatformUsageStats, type PlatformQuota, type MockConfig, type MockErrorMode, type NewApiConfig, type ManualBudget, type ManualBudgetKind, type ManualBudgetUnit, type WindowUnit } from "../services/api";
import { getPlatformLogo, getFaviconUrl } from "../assets/platforms";
import { IconBolt, IconCost, IconCheck, IconClose, IconCoin, IconClock } from "../components/icons";
import { CompactCard, StatChip, BalanceBar, successRateLevel, costLevel, levelColor, levelBg, codingTierLevel, cycleMsForTier, usageLevelToColor, type ColorLevel } from "../components/shared";
import { formatNumber, formatCost, formatPercent } from "../utils/formatters";

import { ModelTestPanel } from "./ModelTestPanel";
import { pinyinMatch } from "../utils/pinyin";

/** 支持的协议选项（含 coding plan 变体） */
type ProtocolOption = { value: Protocol; label: string; codingPlan?: boolean; keywords?: string[] };

const PROTOCOLS: ProtocolOption[] = [
  // ── 官方 ──
  { value: "anthropic", label: "Anthropic（Claude）", keywords: ["claude", "克劳德", "官方"] },
  { value: "openai", label: "OpenAI", keywords: ["gpt", "chatgpt", "官方"] },
  { value: "codex", label: "Codex", keywords: ["openai", "codex"] },
  { value: "gemini", label: "Gemini（Google）", keywords: ["google", "谷歌", "gemini"] },
  // ── 国内官方 ──
  { value: "glm", label: "GLM（智谱）", keywords: ["zhipu", "智谱", "bigmodel", "codegeex"] },
  { value: "glm", label: "GLM Coding Plan", codingPlan: true, keywords: ["智谱编程", "codegeex", "glm code"] },
  { value: "glm_en", label: "GLM 国际版（z.ai）", keywords: ["z.ai", "zhipu en", "智谱国际"] },
  { value: "kimi", label: "Kimi（月之暗面）", keywords: ["moonshot", "月之暗面"] },
  { value: "kimi", label: "Kimi Code Plan", codingPlan: true, keywords: ["kimi编程", "kimi code", "kimi coding"] },
  { value: "minimax", label: "MiniMax（海螺）", keywords: ["海螺", "minimax"] },
  { value: "minimax_en", label: "MiniMax 国际版", keywords: ["minimax io", "minimax en"] },
  { value: "bailian", label: "百炼（阿里）", keywords: ["dashscope", "阿里", "qwen", "通义"] },
  { value: "bailian_coding", label: "百炼编程", keywords: ["dashscope coding", "阿里编程", "百炼编程"] },
  { value: "deepseek", label: "DeepSeek（深度求索）", keywords: ["深度求索", "deepseek"] },
  { value: "stepfun", label: "阶跃星辰（StepFun）", keywords: ["stepfun", "阶跃"] },
  { value: "stepfun_en", label: "StepFun 国际版", keywords: ["stepfun ai", "阶跃国际"] },
  { value: "doubao", label: "火山 Agentplan", keywords: ["火山", "volcengine", "agentplan"] },
  { value: "doubao_seed", label: "豆包 Seed", keywords: ["豆包", "doubao", "seed"] },
  { value: "byteplus", label: "BytePlus", keywords: ["byteplus", "字节国际"] },
  { value: "qianfan", label: "百度千帆", keywords: ["baidu", "百度", "千帆"] },
  { value: "qianfan", label: "百度千帆 Coding Plan Lite", codingPlan: true, keywords: ["baidu", "百度", "千帆", "qianfan", "coding"] },
  { value: "xiaomi_mimo", label: "小米 MiMo", keywords: ["xiaomi", "小米", "mimo"] },
  { value: "bailing", label: "百灵", keywords: ["bailing", "百灵", "tbox"] },
  { value: "longcat", label: "Longcat", keywords: ["longcat", "龙猫"] },
  // ── 聚合平台 ──
  { value: "openrouter", label: "OpenRouter", keywords: ["openrouter", "聚合"] },
  { value: "siliconflow", label: "SiliconFlow", keywords: ["siliconflow", "硅基流动"] },
  { value: "siliconflow_en", label: "SiliconFlow 国际版", keywords: ["siliconflow com"] },
  { value: "aihubmix", label: "AiHubMix", keywords: ["aihubmix"] },
  { value: "dmxapi", label: "DMXAPI", keywords: ["dmxapi"] },
  { value: "modelscope", label: "ModelScope（魔搭）", keywords: ["modelscope", "魔搭"] },
  { value: "shengsuanyun", label: "盛算云", keywords: ["shengsuanyun", "盛算"] },
  { value: "atlascloud", label: "AtlasCloud", keywords: ["atlascloud", "atlas"] },
  { value: "novita", label: "Novita AI", keywords: ["novita"] },
  { value: "therouter", label: "TheRouter", keywords: ["therouter"] },
  { value: "cherryin", label: "CherryIN", keywords: ["cherryin"] },
  // ── 第三方平台 ──
  { value: "packycode", label: "PackyCode", keywords: ["packycode", "packyapi"] },
  { value: "cubence", label: "Cubence", keywords: ["cubence"] },
  { value: "aigocode", label: "AIGoCode", keywords: ["aigocode"] },
  { value: "rightcode", label: "RightCode", keywords: ["rightcode", "right codes"] },
  { value: "aicodemirror", label: "AICodeMirror", keywords: ["aicodemirror", "claudecode net cn"] },
  { value: "nvidia", label: "Nvidia", keywords: ["nvidia", "英伟达"] },
  { value: "pateway", label: "PatewayAI", keywords: ["pateway"] },
  { value: "ccsub", label: "CCSub", keywords: ["ccsub"] },
  { value: "apikeyfun", label: "APIKEY.FUN", keywords: ["apikey fun"] },
  { value: "apinebula", label: "APINebula", keywords: ["apinebula"] },
  { value: "sudocode", label: "SudoCode", keywords: ["sudocode"] },
  { value: "claudeapi", label: "ClaudeAPI", keywords: ["claudeapi"] },
  { value: "claudecn", label: "ClaudeCN", keywords: ["claudecn"] },
  { value: "runapi", label: "RunAPI", keywords: ["runapi"] },
  { value: "relaxycode", label: "RelaxyCode", keywords: ["relaxycode"] },
  { value: "crazyrouter", label: "CrazyRouter", keywords: ["crazyrouter"] },
  { value: "sssaicode", label: "SSSAiCode", keywords: ["sssaicode"] },
  { value: "compshare", label: "Compshare（优云）", keywords: ["compshare", "优云", "ucloud"] },
  { value: "compshare_coding", label: "Compshare Coding Plan", keywords: ["compshare coding", "优云编程"] },
  { value: "micu", label: "Micu", keywords: ["micu"] },
  { value: "ctok", label: "CTok.ai", keywords: ["ctok"] },
  { value: "eflowcode", label: "E-FlowCode", keywords: ["eflowcode", "flowcode"] },
  { value: "lemondata", label: "LemonData", keywords: ["lemondata"] },
  { value: "pipellm", label: "PIPELLM", keywords: ["pipellm"] },
  { value: "opencode", label: "OpenCode Go", keywords: ["opencode"] },
  // ── 中转平台 ──
  { value: "newapi", label: "New API", keywords: ["newapi", "new-api", "one-api", "oneapi", "中转"] },
  // ── 订阅透传 ──
  { value: "claude_code", label: "Claude Code 订阅（透传）", keywords: ["claude code", "订阅", "透传", "subscription", "passthrough"] },
  // ── 测试 ──
  { value: "mock", label: "Mock（本地模拟）", keywords: ["mock", "测试", "调试", "假数据"] },
];

/** Endpoint 协议：只有 AI 请求协议（非平台类型） */
const ENDPOINT_PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "openai", label: "OpenAI Chat" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "openai_completions", label: "OpenAI Completions" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Gemini" },
];

/** 客户端模拟选项：用于通过上游客户端校验 */
const CLIENT_TYPES: { value: ClientType; labelKey?: string; label?: string; group: string }[] = [
  // 默认
  { value: "default", labelKey: "platform.mockDefault", group: "" },
  // Claude Code 家族
  { value: "claude_code", label: "Claude Code CLI", group: "Claude Code" },
  { value: "claude_code_vscode", label: "Claude Code VSCode", group: "Claude Code" },
  { value: "claude_code_sdk_ts", label: "Claude Code SDK (TS)", group: "Claude Code" },
  { value: "claude_code_sdk_py", label: "Claude Code SDK (Python)", group: "Claude Code" },
  { value: "claude_code_gh_action", label: "Claude Code GitHub Action", group: "Claude Code" },
  // Codex 家族
  { value: "codex_cli", label: "Codex CLI (Rust)", group: "Codex" },
  { value: "codex_tui", label: "Codex TUI", group: "Codex" },
  { value: "codex_desktop", label: "Codex Desktop", group: "Codex" },
  { value: "codex_vscode", label: "Codex VSCode", group: "Codex" },
  // IDE
  { value: "cursor", label: "Cursor", group: "IDE" },
  { value: "windsurf", label: "Windsurf", group: "IDE" },
];

/** 根据端点协议返回推荐的默认客户端类型 */
function defaultClientForProtocol(protocol: Protocol): ClientType {
  switch (protocol) {
    case "anthropic": return "claude_code";
    case "openai": return "codex_tui";
    default: return "default";
  }
}


/** 内置平台默认端点：每个平台支持的协议及其 base URL
 * URL 为不含 adapter 路径前缀的基础地址，proxy 会拼接 adapter 路径
 * 来源：各平台官方文档 */
type HealthStatus = "healthy" | "warning" | "error" | "unknown";
const HEALTH_COLORS: Record<HealthStatus, string> = {
  healthy: "var(--color-success, #34c759)",
  warning: "var(--color-warning, #ff9500)",
  error: "var(--color-danger, #ff3b30)",
  unknown: "transparent",
};

/** 判断平台健康状态：最近 N 次请求中失败次数 */
function healthStatus(recentTotal: number, recentFailures: number): HealthStatus {
  if (recentTotal === 0) return "unknown";
  if (recentFailures >= recentTotal) return "error";        // 全部失败
  if (recentFailures > 0) return "warning";                  // 有失败
  return "healthy";                                           // 全部成功
}

/** 根据 ProtocolOption 生成默认端点（含 coding_plan 标记）
 *  数据来源：cc-switch 各平台官方配置 */
function getDefaultEndpoints(protocol: Protocol, codingPlan?: boolean): PlatformEndpoint[] {
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
    glm: [
      { protocol: "openai", base_url: cp ? "https://open.bigmodel.cn/api/coding/paas/v4" : "https://open.bigmodel.cn/api/paas/v4", client_type: "codex_tui", coding_plan: cp },
      { protocol: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", client_type: "claude_code" },
    ],
    glm_en: [
      { protocol: "openai", base_url: "https://api.z.ai/api/paas/v4", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://api.z.ai/api/anthropic", client_type: "claude_code" },
    ],
    kimi: [
      { protocol: "openai", base_url: cp ? "https://api.kimi.com/coding/v1" : "https://api.moonshot.cn/v1", client_type: "claude_code", coding_plan: cp },
      { protocol: "anthropic", base_url: "https://api.moonshot.cn/anthropic", client_type: "claude_code" },
    ],
    minimax: [
      { protocol: "openai", base_url: "https://api.minimaxi.com/v1", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://api.minimaxi.com/anthropic", client_type: "claude_code" },
    ],
    minimax_en: [
      { protocol: "openai", base_url: "https://api.minimax.io/v1", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://api.minimax.io/anthropic", client_type: "claude_code" },
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
    ],
    doubao_seed: [
      { protocol: "anthropic", base_url: "https://ark.cn-beijing.volces.com/api/compatible", client_type: "claude_code" },
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
    xiaomi_mimo: [
      { protocol: "anthropic", base_url: "https://api.xiaomimimo.com/anthropic", client_type: "claude_code" },
    ],
    bailing: [
      { protocol: "anthropic", base_url: "https://api.tbox.cn/api/anthropic", client_type: "claude_code" },
    ],
    longcat: [
      { protocol: "anthropic", base_url: "https://api.longcat.chat/anthropic", client_type: "claude_code" },
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
      { protocol: "openai", base_url: "https://api.aigocode.com", client_type: "codex_tui" },
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
    opencode: [
      { protocol: "openai", base_url: "https://opencode.ai/zen/go", client_type: "codex_tui" },
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

const PROTOCOL_LABELS: Record<Protocol, string> = {
  // ── AI 请求协议 ──
  openai: "OpenAI",
  openai_responses: "OpenAI Responses",
  openai_completions: "OpenAI Completions",
  anthropic: "Anthropic",
  gemini: "Gemini",
  // ── 平台类型 ──
  glm: "GLM",
  glm_en: "GLM 国际版",
  kimi: "Kimi",
  minimax: "MiniMax",
  minimax_en: "MiniMax 国际版",
  codex: "Codex",
  bailian: "百炼",
  bailian_coding: "百炼编程",
  // ── 国内官方 ──
  deepseek: "DeepSeek",
  stepfun: "阶跃星辰",
  stepfun_en: "StepFun 国际版",
  doubao: "火山 Agentplan",
  doubao_seed: "豆包 Seed",
  byteplus: "BytePlus",
  qianfan: "百度千帆",
  xiaomi_mimo: "小米 MiMo",
  bailing: "百灵",
  longcat: "Longcat",
  // ── 聚合平台 ──
  openrouter: "OpenRouter",
  siliconflow: "SiliconFlow",
  siliconflow_en: "SiliconFlow 国际版",
  aihubmix: "AiHubMix",
  dmxapi: "DMXAPI",
  modelscope: "ModelScope",
  shengsuanyun: "盛算云",
  atlascloud: "AtlasCloud",
  novita: "Novita AI",
  therouter: "TheRouter",
  cherryin: "CherryIN",
  // ── 第三方平台 ──
  packycode: "PackyCode",
  cubence: "Cubence",
  aigocode: "AIGoCode",
  rightcode: "RightCode",
  aicodemirror: "AICodeMirror",
  nvidia: "Nvidia",
  pateway: "PatewayAI",
  ccsub: "CCSub",
  apikeyfun: "APIKEY.FUN",
  apinebula: "APINebula",
  sudocode: "SudoCode",
  claudeapi: "ClaudeAPI",
  claudecn: "ClaudeCN",
  runapi: "RunAPI",
  relaxycode: "RelaxyCode",
  crazyrouter: "CrazyRouter",
  sssaicode: "SSSAiCode",
  compshare: "Compshare",
  compshare_coding: "Compshare Coding",
  micu: "Micu",
  ctok: "CTok.ai",
  eflowcode: "E-FlowCode",
  lemondata: "LemonData",
  pipellm: "PIPELLM",
  opencode: "OpenCode Go",
  // ── 订阅透传 ──
  claude_code: "Claude Code 订阅",
  // ── 中转平台 ──
  newapi: "New API",
  // ── 测试 ──
  mock: "Mock",
};

const DEFAULT_NAMES = new Set(Object.values(PROTOCOL_LABELS));

const PROTOCOL_COLORS: Record<string, string> = {
  // ── 官方 ──
  anthropic: "#D97757",
  openai: "#10A37F",
  openai_responses: "#10A37F",
  openai_completions: "#10A37F",
  codex: "#10A37F",
  gemini: "#4285F4",
  // ── 国内官方 ──
  glm: "#3B5FEC",
  glm_en: "#3B5FEC",
  kimi: "#1783FF",
  minimax: "#FF6B6B",
  minimax_en: "#FF6B6B",
  bailian: "#FF6A00",
  bailian_coding: "#FF6A00",
  deepseek: "#1E88E5",
  stepfun: "#16D6D2",
  stepfun_en: "#16D6D2",
  doubao: "#3370FF",
  doubao_seed: "#3370FF",
  byteplus: "#3370FF",
  qianfan: "#2932E1",
  xiaomi_mimo: "#FF6900",
  bailing: "#624AFF",
  longcat: "#29E154",
  // ── 聚合平台 ──
  openrouter: "#6566F1",
  siliconflow: "#6E29F6",
  siliconflow_en: "#6E29F6",
  aihubmix: "#006FFB",
  dmxapi: "#FF6B6B",
  modelscope: "#624AFF",
  shengsuanyun: "#00A67E",
  atlascloud: "#4285F4",
  novita: "#000000",
  therouter: "#6566F1",
  cherryin: "#FF4081",
  // ── 第三方平台 ──
  packycode: "#00BCD4",
  cubence: "#000000",
  aigocode: "#5B7FFF",
  rightcode: "#E96B2C",
  aicodemirror: "#000000",
  nvidia: "#76B900",
  pateway: "#00A67E",
  ccsub: "#FF6B6B",
  apikeyfun: "#FF9500",
  apinebula: "#6C5CE7",
  sudocode: "#00A67E",
  claudeapi: "#D97757",
  claudecn: "#D97757",
  runapi: "#10A37F",
  relaxycode: "#6C5CE7",
  crazyrouter: "#FF6B6B",
  sssaicode: "#FF9500",
  compshare: "#00A67E",
  compshare_coding: "#00A67E",
  micu: "#FF6B6B",
  ctok: "#10A37F",
  eflowcode: "#6C5CE7",
  lemondata: "#FFD21E",
  pipellm: "#4285F4",
  opencode: "#211E1E",
  // ── 订阅透传 ──
  claude_code: "#D97757",
  // ── 中转平台 ──
  newapi: "#10A37F",
  // ── 测试 ──
  mock: "#8E8E93",
};

const MODEL_SLOTS: { key: ModelSlot; labelKey: string }[] = [
  { key: "default", labelKey: "platform.modelDefault" },
  { key: "sonnet", labelKey: "platform.modelSonnet" },
  { key: "opus", labelKey: "platform.modelOpus" },
  { key: "haiku", labelKey: "platform.modelHaiku" },
  { key: "gpt", labelKey: "platform.modelGpt" },
];

/** 从 PlatformModels 中提取所有非空值（去重） */
function allModelValues(models: Platform["models"]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const slot of MODEL_SLOTS) {
    const v = models[slot.key];
    if (v && !seen.has(v)) {
      seen.add(v);
      result.push(v);
    }
  }
  return result;
}

/** 预估 coding plan JSON 结构（后端 est_coding_plan 列） */
interface EstCodingTier {
  name: string;
  est_utilization: number;
  coef_per_token: number;
  util_at_last_real: number;
  tokens_since_real: number;
  has_base: boolean;
  limit?: number;
  /** 本周期起点 unix ms（系统维护）；0/缺失 = 无可靠周期起点 → 配色中性。 */
  window_start?: number;
}
interface EstCodingPlan {
  tiers: EstCodingTier[];
  level: string | null;
}

/** 安全解析 est_coding_plan JSON；非法/空串返回 null */
function parseEstCodingPlan(raw: string): EstCodingPlan | null {
  if (!raw || !raw.trim()) return null;
  try {
    const obj = JSON.parse(raw) as Partial<EstCodingPlan>;
    if (!obj || !Array.isArray(obj.tiers)) return null;
    return { tiers: obj.tiers as EstCodingTier[], level: obj.level ?? null };
  } catch {
    return null;
  }
}

/** 根据模型名模式自动分配到槽位 */
function autoCategorize(modelIds: string[]): Record<ModelSlot, string> {
  const result: Record<ModelSlot, string> = {
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  };
  const patterns: { slot: ModelSlot; test: (id: string) => boolean }[] = [
    { slot: "opus", test: (id) => /opus/i.test(id) },
    { slot: "sonnet", test: (id) => /sonnet/i.test(id) },
    { slot: "haiku", test: (id) => /haiku/i.test(id) },
    { slot: "gpt", test: (id) => /gpt/i.test(id) && !/mini/i.test(id) },
  ];
  const assigned = new Set<string>();
  for (const { slot, test } of patterns) {
    for (const id of modelIds) {
      if (test(id) && !assigned.has(id)) {
        result[slot] = id;
        assigned.add(id);
      }
    }
  }
  const first = modelIds.find(id => !assigned.has(id)) ?? modelIds[0];
  if (first && !result.default) result.default = first;
  return result;
}

/** 可搜索的协议选择器（支持拼音模糊匹配 + Tab/方向键键盘导航） */
function SearchableProtocolSelect({
  value, codingPlan, onChange,
}: {
  value: Protocol;
  codingPlan: boolean;
  onChange: (proto: Protocol, codingPlan?: boolean) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // 当前选中项的显示文本
  const selectedLabel = PROTOCOLS.find(
    p => p.value === value && !!p.codingPlan === codingPlan
  )?.label || PROTOCOL_LABELS[value];

  // 按拼音/关键词过滤
  const filtered = PROTOCOLS.filter(p => {
    if (!query.trim()) return true;
    if (pinyinMatch(query, p.label)) return true;
    if (p.keywords?.some(kw => pinyinMatch(query, kw))) return true;
    if (pinyinMatch(query, p.value)) return true;
    return false;
  });

  // Tab 切换：在完整列表中循环到下一个平台
  const tabCycle = (shift: boolean) => {
    const idx = PROTOCOLS.findIndex(p => p.value === value && !!p.codingPlan === codingPlan);
    const next = shift
      ? (idx - 1 + PROTOCOLS.length) % PROTOCOLS.length
      : (idx + 1) % PROTOCOLS.length;
    const target = PROTOCOLS[next];
    onChange(target.value, target.codingPlan);
  };

  // 打开下拉时定位到当前选中项
  const openDropdown = () => {
    setOpen(true);
    setQuery("");
    const idx = filtered.findIndex(p => p.value === value && !!p.codingPlan === codingPlan);
    setHighlightedIndex(idx >= 0 ? idx : 0);
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  // 高亮项变化时滚动到可见区域
  useEffect(() => {
    if (!open || !scrollRef.current) return;
    const btn = scrollRef.current.children[highlightedIndex] as HTMLElement | undefined;
    btn?.scrollIntoView({ block: "nearest" });
  }, [highlightedIndex, open, filtered.length]);

  // 搜索内容变化时重置高亮到第一项
  useEffect(() => { setHighlightedIndex(0); }, [query]);

  /** 触发器键盘（下拉关闭态） */
  const handleTriggerKey = (e: React.KeyboardEvent) => {
    if (open) return;
    switch (e.key) {
      case "Tab":
        e.preventDefault();
        tabCycle(e.shiftKey);
        break;
      case "ArrowDown":
      case "Enter":
      case " ":
        e.preventDefault();
        openDropdown();
        break;
    }
  };

  /** 搜索输入键盘（下拉打开态） */
  const handleInputKey = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setHighlightedIndex(i => Math.min(i + 1, filtered.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setHighlightedIndex(i => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (filtered[highlightedIndex]) {
          onChange(filtered[highlightedIndex].value, filtered[highlightedIndex].codingPlan);
          setOpen(false);
          setQuery("");
        }
        break;
      case "Escape":
        e.preventDefault();
        setOpen(false);
        setQuery("");
        break;
    }
  };

  return (
    <div style={{ position: "relative" }} ref={listRef}>
      {/* 触发器：点击展开下拉，展示当前选中值 */}
      <div
        className="input"
        tabIndex={0}
        style={{
          display: "flex", alignItems: "center", justifyContent: "space-between",
          cursor: "pointer", userSelect: "none",
        }}
        onClick={() => {
          if (!open) { openDropdown(); }
          else { setOpen(false); setQuery(""); }
        }}
        onKeyDown={handleTriggerKey}
      >
        <span style={{ color: "var(--text-primary)", fontSize: 13 }}>
          {selectedLabel}
        </span>
        <span style={{
          fontSize: 10, color: "var(--text-tertiary)",
          transition: "transform 150ms ease",
          transform: open ? "rotate(180deg)" : "rotate(0deg)",
        }}>▼</span>
      </div>

      {/* 下拉面板 */}
      {open && (
        <div
          className="glass-elevated"
          style={{
            position: "absolute", top: "100%", left: 0, right: 0,
            marginTop: 4, zIndex: 100, padding: 4,
            animation: "fadeIn 150ms ease both",
          }}
        >
          {/* 搜索输入 */}
          <input
            ref={inputRef}
            className="input"
            placeholder={t("platform.searchPlaceholder", "搜索平台...")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
            style={{ fontSize: 12, padding: "6px 10px", marginBottom: 4, width: "100%" }}
            onBlur={() => { setTimeout(() => { setOpen(false); setQuery(""); }, 150); }}
            onKeyDown={handleInputKey}
          />
          {/* 选项列表 */}
          <div style={{ maxHeight: 256, overflowY: "auto" }} ref={scrollRef}>
            {filtered.length === 0 && (
              <div style={{ padding: "8px 12px", fontSize: 12, color: "var(--text-tertiary)" }}>
                No match
              </div>
            )}
            {filtered.map((p, idx) => {
              const isActive = p.value === value && !!p.codingPlan === codingPlan;
              const isHighlighted = idx === highlightedIndex;
              return (
                <button
                  key={`${p.value}-${p.codingPlan ? 1 : 0}`}
                  type="button"
                  className="btn btn-ghost"
                  style={{
                    width: "100%", justifyContent: "flex-start",
                    padding: "7px 12px", fontSize: 13,
                    fontWeight: isActive ? 600 : 400,
                    color: isActive ? "var(--accent)" : "var(--text-primary)",
                    background: isHighlighted
                      ? "var(--accent-subtle)"
                      : isActive ? "var(--accent-subtle)" : "transparent",
                    borderRadius: "var(--radius-sm)",
                    outline: isHighlighted && !isActive ? "1px solid var(--accent)" : "none",
                  }}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onChange(p.value, p.codingPlan);
                    setOpen(false);
                    setQuery("");
                  }}
                  onMouseEnter={() => setHighlightedIndex(idx)}
                >
                  <span style={{
                    display: "inline-block", padding: "1px 6px", borderRadius: "var(--radius-sm)",
                    background: `${PROTOCOL_COLORS[p.value] || "var(--accent)"}20`,
                    color: PROTOCOL_COLORS[p.value] || "var(--accent)",
                    fontSize: 10, fontWeight: 700, marginRight: 8,
                  }}>
                    {p.value.slice(0, 2).toUpperCase()}
                  </span>
                  {p.label}
                  {p.codingPlan && (
                    <span style={{
                      marginLeft: 6, padding: "1px 5px", borderRadius: "var(--radius-sm)",
                      background: "var(--color-success, #34c759)20",
                      color: "var(--color-success, #34c759)",
                      fontSize: 10, fontWeight: 600,
                    }}>
                      Code
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

/** Mock 平台配置编辑器：编辑 platform.extra 的 mock 子对象 */
interface MockConfigEditorProps {
  config: MockConfig;
  onChange: (next: MockConfig) => void;
}

const MOCK_ERROR_MODES: { value: MockErrorMode; labelKey: string }[] = [
  { value: "none", labelKey: "platform.mockErrorNone" },
  { value: "http_error", labelKey: "platform.mockErrorHttp" },
  { value: "rate_limit_429", labelKey: "platform.mockErrorRateLimit" },
  { value: "timeout", labelKey: "platform.mockErrorTimeout" },
];

function MockConfigEditor({ config, onChange }: MockConfigEditorProps) {
  const { t } = useTranslation();
  const setField = <K extends keyof MockConfig>(key: K, value: MockConfig[K]) => {
    onChange({ ...config, [key]: value });
  };

  const numberField = (label: string, key: "status_code" | "delay_ms" | "input_tokens" | "output_tokens" | "cache_tokens" | "chunk_count", hint?: string) => (
    <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{label}</span>
      <input
        className="input"
        type="number"
        value={config[key]}
        onChange={(e) => setField(key, Number(e.target.value))}
      />
      {hint && <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>{hint}</span>}
    </label>
  );

  // stream_override: null=跟随请求 / true / false → 用三态下拉
  const streamValue = config.stream_override === null ? "follow" : config.stream_override ? "force_on" : "force_off";

  return (
    <div style={{
      display: "flex", flexDirection: "column", gap: 12,
      padding: 12, borderRadius: "var(--radius-sm)",
      background: "var(--bg-glass)", border: "1px solid var(--border)",
    }}>
      <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
        {t("platform.mockConfig")}（{t("platform.mockConfigHint")}）
      </div>

      {/* 响应文本 */}
      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{t("platform.mockResponseText")}（response_text）</span>
        <textarea
          className="input"
          style={{ minHeight: 60, resize: "vertical" }}
          value={config.response_text}
          onChange={(e) => setField("response_text", e.target.value)}
        />
      </label>

      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>finish_reason</span>
        <input
          className="input"
          value={config.finish_reason}
          onChange={(e) => setField("finish_reason", e.target.value)}
        />
      </label>

      {/* 数值字段网格 */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
        {numberField(`${t("platform.mockStatusCode")}（status_code）`, "status_code")}
        {numberField(`${t("platform.mockDelayMs")}（delay_ms）`, "delay_ms")}
        {numberField(`${t("platform.mockInputTokens")}（input_tokens）`, "input_tokens")}
        {numberField(`${t("platform.mockOutputTokens")}（output_tokens）`, "output_tokens")}
        {numberField(`${t("platform.mockCacheTokens")}（cache_tokens）`, "cache_tokens")}
        {numberField(`${t("platform.mockChunkCount")}（chunk_count）`, "chunk_count")}
      </div>

      {/* error_mode + stream_override */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
        <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{t("platform.mockErrorMode")}（error_mode）</span>
          <select
            className="input"
            value={config.error_mode}
            onChange={(e) => setField("error_mode", e.target.value as MockErrorMode)}
          >
            {MOCK_ERROR_MODES.map((m) => (
              <option key={m.value} value={m.value}>{t(m.labelKey)}</option>
            ))}
          </select>
        </label>
        <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{t("platform.mockStreamOverride")}（stream_override）</span>
          <select
            className="input"
            value={streamValue}
            onChange={(e) => {
              const v = e.target.value;
              setField("stream_override", v === "follow" ? null : v === "force_on");
            }}
          >
            <option value="follow">{t("platform.mockStreamFollow")}（null）</option>
            <option value="force_on">{t("platform.mockStreamForceOn")}（true）</option>
            <option value="force_off">{t("platform.mockStreamForceOff")}（false）</option>
          </select>
        </label>
      </div>
    </div>
  );
}

/** 配额展示数据：合并预估(est_*)与真查(quotaMap)，优先级与原列表逻辑一致。
 *  手动刷新校准过(preferReal)→优先真值；否则有预估→预估；冷启动→真查回退。 */
interface QuotaDisplay {
  estimated: boolean;
  /** 余额剩余（用于 BalanceBar）。null 表示无余额数据。 */
  balanceRemaining: number | null;
  balanceTotal: number | null;
  currency: string;
  /** coding plan 各档剩余百分比（0–100，越高越充足）。level 按使用速率算（usageColor，唯一阈值源）。 */
  tiers: { name: string; remainPct: number; utilization: number; resetsAt: string | null; limit: number | null; remaining: number | null; level: ColorLevel }[];
  /** 是否有任意配额数据（余额或 coding plan）。 */
  hasData: boolean;
}

function computeQuotaDisplay(p: Platform, q: PlatformQuota | undefined, preferRealCalibrated: boolean): QuotaDisplay {
  const tierRemain = (utilization: number) => Math.max(0, Math.min(100, 100 - utilization));
  const preferReal = preferRealCalibrated && !!q;
  const estCoding = parseEstCodingPlan(p.est_coding_plan);
  const hasEstBalance = p.est_balance_remaining > 0;
  const hasEst = hasEstBalance || (estCoding !== null && estCoding.tiers.length > 0);

  if (hasEst && !preferReal) {
    const tiers = estCoding
      ? estCoding.tiers.map(tier => {
          const limit = tier.limit ?? null;
          const remaining = limit != null ? Math.round(limit * tierRemain(tier.est_utilization) / 100) : null;
          // 预估侧：remain = window_start + cycle - now（无 window_start → null → 中性）。
          const cycleMs = cycleMsForTier(tier.name);
          const remainMs = tier.window_start && tier.window_start > 0 && cycleMs != null
            ? tier.window_start + cycleMs - Date.now()
            : null;
          const level = codingTierLevel(tier.est_utilization, remainMs, cycleMs);
          const resetsAt = remainMs != null ? new Date(Date.now() + remainMs).toISOString() : null;
          return { name: tier.name, remainPct: tierRemain(tier.est_utilization), utilization: tier.est_utilization, resetsAt, limit, remaining, level };
        })
      : [];
    return {
      estimated: true,
      balanceRemaining: hasEstBalance ? p.est_balance_remaining : null,
      balanceTotal: null,
      currency: q?.balance?.currency || "USD",
      tiers,
      hasData: hasEstBalance || tiers.length > 0,
    };
  }
  if (q) {
    const tiers = q.coding_plan
      ? q.coding_plan.tiers.map(tier => {
          // 真查侧：remain = resets_at - now（无 resets_at → null → 中性）。
          const cycleMs = cycleMsForTier(tier.name);
          const resetsMs = tier.resets_at ? new Date(tier.resets_at).getTime() : NaN;
          const remainMs = Number.isFinite(resetsMs) && cycleMs != null ? resetsMs - Date.now() : null;
          const level = codingTierLevel(tier.utilization, remainMs, cycleMs);
          return { name: tier.name, remainPct: tierRemain(tier.utilization), utilization: tier.utilization, resetsAt: tier.resets_at, limit: tier.limit, remaining: tier.remaining, level };
        })
      : [];
    return {
      estimated: false,
      balanceRemaining: q.balance ? q.balance.remaining : null,
      balanceTotal: q.balance?.total ?? null,
      currency: q.balance?.currency || "USD",
      tiers,
      hasData: !!q.balance || tiers.length > 0,
    };
  }
  return { estimated: false, balanceRemaining: null, balanceTotal: null, currency: "USD", tiers: [], hasData: false };
}

/** coding plan 档名 → 简短标签 */
function tierLabel(name: string): string {
  if (name === "five_hour") return "5h";
  if (name === "weekly_limit") return "week";
  if (name === "mcp_monthly") return "MCP";
  return name;
}

/** ISO 8601 或 millis → 剩余时间人类可读字符串 */
function formatResetCountdown(resetsAt: string | null): string {
  if (!resetsAt) return "";
  const ts = new Date(resetsAt).getTime();
  if (isNaN(ts)) return "";
  const diffMs = ts - Date.now();
  if (diffMs <= 0) return "";
  const diffMin = Math.ceil(diffMs / 60000);
  const diffHours = Math.floor(diffMin / 60);
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays > 0) return `${diffDays}d ${diffHours % 24}h`;
  if (diffHours > 0) return `${diffHours}h ${diffMin % 60}m`;
  return `${diffMin}m`;
}

// ── 手动预算（无上游 quota 平台）──

/** 生成一条新手动预算的默认值（uuid id + total/usd）。 */
function newManualBudget(): ManualBudget {
  const id = (typeof crypto !== "undefined" && crypto.randomUUID)
    ? crypto.randomUUID().replace(/-/g, "")
    : Math.random().toString(36).slice(2) + Date.now().toString(36);
  return { id, kind: "total", unit: "usd", amount: 0, window_hours: null, window_unit: "hour", consumed: 0, window_start_at: null, enabled: true };
}

/** 手动预算剩余展示数据（取剩余比例最低那条；token 单位尽力折算，缺价显 token）。 */
interface ManualBudgetDisplay {
  hasData: boolean;
  /** 剩余值（usd 单位为 $；token 单位为 token 数）。 */
  remaining: number;
  amount: number;
  unit: ManualBudgetUnit;
  kind: ManualBudgetKind;
  /** 剩余占比 0–1，越低越紧。 */
  ratio: number;
  depleted: boolean;
}

/** 从平台 manual_budgets 选「剩余比例最低」那条用于卡片展示。 */
function computeManualBudgetDisplay(budgets: ManualBudget[] | undefined): ManualBudgetDisplay | null {
  const enabled = (budgets ?? []).filter(b => b.enabled && b.amount > 0);
  if (enabled.length === 0) return null;
  let tightest: ManualBudget | null = null;
  let minRatio = Infinity;
  for (const b of enabled) {
    const rem = b.amount - b.consumed;
    const ratio = b.amount > 0 ? rem / b.amount : 0;
    if (ratio < minRatio) { minRatio = ratio; tightest = b; }
  }
  if (!tightest) return null;
  const rem = tightest.amount - tightest.consumed;
  return {
    hasData: true,
    remaining: rem,
    amount: tightest.amount,
    unit: tightest.unit,
    kind: tightest.kind,
    ratio: Math.max(0, Math.min(1, minRatio === Infinity ? 0 : minRatio)),
    depleted: rem <= 0,
  };
}

/** 编辑页分区卡片：glass-surface 容器 + 标题 + 可选描述 + 内容区，统一视觉层次。 */
interface FormSectionProps {
  title: string;
  desc?: string;
  /** 标题右侧操作区（如「添加端点」「获取模型」按钮）。 */
  action?: React.ReactNode;
  children: React.ReactNode;
}

function FormSection({ title, desc, action, children }: FormSectionProps) {
  return (
    <div
      className="glass-surface"
      style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, borderRadius: "var(--radius-md)" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 13, fontWeight: 700, color: "var(--text-primary)" }}>{title}</div>
          {desc && (
            <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.4, marginTop: 2 }}>{desc}</div>
          )}
        </div>
        {action && <div style={{ flexShrink: 0 }}>{action}</div>}
      </div>
      {children}
    </div>
  );
}

/** 平台卡片操作集合（父组件以 latest-ref 方式提供稳定引用，避免破坏 memo） */
interface PlatformCardActions {
  onPointerDown: (e: React.PointerEvent, index: number) => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onPointerUp: () => void;
  onToggleExpanded: (id: number, next: boolean) => void;
  onRefreshQuota: (p: Platform) => void;
  onToggleEnabled: (p: Platform) => void;
  onEdit: (p: Platform) => void;
  onDelete: (id: number) => void;
  onQuickTest: (p: Platform) => void;
  onCustomTest: (p: Platform) => void;
  onFaviconFailed: (id: number) => void;
}

interface PlatformCardProps {
  platform: Platform;
  index: number;
  isDragging: boolean;
  dragActive: boolean;
  quota: PlatformQuota | undefined;
  preferReal: boolean;
  refreshing: boolean;
  usage: PlatformUsageStats | undefined;
  expanded: boolean;
  manualResult: "ok" | "fail" | undefined;
  testing: boolean;
  faviconFailed: boolean;
  actions: PlatformCardActions;
}

const PlatformCard = memo(function PlatformCard({
  platform: p,
  index: i,
  isDragging,
  dragActive,
  quota: q,
  preferReal,
  refreshing,
  usage: u,
  expanded,
  manualResult: manual,
  testing,
  faviconFailed: faviconHasFailed,
  actions,
}: PlatformCardProps) {
  const { t } = useTranslation();
  const color = PROTOCOL_COLORS[p.platform_type] || "var(--accent)";
  const configuredModels = allModelValues(p.models);
  const quota = computeQuotaDisplay(p, q, preferReal);
  const showQuota = p.platform_type !== "mock" && p.platform_type !== "claude_code" && quota.hasData;
  const mb = computeManualBudgetDisplay(p.manual_budgets);  const total = u ? u.total_input_tokens + u.total_output_tokens : 0;
  const sr = u && u.total_requests > 0 ? (u.success_count / u.total_requests * 100) : 0;
  const hasDetail = !!u || (p.endpoints && p.endpoints.length > 0) || configuredModels.length > 0 || quota.tiers.length > 0;
  const health = manual
    ? (manual === "ok" ? "healthy" : "error")
    : u ? healthStatus(u.recent_total, u.recent_failures) : "unknown";
  const logoSvg = getPlatformLogo(p.platform_type);
  const favicon = !logoSvg && !faviconHasFailed ? getFaviconUrl(p) : null;
  const getBaseUrl = (proto: Protocol, eps: PlatformEndpoint[]): string => {
    const primary = eps.find(ep => ep.protocol === proto);
    if (primary) return primary.base_url;
    return eps[0]?.base_url || "";
  };
  return (
                  <div
                    data-platform-id={p.id}
                    style={{
                      animationDelay: `${i * 50}ms`,
                      opacity: dragActive ? (isDragging ? 0 : 0.4) : p.enabled ? 1 : 0.5,
                      ...(isDragging ? { height: 0, overflow: "hidden", padding: 0, margin: 0, borderWidth: 0, minHeight: 0 } : {}),
                      transition: "opacity 150ms ease",
                    }}
                  >
                  <CompactCard
                    expanded={hasDetail ? expanded : undefined}
                    onToggle={hasDetail ? (next) => actions.onToggleExpanded(p.id, next) : undefined}
                    toggleLabel={t("platform.toggleDetail", "展开/收起明细")}
                    header={(
                      <div style={{ display: "flex", flexDirection: "column", gap: 10, minWidth: 0 }}>
                        {/* ── 行 1：身份 + 快操作 ── */}
                        <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0 }}>
                        {/* 拖拽把手 */}
                        <div
                          className={`drag-handle-inline${isDragging ? " is-active" : ""}`}
                          style={{ cursor: "grab", color: "var(--text-tertiary)", flexShrink: 0, display: "flex", touchAction: "none" }}
                          onPointerDown={e => actions.onPointerDown(e, i)}
                          onPointerMove={actions.onPointerMove}
                          onPointerUp={actions.onPointerUp}
                          title={t("platform.dragReorder", "拖拽排序")}
                        >
                          <svg width="12" height="18" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
                        </div>
                        {/* Logo + 健康点 */}
                        <div style={{ position: "relative", flexShrink: 0 }}>
                          <div style={{
                            width: 36, height: 36, borderRadius: "var(--radius-sm)",
                            display: "flex", alignItems: "center", justifyContent: "center",
                            background: (logoSvg || favicon) ? "transparent" : `${color}15`,
                            border: `1px solid ${color}30`,
                            color: color, fontSize: 12, fontWeight: 700, overflow: "hidden",
                          }}>
                            {logoSvg
                              ? <img src={logoSvg} alt={p.platform_type} style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
                              : favicon
                                ? <img src={favicon} alt={p.platform_type}
                                    style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }}
                                    onError={() => actions.onFaviconFailed(p.id)}
                                  />
                                : p.platform_type.slice(0, 2).toUpperCase()
                            }
                          </div>
                          {health !== "unknown" && (
                            <div style={{
                              position: "absolute", top: -3, right: -3,
                              width: 10, height: 10, borderRadius: "50%",
                              background: HEALTH_COLORS[health],
                              border: "2px solid var(--bg-primary)",
                              boxShadow: `0 0 4px ${HEALTH_COLORS[health]}60`,
                            }} />
                          )}
                        </div>
                        {/* 名称 + 协议·base_url */}
                        <div style={{ minWidth: 0, flex: 1 }}>
                          <div style={{ fontWeight: 600, fontSize: 14, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.name}</div>
                          <div className="text-secondary" style={{ fontSize: 11, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {p.platform_type.toUpperCase()} · {getBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url}
                          </div>
                          {p.status === "auto_disabled" && (
                            <div
                              style={{
                                marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
                                fontSize: 10, fontWeight: 600, color: "var(--color-warning)",
                                background: "color-mix(in srgb, var(--color-warning) 14%, transparent)",
                                border: "1px solid color-mix(in srgb, var(--color-warning) 35%, transparent)",
                                borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
                              }}
                              title={t("platform.autoDisabledHint", "401/403 自动禁用，下次试探时间 {{time}}")
                                .replace("{{time}}", p.auto_disabled_until > 0 ? new Date(p.auto_disabled_until).toLocaleString() : "-")}
                            >
                              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                                <path d="M12 9v4" /><path d="M12 17h.01" />
                                <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                              </svg>
                              {t("platform.autoDisabled", "自动禁用")}
                            </div>
                          )}
                        </div>
                        {/* 快操作 */}
                        <div style={{ display: "flex", gap: 4, flexShrink: 0, alignItems: "center" }}>
                          {showQuota && (
                            <button
                              className="btn btn-ghost btn-icon"
                              style={{ padding: 4, lineHeight: 0, minWidth: "auto" }}
                              disabled={refreshing}
                              title={t("platform.quotaRefresh", "刷新额度")}
                              onClick={(e) => { e.stopPropagation(); actions.onRefreshQuota(p); }}
                            >
                              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                                strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"
                                style={refreshing ? { animation: "spin 0.9s linear infinite" } : undefined}>
                                <path d="M21 12a9 9 0 1 1-2.64-6.36" />
                                <polyline points="21 3 21 9 15 9" />
                              </svg>
                            </button>
                          )}
                          <div
                            className={`toggle ${p.status === "enabled" ? "active" : ""}`}
                            style={{ cursor: "pointer" }}
                            onClick={(e) => { e.stopPropagation(); actions.onToggleEnabled(p); }}
                            title={p.status === "enabled"
                              ? t("platform.disable", "禁用")
                              : p.status === "auto_disabled"
                                ? t("platform.reenable", "重新启用")
                                : t("platform.enable", "启用")}
                          />
                          <div style={{ display: "inline-flex", fontSize: 11 }}>
                            <button
                              className="btn btn-ghost"
                              style={{ fontSize: 11, gap: 4, padding: "3px 8px", borderRadius: "6px 0 0 6px", borderRight: "1px solid var(--border)" }}
                              disabled={testing}
                              onClick={(e) => { e.stopPropagation(); actions.onQuickTest(p); }}
                              title={t("platform.quickTest", "快速测试默认模型")}
                            >
                              <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" stroke="none">
                                <path d="M13 2L4 14h7l-2 8 9-12h-7l2-8z"/>
                              </svg>
                              {testing ? "..." : t("platform.quickTest", "快速测试")}
                            </button>
                            <button
                              className="btn btn-ghost"
                              style={{ fontSize: 11, padding: "3px 6px", borderRadius: "0 6px 6px 0" }}
                              onClick={(e) => { e.stopPropagation(); actions.onCustomTest(p); }}
                              title={t("platform.customTest", "自定义测试")}
                            >
                              <svg width="10" height="10" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <path d="M3 5l4 4 4-4" />
                              </svg>
                            </button>
                          </div>
                          <button className="btn btn-ghost btn-icon" onClick={(e) => { e.stopPropagation(); actions.onEdit(p); }}>
                            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                              <path d="M10 2l2 2-7 7H3v-2l7-7z" />
                            </svg>
                          </button>
                          <button className="btn btn-ghost btn-icon btn-danger" onClick={(e) => { e.stopPropagation(); actions.onDelete(p.id); }}>
                            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                              <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                            </svg>
                          </button>
                        </div>
                        </div>
                        {/* ── 行 2：余额 / 预算 / coding tiers ── */}
                        {showQuota && (quota.balanceRemaining != null || (mb && mb.hasData) || (quota.balanceRemaining == null && quota.tiers.length > 0)) && (
                          <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", paddingLeft: 24 }}>
                        {/* 余额（直显，缺值不渲染）。颜色按后端 balance_level（使用速率）；
                            neutral（无消费数据）→ undefined 回退现有 total 占比色。 */}
                        {quota.balanceRemaining != null && (() => {
                          const balColor = usageLevelToColor(p.balance_level);
                          return (
                            <div style={{ flexShrink: 0, width: 120, display: "flex", flexDirection: "column", gap: 2 }}>
                              <BalanceBar remaining={quota.balanceRemaining} total={quota.balanceTotal} currency={quota.currency === "USD" ? "$" : quota.currency} level={balColor === "neutral" ? undefined : balColor} />
                            </div>
                          );
                        })()}
                        {/* 手动预算剩余（无上游 quota 平台；取最紧那条，耗尽 danger 标记）*/}
                        {mb && mb.hasData && (
                          <div style={{ flexShrink: 0, width: 120, display: "flex", flexDirection: "column", gap: 2 }}>
                            {mb.unit === "usd" ? (
                              <BalanceBar remaining={mb.remaining} total={mb.amount} currency="$" />
                            ) : (
                              <div style={{ display: "flex", flexDirection: "column", gap: 3, minWidth: 0 }}>
                                <span style={{ fontWeight: 700, fontSize: 12, color: mb.depleted ? "var(--color-danger)" : mb.ratio < 0.2 ? "var(--color-warning)" : "var(--text-primary)" }}>
                                  {formatNumber(Math.max(0, mb.remaining))}
                                  <span style={{ fontSize: 9, color: "var(--text-tertiary)", marginLeft: 3 }}>/ {formatNumber(mb.amount)} tok</span>
                                </span>
                                <div style={{ height: 4, borderRadius: "var(--radius-sm)", background: "var(--bg-glass)", overflow: "hidden" }}>
                                  <div style={{ width: `${mb.ratio * 100}%`, height: "100%", background: mb.depleted ? "var(--color-danger)" : mb.ratio < 0.2 ? "var(--color-warning)" : "var(--color-success)", borderRadius: "var(--radius-sm)", transition: "width 0.3s ease" }} />
                                </div>
                              </div>
                            )}
                            <span style={{ fontSize: 9, fontWeight: 700, color: mb.depleted ? "var(--color-danger)" : "var(--text-tertiary)" }}>
                              {mb.depleted
                                ? t("platform.manualBudgetDepleted", "额度耗尽")
                                : t("platform.manualBudgetLabel", "手动预算")}
                              {mb.unit === "token" && ` · ${t("platform.manualBudgetTokenApprox", "≈未知$")}`}
                            </span>
                          </div>
                        )}
                        {/* Coding plan tiers（无余额时展示最紧急 tier） */}
                        {quota.balanceRemaining == null && quota.tiers.length > 0 && (
                          <div style={{ flexShrink: 0, display: "flex", gap: 4, flexWrap: "wrap", maxWidth: 260 }}>
                            {quota.tiers.map(tier => {
                              const isMcp = tier.name === "mcp_monthly";
                              const value = isMcp && tier.limit != null
                                ? `${tier.remaining ?? 0}/${tier.limit}`
                                : `${tier.remainPct.toFixed(0)}%`;
                              const tierColor = levelColor(tier.level);
                              const countdown = formatResetCountdown(tier.resetsAt);
                              return (
                                <span key={tier.name} style={{
                                  display: "inline-flex", alignItems: "center", gap: 3,
                                  padding: "2px 6px", borderRadius: "var(--radius-sm)",
                                  fontSize: 10, fontWeight: 600,
                                  background: tier.level === "neutral" ? "var(--bg-glass)" : levelBg(tier.level),
                                  color: tierColor,
                                }}>
                                  <span style={{ fontSize: 11, fontWeight: 700 }}>{value}</span>
                                  <span style={{ fontSize: 9, opacity: 0.7 }}>{tierLabel(tier.name)}</span>
                                  {countdown && <span style={{ fontSize: 8, opacity: 0.6 }}>·{countdown}</span>}
                                </span>
                              );
                            })}
                          </div>
                        )}
                          </div>
                        )}
                      </div>
                    )}
                  >
                    {hasDetail && (
                      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                        {/* 已使用统计（色编码，点击展开后才见） */}
                        {u && (
                          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                            <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.usageLabel", "已使用")}</span>
                            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                              <StatChip icon={<IconBolt size={13} />} value={formatNumber(total)} label="tokens" />
                              <StatChip icon={<IconCost size={13} />} value={`$${formatCost(u.total_cost)}`} label="cost" level={costLevel(u.total_cost)} />
                              <StatChip icon={<IconCheck size={13} />} value={formatPercent(sr)} label="ok" level={successRateLevel(sr, u.total_requests)} />
                            </div>
                          </div>
                        )}
                        {/* 配额各档明细（coding plan tiers，色编码） */}
                        {showQuota && quota.tiers.length > 0 && (
                          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                            <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.quotaLabel", "额度")}</span>
                            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                              {quota.tiers.map(tier => {
                                const countdown = formatResetCountdown(tier.resetsAt);
                                const value = tier.name === "mcp_monthly" && tier.limit != null
                                  ? `${tier.remaining ?? 0}/${tier.limit}`
                                  : `${tier.remainPct.toFixed(0)}%`;
                                return (
                                  <div key={tier.name} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                                    <StatChip icon={<IconCoin size={13} />}
                                      value={value}
                                      label={tierLabel(tier.name)}
                                      level={tier.level} />
                                    {countdown && (
                                      <span className="text-tertiary" style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 10, fontWeight: 600, paddingLeft: 2 }}>
                                        <IconClock size={11} />
                                        {t("platform.resetIn", "重置 {{countdown}}", { countdown })}
                                      </span>
                                    )}
                                  </div>
                                );
                              })}
                            </div>
                          </div>
                        )}
                        {/* Endpoints badges */}
                        {p.endpoints && p.endpoints.length > 0 && (
                          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                            <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.endpoints", "Protocol Endpoints")}</span>
                            <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                              {p.endpoints.map((ep, ei) => (
                                <span key={ei} className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px", opacity: 0.85 }}>
                                  {PROTOCOL_LABELS[ep.protocol] || ep.protocol}
                                  {ep.coding_plan && <span style={{ color: "var(--color-success)", marginLeft: 2, fontWeight: 700 }}>Code</span>}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}
                        {/* 模型 badges */}
                        {configuredModels.length > 0 && (
                          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                            <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.models")}</span>
                            <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                              {configuredModels.map((m, mi) => (
                                <span key={mi} className="badge badge-muted" style={{ fontSize: 11, padding: "2px 6px" }}>{m}</span>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    )}
                  </CompactCard>
                  </div>
  );
});

export function Platforms() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  // ── Drag reorder for platform list ──
  const [platDrag, setPlatDrag] = useState<{ from: number; to: number } | null>(null);
  const platListRef = useRef<HTMLDivElement>(null);
  const platDragStartRef = useRef<{ y: number; index: number } | null>(null);
  const platDidDragRef = useRef(false);
  // 拖拽 geometry 计算 rAF 节流：每帧最多算一次，避免逐 pointermove 全列 getBoundingClientRect
  const platDragRafRef = useRef<number | null>(null);
  const platDragYRef = useRef(0);

  const handlePlatPointerDown = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    platDragStartRef.current = { y: e.clientY, index };
  };

  // rAF 内执行：基于最新 clientY 重算插入位置
  const computeDragTarget = (clientY: number) => {
    const start = platDragStartRef.current;
    if (!start) return;
    if (!platDrag) {
      if (Math.abs(clientY - start.y) < 5) return;
      setPlatDrag({ from: start.index, to: start.index });
      platDidDragRef.current = true;
    }
    if (!platListRef.current) return;
    const cards = platListRef.current.querySelectorAll<HTMLElement>("[data-platform-id]");
    let newTo = cards.length;
    for (let i = 0; i < cards.length; i++) {
      const rect = cards[i].getBoundingClientRect();
      if (clientY < rect.top + rect.height / 2) { newTo = i; break; }
    }
    setPlatDrag(d => d ? { ...d, to: newTo } : null);
  };

  const handlePlatPointerMove = (e: React.PointerEvent) => {
    if (!platDragStartRef.current) return;
    platDragYRef.current = e.clientY; // 始终记录最新位置
    if (platDragRafRef.current !== null) return; // 本帧已排程，下一帧用最新 Y
    platDragRafRef.current = requestAnimationFrame(() => {
      platDragRafRef.current = null;
      computeDragTarget(platDragYRef.current);
    });
  };

  const handlePlatPointerUp = () => {
    if (platDragRafRef.current !== null) {
      cancelAnimationFrame(platDragRafRef.current);
      platDragRafRef.current = null;
    }
    if (platDrag) {
      const effectiveTo = platDrag.from < platDrag.to ? platDrag.to - 1 : platDrag.to;
      if (platDrag.from !== effectiveTo) {
        const reordered = [...platforms];
        const [moved] = reordered.splice(platDrag.from, 1);
        reordered.splice(effectiveTo, 0, moved);
        setPlatforms(reordered);
        platformApi.reorder(reordered.map(pp => pp.id)).catch(console.error);
      }
    }
    setPlatDrag(null);
    platDragStartRef.current = null;
    setTimeout(() => { platDidDragRef.current = false; }, 50);
  };
  const [usageMap, setUsageMap] = useState<Record<number, PlatformUsageStats>>({});
  const [quotaMap, setQuotaMap] = useState<Record<number, PlatformQuota>>({});
  // 手动刷新（真查校准）后的平台 id → 优先展示 quotaMap 真值而非预估
  const [quotaRealIds, setQuotaRealIds] = useState<Record<number, boolean>>({});
  const [quotaRefreshing, setQuotaRefreshing] = useState<Record<number, boolean>>({});
  const [testResults, setTestResults] = useState<Record<number, "ok" | "fail">>({});
  /** favicon 加载失败的平台 ID 集合（回退到文字缩写） */
  const [faviconFailed, setFaviconFailed] = useState<Set<number>>(new Set());
  /** 列表卡片已展开（显 endpoints/模型明细）的平台 ID 集合 */
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
  const toggleExpanded = (id: number, next: boolean) => {
    setExpandedIds(prev => {
      const s = new Set(prev);
      if (next) s.add(id); else s.delete(id);
      return s;
    });
  };
  const [testingId, setTestingId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<Platform | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");
  const [saveError, setSaveError] = useState("");
const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);
  const [toast, setToast] = useState<{ text: string; ok: boolean } | null>(null);
  const [showKey, setShowKey] = useState(false);

  // Form state
  const [name, setName] = useState("OpenAI");
  const [protocol, setProtocol] = useState<Protocol>("openai");
  const [codingPlan, setCodingPlan] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<Record<ModelSlot, string>>({
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  });
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [endpoints, setEndpoints] = useState<PlatformEndpoint[]>([]);
  const [activeDropdown, setActiveDropdown] = useState<ModelSlot | null>(null);
  const [showClaudeConfig, setShowClaudeConfig] = useState(false);
  const [claudeConfigJson, setClaudeConfigJson] = useState("");
  const [globalClaudeConfig, setGlobalClaudeConfig] = useState<Record<string, any>>({});
  // Mock 平台配置（持久化到 platform.extra 的 mock 子对象）
  const [extra, setExtra] = useState("");
  const [mockConfig, setMockConfig] = useState<MockConfig>({ ...DEFAULT_MOCK_CONFIG });
  const [newApiConfig, setNewApiConfig] = useState<NewApiConfig>({ ...DEFAULT_NEWAPI_CONFIG });
  // 手动预算限额（仅无上游 quota 自动支持平台可配；编辑表单态）
  const [manualBudgets, setManualBudgets] = useState<ManualBudget[]>([]);

  const isMock = protocol === "mock";
  // Claude Code 订阅纯透传：客户端自带订阅 OAuth 认证，aidog 原样转发。
  // 仅需 base_url（host 根），api_key 可空，隐藏 endpoints/models 编辑。
  const isPassthrough = protocol === "claude_code";

  /** 从 endpoints 中推导主 base_url（匹配主协议的 endpoint，否则取第一个） */
  const getPrimaryBaseUrl = (proto: Protocol, eps: PlatformEndpoint[]): string => {
    const primary = eps.find(ep => ep.protocol === proto);
    if (primary) return primary.base_url;
    return eps[0]?.base_url || "";
  };

  const handleProtocolChange = (newProtocol: Protocol, newCodingPlan?: boolean) => {
    const cp = !!newCodingPlan;
    // Auto-fill name with protocol label if empty or still at a default name
    if (!name.trim() || DEFAULT_NAMES.has(name)) {
      setName(cp ? `${PROTOCOL_LABELS[newProtocol]} Coding Plan` : PROTOCOL_LABELS[newProtocol]);
    }
    // Auto-fill endpoints from defaults（mock 无真实上游，返回空）
    const defaultEps = getDefaultEndpoints(newProtocol, cp);
    if (defaultEps.length > 0) {
      setEndpoints(defaultEps);
    } else {
      setEndpoints([]);
    }
    // 切到 mock 时用当前 extra 初始化 mock 配置编辑器
    if (newProtocol === "mock") {
      setMockConfig(parseMockConfig(extra));
    }
    // 切到 newapi 时用当前 extra 初始化 newapi 配置
    if (newProtocol === "newapi") {
      setNewApiConfig(parseNewApiConfig(extra));
    }
    setProtocol(newProtocol);
    setCodingPlan(cp);
  };

  const load = async () => {
    setLoading(true);
    let list: Platform[] = [];
    try {
      list = (await platformApi.list()) || [];
      setPlatforms(list);
    } catch (e) { console.error(e); }
    // 平台列表到手即渲染，余额/用量改后台渐进填充，禁止外部 quota HTTP 阻塞整页
    setLoading(false);

    // Usage stats（本地查询）渐进填充
    list.forEach(async (p) => {
      try {
        const s = await platformApi.usageStats(p.id);
        if (s && s.total_requests > 0) setUsageMap(prev => ({ ...prev, [p.id]: s }));
      } catch { /* ignore */ }
    });

    // Quota（balance & coding plan，外部 HTTP，慢）逐平台返回逐个更新
    // 传 platformId → 后端 calibrate_from_quota 校准 est_balance/est_coding_plan
    list.forEach(async (p) => {
      if (!p.api_key) return;
      const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
      if (!baseUrl) return;
      try {
        const q = p.platform_type === "newapi"
          ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
          : await quotaApi.query(baseUrl, p.api_key, p.id);
        if (q.success) setQuotaMap(prev => ({ ...prev, [p.id]: q }));
      } catch { /* ignore */ }
    });
  };

  /** 轻量刷新：更新平台列表（含 est_balance/est_coding_plan）+ usage stats，不拉 quota HTTP */
  const refreshStats = async () => {
    try {
      const list = await platformApi.list();
      if (list) {
        setPlatforms(list);
        list.forEach(async (p) => {
          try {
            const s = await platformApi.usageStats(p.id);
            if (s && s.total_requests > 0) setUsageMap(prev => ({ ...prev, [p.id]: s }));
          } catch { /* ignore */ }
        });
      }
    } catch { /* ignore */ }
  };

  useEffect(() => { load(); }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  /** 刷新单个平台 quota（合查 balance + coding_plan） */
  const refreshQuota = async (p: Platform) => {
    if (!p.api_key) {
      setToast({ text: `${p.name}: ${t("platform.quotaNoKey", "缺少 API Key")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
      return;
    }
    setQuotaRefreshing((s) => ({ ...s, [p.id]: true }));
    try {
      const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url;
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) {
        setQuotaMap((s) => ({ ...s, [p.id]: q }));
        setQuotaRealIds((s) => ({ ...s, [p.id]: true }));
        // New API: 自动回填 user_id
        if (p.platform_type === "newapi" && q.newapi_user_id && editing?.id === p.id) {
          setNewApiConfig(prev => prev.user_id ? prev : { ...prev, user_id: q.newapi_user_id! });
        }
      } else {
        setToast({ text: `${p.name}: ${q.error || t("platform.quotaRefreshFail", "刷新额度失败")}`, ok: false });
        setTimeout(() => setToast(null), 3000);
      }
    } catch (e) {
      console.error(e);
      setToast({ text: `${p.name}: ${t("platform.quotaRefreshFail", "刷新额度失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
    setQuotaRefreshing((s) => ({ ...s, [p.id]: false }));
  };

  const resetForm = () => {
    setName(""); setProtocol("openai"); setCodingPlan(false); setApiKey("");
    setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "" });
    setAvailableModels([]); setEndpoints([]);
    setEditing(null); setShowForm(false); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
    setExtra(""); setMockConfig({ ...DEFAULT_MOCK_CONFIG });
    setNewApiConfig({ ...DEFAULT_NEWAPI_CONFIG });
    setManualBudgets([]);
  };

  const handleEdit = async (p: Platform) => {
    setName(p.name); setProtocol(p.platform_type); setApiKey(p.api_key);
    // 检测 endpoints 中是否有 coding_plan
    const hasCodingPlan = (p.endpoints || []).some(ep => ep.coding_plan);
    setCodingPlan(hasCodingPlan);
    setModels({
      default: p.models.default ?? "",
      sonnet: p.models.sonnet ?? "",
      opus: p.models.opus ?? "",
      haiku: p.models.haiku ?? "",
      gpt: p.models.gpt ?? "",
    });
    setAvailableModels(p.available_models ?? []);
    setEndpoints(p.endpoints ?? []);
    setEditing(p); setShowForm(true); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
    setExtra(p.extra ?? "");
    setMockConfig(parseMockConfig(p.extra ?? ""));
    setNewApiConfig(parseNewApiConfig(p.extra ?? ""));
    setManualBudgets(p.manual_budgets ?? []);

    // Load global + platform Claude Code config
    try {
      const [globalResult, platformResult] = await Promise.all([
        settingsApi.get("global", "claude_code"),
        settingsApi.get(`platform:${p.id}`, "claude_code"),
      ]);
      const gv = (globalResult as Record<string, any>) ?? {};
      const pv = (platformResult as Record<string, any>) ?? {};
      setGlobalClaudeConfig(gv);
      setClaudeConfigJson(JSON.stringify({ ...gv, ...pv }, null, 2));
    } catch (e) { console.error(e); }
  };

  const handleModelChange = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 从下拉选择一个模型填入指定槽位 */
  const handleModelSelect = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 一键获取：获取模型列表 + 自动分类 + 持久化
   *  默认使用 OpenAI 协议 endpoint，回退到主协议 endpoint */
  const handleFetchModels = async () => {
    const openaiEp = endpoints.find(ep => ep.protocol === "openai");
    const fetchUrl = openaiEp?.base_url || getPrimaryBaseUrl(protocol, endpoints);
    if (!fetchUrl || !apiKey) return;
    setFetching(true); setFetchError("");
    try {
      const fetchProtocol: Protocol = openaiEp ? "openai" : protocol;
      const modelIds = await platformApi.fetchModels(fetchProtocol, fetchUrl, apiKey);
      if (modelIds.length === 0) {
        setFetchError(t("platform.fetchEmpty"));
      } else {
        setAvailableModels(modelIds);
        const categorized = autoCategorize(modelIds);
        setModels(categorized);
      }
    } catch (e: any) {
      setFetchError(e.toString());
    }
    setFetching(false);
  };

  /** 一键填充：把 default 模型填到所有槽位（覆盖已有值） */
  const handleFillAll = () => {
    const defaultModel = models.default.trim();
    if (!defaultModel) return;
    setModels(prev => {
      const next = { ...prev };
      for (const slot of MODEL_SLOTS) {
        if (slot.key !== "default") {
          next[slot.key] = defaultModel;
        }
      }
      return next;
    });
  };

  const buildModelsPayload = () => {
    const result: Record<string, string | undefined> = {};
    let hasAny = false;
    for (const slot of MODEL_SLOTS) {
      const v = models[slot.key].trim();
      if (v) { result[slot.key] = v; hasAny = true; }
      else { result[slot.key] = undefined; }
    }
    return hasAny ? result : undefined;
  };

  const handleSave = async () => {
    setSaveError("");
    try {
      const modelsPayload = buildModelsPayload() as Platform["models"] | undefined;
      const availablePayload = availableModels.length > 0 ? availableModels : undefined;
      const baseUrl = getPrimaryBaseUrl(protocol, endpoints);
      // mock 平台：把配置写回 extra；newapi 平台写回 newapi 配置；其余原样保留
      let extraPayload = extra;
      if (isMock) extraPayload = serializeMockConfig(extra, mockConfig);
      if (protocol === "newapi") extraPayload = serializeNewApiConfig(extraPayload, newApiConfig);
      const extraArg = extraPayload ? extraPayload : undefined;
      // 手动预算：所有平台可设（含 mock / 有上游配额支持的平台），仅透传订阅强制清空。
      const manualBudgetsPayload: ManualBudget[] = isPassthrough ? [] : manualBudgets;
      let savedId: number | undefined;
      if (editing) {
        await platformApi.update({
          id: editing.id, name, platform_type: protocol, base_url: baseUrl, api_key: apiKey,
          extra: extraArg,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
          manual_budgets: manualBudgetsPayload,
        });
        savedId = editing.id;
      } else {
        const created = await platformApi.create({
          name, platform_type: protocol, base_url: baseUrl, api_key: apiKey,
          extra: extraArg,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
          manual_budgets: manualBudgetsPayload.length > 0 ? manualBudgetsPayload : undefined,
        });
        savedId = created.id;
      }

      // Save Claude Code config overrides for this platform
      if (savedId && showClaudeConfig && claudeConfigJson.trim()) {
        try {
          const merged = JSON.parse(claudeConfigJson);
          const diff: Record<string, any> = {};
          for (const [k, v] of Object.entries(merged)) {
            if (JSON.stringify(v) !== JSON.stringify(globalClaudeConfig[k])) {
              diff[k] = v;
            }
          }
          if (Object.keys(diff).length > 0) {
            await settingsApi.set(`platform:${savedId}`, "claude_code", diff);
          } else {
            await settingsApi.delete(`platform:${savedId}`, "claude_code");
          }
        } catch (e) { /* ignore JSON parse errors for config */ }
      }

      resetForm(); load();
    } catch (e: any) {
      const msg = e?.toString() || "Unknown error";
      console.error(msg);
      setSaveError(msg);
    }
  };

  const handleDelete = async (id: number) => {
    try { await platformApi.delete(id); load(); } catch (e) { console.error(e); }
  };

  const handleToggle = async (p: Platform) => {
    try {
      // 三态切换：enabled → disabled；disabled / auto_disabled → enabled（恢复并清退避）。
      const nextStatus = p.status === "enabled" ? "disabled" : "enabled";
      await platformApi.update({ id: p.id, status: nextStatus });
      load();
    } catch (e) { console.error(e); }
  };

  const handleQuickTest = async (p: Platform) => {
    setTestingId(p.id);
    try {
      const defaultModel = p.models.default || p.available_models[0] || "";
      const r = await modelTestApi.test({ platform_id: p.id, model: defaultModel, max_tokens: 64 });
      setTestResults(prev => ({ ...prev, [p.id]: r.success ? "ok" : "fail" }));
      setToast({ text: r.success
        ? `${p.name}: ${t("platform.testOk", "测试成功")}${r.duration_ms > 0 ? ` (${r.duration_ms}ms)` : ""}`
        : `${p.name}: ${r.error || t("platform.testFail", "测试失败")}`,
        ok: r.success });
    } catch (err: any) {
      setTestResults(prev => ({ ...prev, [p.id]: "fail" }));
      setToast({ text: `${p.name}: ${err?.message || t("platform.testFail", "测试失败")}`, ok: false });
    }
    setTestingId(null);
    setTimeout(() => setToast(null), 3000);
  };

  // 卡片操作集合：用 latest-ref 持有最新闭包，对外暴露稳定引用，保证 PlatformCard memo 生效
  const actionsRef = useRef({
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    toggleExpanded, refreshQuota, handleToggle, handleEdit, handleDelete,
    handleQuickTest, setTestingPlatform, setFaviconFailed,
  });
  actionsRef.current = {
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    toggleExpanded, refreshQuota, handleToggle, handleEdit, handleDelete,
    handleQuickTest, setTestingPlatform, setFaviconFailed,
  };
  const cardActions = useMemo<PlatformCardActions>(() => ({
    onPointerDown: (e, index) => actionsRef.current.handlePlatPointerDown(e, index),
    onPointerMove: (e) => actionsRef.current.handlePlatPointerMove(e),
    onPointerUp: () => actionsRef.current.handlePlatPointerUp(),
    onToggleExpanded: (id, next) => actionsRef.current.toggleExpanded(id, next),
    onRefreshQuota: (p) => actionsRef.current.refreshQuota(p),
    onToggleEnabled: (p) => actionsRef.current.handleToggle(p),
    onEdit: (p) => actionsRef.current.handleEdit(p),
    onDelete: (id) => actionsRef.current.handleDelete(id),
    onQuickTest: (p) => actionsRef.current.handleQuickTest(p),
    onCustomTest: (p) => actionsRef.current.setTestingPlatform(p),
    onFaviconFailed: (id) => actionsRef.current.setFaviconFailed(prev => new Set(prev).add(id)),
  }), []);

  // ── Edit / Add form (full page, no list) ──
  if (showForm) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
        {/* Edit page header */}
        <div className="section-header" style={{ gap: 10 }}>
          <button className="btn btn-ghost" style={{ padding: "4px 8px", fontSize: 14 }} onClick={resetForm}>
            ← {t("action.back", "Back")}
          </button>
          <div style={{ flex: 1 }}>
            <div className="section-title">
              {editing ? editing.name : t("platform.add")}
            </div>
            {editing && (
              <div className="section-desc">{editing.platform_type.toUpperCase()} · {getPrimaryBaseUrl(editing.platform_type, editing.endpoints ?? []) || editing.base_url}</div>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleSave}
              disabled={!name || (isPassthrough ? endpoints.length === 0 : (!isMock && (endpoints.length === 0 || !apiKey)))}>
              {editing ? t("action.save") : t("action.create")}
            </button>
          </div>
        </div>

        <div className="animate-fade-in" style={{
          display: "flex",
          flexDirection: "column",
          gap: 16,
        }}>
          {/* 基础信息：名称 + 协议 */}
          <FormSection title={t("platform.sectionBasic", "基础信息")}>
            <input className="input" placeholder={t("platform.name")} value={name}
              onChange={(e) => setName(e.target.value)} />
          {editing ? (
            <div style={{
              display: "flex", alignItems: "center", gap: 8,
              padding: "10px 14px", borderRadius: "var(--radius-sm)",
              background: "var(--bg-glass)", border: "1px solid var(--border)",
              fontSize: 14,
            }}>
              <span style={{
                display: "inline-block", padding: "2px 8px", borderRadius: "var(--radius-sm)",
                background: `${PROTOCOL_COLORS[protocol] || "var(--accent)"}20`,
                color: PROTOCOL_COLORS[protocol] || "var(--accent)",
                fontSize: 11, fontWeight: 700,
              }}>
                {protocol.toUpperCase()}
              </span>
              <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>
                {t("platform.protocolLocked", "Protocol cannot be changed after creation")}
              </span>
            </div>
          ) : (
            <SearchableProtocolSelect
              value={protocol}
              codingPlan={codingPlan}
              onChange={handleProtocolChange}
            />
          )}
          </FormSection>

          {/* Mock 平台配置编辑器（仅 mock 平台显示，替代 endpoints / API Key / 模型） */}
          {isMock && (
            <FormSection title={t("platform.sectionSpecial", "特例配置")}>
              <MockConfigEditor config={mockConfig} onChange={setMockConfig} />
            </FormSection>
          )}

          {/* New API 余额查询配置（仅 newapi 平台显示） */}
          {protocol === "newapi" && (
            <FormSection
              title={t("platform.newapiBalanceConfig", "余额查询配置")}
              desc={t("platform.newapiBalanceHint", "查询余额需要独立的地址和 Token（从控制台获取），与 API Key 不同")}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t("platform.newapiBalanceUrl", "余额查询地址")}
                </div>
                <input
                  className="input"
                  placeholder={t("platform.newapiBalanceUrlPlaceholder", "https://your-newapi-instance.com")}
                  value={newApiConfig.balance_base_url}
                  onChange={(e) => setNewApiConfig(prev => ({ ...prev, balance_base_url: e.target.value }))}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t("platform.newapiBalanceKey", "余额查询 Token")}
                </div>
                <input
                  className="input"
                  type="text"
                  placeholder={t("platform.newapiBalanceKeyPlaceholder", "sess-xxxx 或 access token")}
                  value={newApiConfig.balance_api_key}
                  onChange={(e) => setNewApiConfig(prev => ({ ...prev, balance_api_key: e.target.value }))}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t("platform.newapiUserId", "用户 ID")}
                </div>
                <input
                  className="input"
                  placeholder={t("platform.newapiUserIdPlaceholder", "数字 ID（可选）")}
                  value={newApiConfig.user_id}
                  onChange={(e) => setNewApiConfig(prev => ({ ...prev, user_id: e.target.value }))}
                />
              </div>
            </FormSection>
          )}

          {/* Claude Code 订阅（透传）配置：仅 base_url（host 根）+ 可空 api_key */}
          {isPassthrough && (
            <FormSection title={t("platform.sectionPassthrough", "透传配置")}>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
                  {t("platform.passthroughBaseUrl", "上游地址（Base URL）")}
                </div>
                <input
                  className="input"
                  placeholder="https://api.anthropic.com"
                  value={endpoints[0]?.base_url ?? ""}
                  onChange={(e) => {
                    const next = [...endpoints];
                    if (next.length === 0) {
                      next.push({ protocol: "anthropic" as Protocol, base_url: e.target.value, client_type: "default" });
                    } else {
                      next[0] = { ...next[0], base_url: e.target.value };
                    }
                    setEndpoints(next);
                  }}
                />
                <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                  {t("platform.passthroughBaseUrlHint", "填 host 根（如 https://api.anthropic.com）。纯透传会拼接客户端原始 path/query 直接转发，请勿带版本前缀。")}
                </div>
              </div>
              {/* 可空 API Key（透传模式客户端自带认证，留空即可） */}
              <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <input
                  className="input"
                  type={showKey ? "text" : "password"}
                  placeholder={t("platform.apiKeyOptional", "API Key（可选，透传可留空）")}
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  style={{ flex: 1 }}
                />
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  title={showKey ? "Hide key" : "Show key"}
                  onClick={() => setShowKey(!showKey)}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    {showKey ? (
                      <>
                        <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                        <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                        <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
                        <line x1="1" y1="1" x2="23" y2="23" />
                      </>
                    ) : (
                      <>
                        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                        <circle cx="12" cy="12" r="3" />
                      </>
                    )}
                  </svg>
                </button>
              </div>
              <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                {t("platform.passthroughNote", "纯透传：客户端请求的 header（含订阅 OAuth 认证）与 body 原样转发，aidog 不做任何转换或认证注入。上方 API Key 可留空。")}
              </div>
            </FormSection>
          )}

          {/* Protocol Endpoints（mock / 透传平台隐藏，无可编辑上游） */}
          {!isMock && !isPassthrough && (
          <>
          <FormSection
            title={t("platform.endpoints", "Protocol Endpoints")}
            desc={t("platform.endpointsHint", "Additional protocols this platform supports with different base URLs")}
            action={(
              <button
                type="button"
                className="btn btn-ghost"
                style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                onClick={() => setEndpoints([...endpoints, { protocol: "openai" as Protocol, base_url: "", client_type: defaultClientForProtocol("openai"), coding_plan: false }])}
              >
                + {t("platform.addEndpoint", "Add Endpoint")}
              </button>
            )}
          >
            {endpoints.length === 0 && (
              <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "4px 0", fontStyle: "italic" }}>
                {t("platform.noEndpoints", "No additional endpoints")}
              </div>
            )}
            {endpoints.map((ep, idx) => (
              <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <select
                  className="input"
                  style={{ width: 120, flexShrink: 0 }}
                  value={ep.protocol}
                  onChange={(e) => {
                    const newProto = e.target.value as Protocol;
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], protocol: newProto, client_type: defaultClientForProtocol(newProto) };
                    setEndpoints(next);
                  }}
                >
                  {ENDPOINT_PROTOCOLS.map((p) => (
                    <option key={p.value} value={p.value}>{p.label}</option>
                  ))}
                </select>
                <input
                  className="input"
                  style={{ flex: 1 }}
                  placeholder="Endpoint Base URL"
                  value={ep.base_url}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], base_url: e.target.value };
                    setEndpoints(next);
                  }}
                />
                <select
                  className="input"
                  style={{ width: 140, flexShrink: 0 }}
                  value={ep.client_type || "default"}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], client_type: e.target.value as ClientType };
                    setEndpoints(next);
                  }}
                  title={t("platform.clientType", "客户端模拟")}
                >
                  <option value="default">{t(CLIENT_TYPES[0].labelKey!)}</option>
                  {["Claude Code", "Codex", "IDE"].map(group => (
                    <optgroup key={group} label={group}>
                      {CLIENT_TYPES.filter(c => c.group === group).map(c => (
                        <option key={c.value} value={c.value}>{c.label}</option>
                      ))}
                    </optgroup>
                  ))}
                </select>
                {/* Coding Plan 开关 */}
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  style={{
                    flexShrink: 0,
                    width: 28, height: 28, minWidth: 28,
                    padding: 0,
                    fontSize: 11, fontWeight: 700,
                    color: ep.coding_plan ? "var(--color-success, #34c759)" : "var(--text-tertiary)",
                    background: ep.coding_plan ? "var(--color-success, #34c759)15" : "transparent",
                    border: `1px solid ${ep.coding_plan ? "var(--color-success, #34c759)40" : "var(--border)"}`,
                    borderRadius: "var(--radius-sm)",
                  }}
                  title={ep.coding_plan ? "Coding Plan ON" : "Coding Plan"}
                  onClick={() => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], coding_plan: !next[idx].coding_plan };
                    setEndpoints(next);
                  }}
                >
                  C
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon btn-danger"
                  style={{ flexShrink: 0 }}
                  onClick={() => setEndpoints(endpoints.filter((_, i) => i !== idx))}
                >
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                  </svg>
                </button>
              </div>
            ))}
          </FormSection>

          {/* API Key with show/copy */}
          <FormSection title={t("platform.sectionAuth", "认证")}>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <input
              className="input"
              type={showKey ? "text" : "password"}
              placeholder="API Key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              style={{ flex: 1 }}
            />
            <button
              type="button"
              className="btn btn-ghost btn-icon"
              title={showKey ? "Hide key" : "Show key"}
              onClick={() => setShowKey(!showKey)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                {showKey ? (
                  <>
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                    <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                    <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
                    <line x1="1" y1="1" x2="23" y2="23" />
                  </>
                ) : (
                  <>
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                    <circle cx="12" cy="12" r="3" />
                  </>
                )}
              </svg>
            </button>
            {editing && apiKey && (
              <button
                type="button"
                className="btn btn-ghost btn-icon"
                title="Copy key"
                onClick={() => navigator.clipboard.writeText(apiKey)}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
            )}
          </div>
          </FormSection>

          {/* Models Configuration */}
          <FormSection
            title={t("platform.models")}
            action={(
              <div style={{ display: "flex", gap: 6 }}>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--text-secondary)" }}
                  onClick={handleFillAll}
                  disabled={!models.default.trim()}
                  title={t("platform.fillAllHint")}
                >
                  {t("platform.fillAll")}
                </button>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                  onClick={handleFetchModels}
                  disabled={!apiKey || endpoints.length === 0 || fetching}
                >
                  {fetching ? t("status.loading") : t("platform.fetchModels")}
                </button>
              </div>
            )}
          >
            {fetchError && (
              <div style={{ fontSize: 12, color: "var(--danger, #e55)", padding: "2px 0" }}>
                {fetchError}
              </div>
            )}
            {MODEL_SLOTS.map(({ key, labelKey }) => {
              const query = models[key].trim().toLowerCase();
              const filtered = availableModels.length > 0
                ? (query
                  ? availableModels.filter(m => pinyinMatch(query, m))
                  : availableModels)
                : [];
              return (
              <div key={key} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{
                  fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)",
                  width: 56, textAlign: "right", flexShrink: 0,
                }}>
                  {t(labelKey)}
                </span>
                <div style={{ position: "relative", flex: 1 }}>
                  <input
                    className="input"
                    style={{ width: "100%", paddingRight: availableModels.length > 0 ? 28 : undefined }}
                    placeholder={t(labelKey)}
                    value={models[key]}
                    onChange={(e) => {
                      handleModelChange(key, e.target.value);
                      if (availableModels.length > 0) setActiveDropdown(key);
                    }}
                    onFocus={() => {
                      if (availableModels.length > 0) setActiveDropdown(key);
                    }}
                  />
                  {availableModels.length > 0 && (
                    <button
                      type="button"
                      className="btn btn-ghost btn-icon"
                      style={{
                        position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
                        width: 24, height: 24, minWidth: 24, padding: 0,
                        color: "var(--text-tertiary)", cursor: "pointer",
                      }}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        setActiveDropdown(activeDropdown === key ? null : key);
                      }}
                      title={t("platform.selectModel")}
                    >
                      ▾
                    </button>
                  )}
                  {/* 可搜索下拉列表 — 主题化 */}
                  {activeDropdown === key && filtered.length > 0 && (
                    <>
                      <div
                        style={{ position: "fixed", inset: 0, zIndex: 99 }}
                        onMouseDown={() => setActiveDropdown(null)}
                      />
                      <div
                        className="glass-elevated"
                        style={{
                          position: "absolute",
                          top: "100%",
                          left: 0,
                          right: 0,
                          marginTop: 4,
                          maxHeight: 200,
                          overflowY: "auto",
                          zIndex: 100,
                          padding: 4,
                          animation: "fadeIn 150ms ease both",
                        }}
                      >
                        {filtered.map((m) => (
                          <button
                            key={m}
                            type="button"
                            className="btn btn-ghost"
                            style={{
                              width: "100%",
                              justifyContent: "flex-start",
                              padding: "6px 10px",
                              fontSize: 12,
                              fontWeight: models[key] === m ? 600 : 400,
                              color: models[key] === m ? "var(--accent)" : "var(--text-primary)",
                              background: models[key] === m ? "var(--accent-subtle)" : "transparent",
                              borderRadius: "var(--radius-sm)",
                            }}
                            onMouseDown={(e) => {
                              e.preventDefault();
                              handleModelSelect(key, m);
                              setActiveDropdown(null);
                            }}
                          >
                            {m}
                          </button>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </div>
              );
            })}
          </FormSection>
          </>
          )}

          {/* Manual Budgets — 所有平台可设（含 mock / 有上游配额支持的平台），仅透传订阅不需要 */}
          {!isPassthrough && (
            <FormSection
              title={t("platform.manualBudgetTitle", "手动预算")}
              desc={t("platform.manualBudgetDesc", "该平台无上游额度自动查询，可手动设置一个或多个预算限额，按用量预估扣减；任一耗尽时停止转发（返回 402），窗口/次日恢复后自动放行。")}
              action={(
                <button
                  type="button"
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                  onClick={() => setManualBudgets([...manualBudgets, newManualBudget()])}
                >
                  {t("platform.manualBudgetAdd", "添加限额")}
                </button>
              )}
            >
              {manualBudgets.length === 0 && (
                <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "2px 0" }}>
                  {t("platform.manualBudgetEmpty", "暂无限额，点击「添加限额」开始配置。")}
                </div>
              )}
              {manualBudgets.map((b, idx) => {
                const update = (patch: Partial<ManualBudget>) =>
                  setManualBudgets(manualBudgets.map((x, i) => i === idx ? { ...x, ...patch } : x));
                const needsWindow = b.kind === "rolling" || b.kind === "fixed";
                const onKindChange = (kind: ManualBudgetKind) => {
                  const willNeedWindow = kind === "rolling" || kind === "fixed";
                  // 切到 rolling/fixed 且尚无窗口配置 → 给合理默认（7 天）
                  if (willNeedWindow && (b.window_hours == null || b.window_hours <= 0)) {
                    update({ kind, window_hours: 7, window_unit: "day" });
                  } else {
                    update({ kind });
                  }
                };
                return (
                  <div key={b.id} style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
                    <select
                      className="input"
                      style={{ width: 110, flexShrink: 0 }}
                      value={b.kind}
                      onChange={e => onKindChange(e.target.value as ManualBudgetKind)}
                    >
                      <option value="total">{t("platform.manualBudgetKindTotal", "总额")}</option>
                      <option value="rolling">{t("platform.manualBudgetKindRolling", "滑动窗口")}</option>
                      <option value="fixed">{t("platform.manualBudgetKindFixed", "固定窗口")}</option>
                      <option value="daily">{t("platform.manualBudgetKindDaily", "每日")}</option>
                    </select>
                    <select
                      className="input"
                      style={{ width: 90, flexShrink: 0 }}
                      value={b.unit}
                      onChange={e => update({ unit: e.target.value as ManualBudgetUnit })}
                    >
                      <option value="usd">$ USD</option>
                      <option value="token">{t("platform.manualBudgetUnitToken", "Token")}</option>
                    </select>
                    <input
                      className="input"
                      type="number"
                      min={0}
                      step="any"
                      style={{ width: 100, flexShrink: 0 }}
                      placeholder={t("platform.manualBudgetAmount", "额度")}
                      value={b.amount || ""}
                      onChange={e => update({ amount: parseFloat(e.target.value) || 0 })}
                    />
                    {needsWindow && (
                      <>
                        <input
                          className="input"
                          type="number"
                          min={0}
                          step="any"
                          style={{ width: 80, flexShrink: 0 }}
                          placeholder={t("platform.manualBudgetWindow", "窗口")}
                          value={b.window_hours ?? ""}
                          onChange={e => update({ window_hours: e.target.value === "" ? null : (parseFloat(e.target.value) || 0) })}
                        />
                        <select
                          className="input"
                          style={{ width: 90, flexShrink: 0 }}
                          value={b.window_unit ?? "hour"}
                          onChange={e => update({ window_unit: e.target.value as WindowUnit })}
                        >
                          <option value="minute">{t("platform.windowUnitMinute", "分钟")}</option>
                          <option value="hour">{t("platform.windowUnitHour", "小时")}</option>
                          <option value="day">{t("platform.windowUnitDay", "天")}</option>
                          <option value="week">{t("platform.windowUnitWeek", "周")}</option>
                          <option value="month">{t("platform.windowUnitMonth", "月")}</option>
                        </select>
                      </>
                    )}
                    <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                      <input
                        type="checkbox"
                        checked={b.enabled}
                        onChange={e => update({ enabled: e.target.checked })}
                      />
                      {t("platform.manualBudgetEnabled", "启用")}
                    </label>
                    <button
                      type="button"
                      className="btn btn-ghost btn-icon btn-danger"
                      style={{ flexShrink: 0 }}
                      title={t("action.delete", "删除")}
                      onClick={() => setManualBudgets(manualBudgets.filter((_, i) => i !== idx))}
                    >
                      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                      </svg>
                    </button>
                  </div>
                );
              })}
            </FormSection>
          )}

          {/* Claude Code Config */}
          {editing && (
            <FormSection title={t("settings.claudeCodeConfig")}>
              <button
                type="button"
                className="btn btn-ghost"
                style={{
                  width: "100%",
                  justifyContent: "space-between",
                  fontSize: 12,
                  padding: "6px 4px",
                  color: "var(--text-secondary)",
                }}
                onClick={() => setShowClaudeConfig(!showClaudeConfig)}
              >
                <span style={{ fontWeight: 600 }}>{t("settings.claudeConfigToggle", "Config Override")}</span>
                <span style={{ opacity: 0.5 }}>{showClaudeConfig ? "▾" : "▸"}</span>
              </button>
              {showClaudeConfig && (
                <div className="animate-fade-in" style={{ marginTop: 6 }}>
                  <textarea
                    className="input"
                    style={{
                      fontFamily: '"SF Mono", "Fira Code", monospace',
                      fontSize: 12,
                      lineHeight: 1.6,
                      minHeight: 180,
                      resize: "vertical",
                      whiteSpace: "pre",
                    }}
                    value={claudeConfigJson}
                    onChange={(e) => setClaudeConfigJson(e.target.value)}
                    spellCheck={false}
                  />
                  <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4, lineHeight: 1.5 }}>
                    {t("settings.platformConfigHint")}
                  </div>
                  {(() => {
                    try {
                      const merged = JSON.parse(claudeConfigJson);
                      const overridden = Object.keys(merged).filter(
                        k => JSON.stringify(merged[k]) !== JSON.stringify(globalClaudeConfig[k]),
                      );
                      return overridden.length > 0 ? (
                        <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                          {overridden.map(k => (
                            <span key={k} className="badge badge-accent" style={{ fontSize: 10 }}>
                              {k}
                            </span>
                          ))}
                        </div>
                      ) : (
                        <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>
                          {t("settings.allAligned")}
                        </div>
                      );
                    } catch { return null; }
                  })()}
                </div>
              )}
            </FormSection>
          )}

          {saveError && (
            <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
              {saveError}
            </div>
          )}
        </div>
      </div>
    );
  }

  // ── List view ──
  return (
    <>
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.platforms")}</div>
          <div className="section-desc">
            {platforms.length > 0 ? `${platforms.filter(p => p.enabled).length} / ${platforms.length} active` : t("platform.empty")}
          </div>
        </div>
        <button className="btn btn-primary" onClick={() => { resetForm(); setShowForm(true); }}>
          + {t("platform.add")}
        </button>
      </div>

      {/* Platform List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div ref={platListRef} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {platforms.length === 0 && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("platform.empty")}</div>
            </div>
          )}
          {platforms.map((p, i) => {
            const isDragging = platDrag?.from === i;
            const draggedPlat = platDrag ? platforms[platDrag.from] : null;
            const draggedColor = draggedPlat ? (PROTOCOL_COLORS[draggedPlat.platform_type] || "var(--accent)") : "";
            return (
              <React.Fragment key={p.id}>
                {/* Ghost card at insertion point */}
                {platDrag && platDrag.to === i && draggedPlat && (
                  <div style={{
                    display: "flex", alignItems: "center", gap: 14, paddingLeft: 44,
                    padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                    background: "var(--glass-bg, rgba(255,255,255,0.06))",
                    border: "1.5px dashed var(--accent)",
                    opacity: 0.5, filter: "grayscale(0.8)",
                    pointerEvents: "none", transition: "all 150ms ease",
                  }}>
                    <div style={{ width: 10, height: 10, borderRadius: "50%", background: draggedColor, flexShrink: 0 }} />
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{draggedPlat.name}</span>
                    <span className="badge badge-muted" style={{ fontSize: 10 }}>{PROTOCOL_LABELS[draggedPlat.platform_type] || draggedPlat.platform_type}</span>
                  </div>
                )}
                <PlatformCard
                  platform={p}
                  index={i}
                  isDragging={isDragging}
                  dragActive={!!platDrag}
                  quota={quotaMap[p.id]}
                  preferReal={!!quotaRealIds[p.id]}
                  refreshing={!!quotaRefreshing[p.id]}
                  usage={usageMap[p.id]}
                  expanded={expandedIds.has(p.id)}
                  manualResult={testResults[p.id]}
                  testing={testingId === p.id}
                  faviconFailed={faviconFailed.has(p.id)}
                  actions={cardActions}
                />
              </React.Fragment>
            );
          })}
          {platDrag && (() => {
            if (platDrag.to !== platforms.length) return null;
            const dp = platforms[platDrag.from];
            const dc = PROTOCOL_COLORS[dp.platform_type] || "var(--accent)";
            return (
              <div style={{
                display: "flex", alignItems: "center", gap: 14, paddingLeft: 44,
                padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                background: "var(--glass-bg, rgba(255,255,255,0.06))",
                border: "1.5px dashed var(--accent)",
                opacity: 0.5, filter: "grayscale(0.8)",
                pointerEvents: "none", transition: "all 150ms ease",
              }}>
                <div style={{ width: 10, height: 10, borderRadius: "50%", background: dc, flexShrink: 0 }} />
                <span style={{ fontSize: 13, fontWeight: 600 }}>{dp.name}</span>
                <span className="badge badge-muted" style={{ fontSize: 10 }}>{PROTOCOL_LABELS[dp.platform_type] || dp.platform_type}</span>
              </div>
            );
          })()}
        </div>
      )}
    </div>

      {/* Custom test overlay */}
      {testingPlatform !== null && (
        <div style={{
          position: "fixed", inset: 0, zIndex: 1000,
          background: "rgba(0,0,0,0.4)", backdropFilter: "blur(4px)",
          display: "flex", alignItems: "center", justifyContent: "center",
        }}>
          <ModelTestPanel
            platform={testingPlatform as Platform}
            onClose={() => setTestingPlatform(null)}
            onResult={(success) => { if (testingPlatform) setTestResults(prev => ({ ...prev, [testingPlatform.id]: success ? "ok" : "fail" })); }}
          />
        </div>
      )}

      {/* Test result toast */}
      {toast && (
        <div style={{
          position: "fixed", top: 24, left: "50%", transform: "translateX(-50%)",
          zIndex: 2000, pointerEvents: "none",
          padding: "10px 20px", borderRadius: 10,
          background: toast.ok ? "var(--color-success, #22c55e)" : "var(--color-danger, #ef4444)",
          color: "#fff", fontSize: 13, fontWeight: 600,
          boxShadow: "0 4px 20px rgba(0,0,0,0.25)",
          opacity: 0.95,
          transition: "opacity 0.3s",
        }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>{toast.ok ? <IconCheck size={14} color="#fff" /> : <IconClose size={14} color="#fff" />} {toast.text}</span>
        </div>
      )}
    </>
  );
}

