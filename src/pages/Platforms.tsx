import React, { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { platformApi, settingsApi, modelTestApi, quotaApi, schedulingApi, groupDetailApi, parseMockConfig, serializeMockConfig, parseNewApiConfig, serializeNewApiConfig, parsePlatformBreaker, serializePlatformBreaker, onProxyLogUpdated, DEFAULT_MOCK_CONFIG, DEFAULT_NEWAPI_CONFIG, type Platform, type PlatformStatus, type Protocol, type ModelSlot, type PlatformEndpoint, type ClientType, type PlatformUsageStats, type PlatformQuota, type LastTestResult, type MockConfig, type MockErrorMode, type NewApiConfig, type ManualBudget, type ManualBudgetKind, type ManualBudgetUnit, type WindowUnit, type SchedulingBreakerSettings, type GroupDetail } from "../services/api";
import { IconClose, IconCheck } from "../components/icons";
import { cycleMsForTier, codingTierLevel, type ColorLevel } from "../components/shared";

import { ModelTestPanel } from "./ModelTestPanel";
import { GroupsEmbedded } from "./Groups";
import { MiddlewareRulesPanel } from "../components/settings/MiddlewareRules";
import { pinyinMatch } from "../utils/pinyin";
import { SmartPasteModal, type SmartPasteApplyResult } from "../components/platforms/SmartPasteModal";
import { PlatformCard, type PlatformCardActions } from "../components/platforms/PlatformCard";

/** 支持的协议选项（含 coding plan 变体） */
export type ProtocolOption = { value: Protocol; label: string; codingPlan?: boolean; keywords?: string[]; hosts?: string[] };

export const PROTOCOLS: ProtocolOption[] = [
  // ── 官方 ──
  { value: "anthropic", label: "Anthropic（Claude）", keywords: ["claude", "克劳德", "官方"] },
  { value: "openai", label: "OpenAI", keywords: ["gpt", "chatgpt", "官方"] },
  { value: "codex", label: "Codex", keywords: ["codex"] },
  { value: "gemini", label: "Gemini（Google）", keywords: ["google", "谷歌", "gemini"] },
  // ── 国内官方 ──
  { value: "glm", label: "GLM（智谱）", keywords: ["zhipu", "智谱", "bigmodel", "codegeex"] },
  { value: "glm", label: "GLM Coding Plan", codingPlan: true, keywords: ["智谱编程", "codegeex", "glm code"] },
  { value: "glm_en", label: "GLM 国际版（z.ai）", keywords: ["z.ai", "zhipu en", "智谱国际"] },
  { value: "kimi", label: "Kimi（月之暗面）", keywords: ["moonshot", "月之暗面"] },
  { value: "kimi", label: "Kimi Code Plan", codingPlan: true, keywords: ["kimi编程", "kimi code", "kimi coding"] },
  { value: "minimax", label: "MiniMax（海螺）", keywords: ["海螺", "minimax"] },
  { value: "minimax", label: "MiniMax Coding Plan", codingPlan: true, keywords: ["海螺编程", "minimax coding", "minimax code"] },
  { value: "minimax_en", label: "MiniMax 国际版", keywords: ["minimax io", "minimax en"] },
  { value: "minimax_en", label: "MiniMax Coding Plan 国际版", codingPlan: true, keywords: ["minimax coding intl", "海螺编程国际"] },
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
  { value: "xiaomi_mimo", label: "小米 MiMo Coding Plan", codingPlan: true, keywords: ["xiaomi coding", "小米编程", "mimo token plan", "token plan"] },
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
  { value: "opencode_zen", label: "OpenCode Zen (Free)", keywords: ["opencode", "zen", "opencode.ai", "free"] },
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
export type HealthStatus = "healthy" | "warning" | "error" | "unknown";
export const HEALTH_COLORS: Record<HealthStatus, string> = {
  healthy: "var(--color-success, var(--color-success))",
  warning: "var(--color-warning, #ff9500)",
  error: "var(--color-danger, #ff3b30)",
  unknown: "var(--text-tertiary, #8e8e93)",
};

/** 判断平台健康状态：「成功即绿」语义 —— 最近 N 次请求中只要有一次成功即判健康，
 * 全失败才红，无请求灰。不返回 warning 中间态（避免「能用却显黄」），warning
 * 仅作类型成员保留供其它语义复用。 */
export function healthStatus(recentTotal: number, recentFailures: number): HealthStatus {
  if (recentTotal === 0) return "unknown";
  if (recentFailures >= recentTotal) return "error";        // 全部失败
  return "healthy";                                          // 有任一成功即绿
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
    // 百炼（通义千问）；qwen3-max 已被 qwen3.7-max(2026-05-20) 取代
    bailian: { default: "qwen3.7-max" },
    // deepseek-chat 将 2026-07-24 弃用，v4-flash 为后继
    deepseek: { default: "deepseek-v4-flash" },
    // 小米 MiMo 旗舰文本模型（按量 openai 端点）
    xiaomi_mimo: { default: "mimo-v2.5-pro" },
    // OpenCode Zen 免费旗舰（catalog 定价 0）；其余免费模型靠 fetchModels /v1/models 拉取
    opencode_zen: { default: "big-pickle" },
  };
  return { ...(presets[protocol] || {}) };
}

/** 平台内置候选模型列表（供模型槽位下拉冷启动兜底）。
 *  未刷新（fetchModels）时槽位下拉展示此列表；刷新成功后改用接口 available_models。
 *  数据来自 .trellis/tasks/06-17-platform-model-list/research/*.md（一方/聚合/第三方三组核查）。
 *  约定：列表有序，旗舰/默认在前，**首项 = getDefaultModels 默认值**（保 route resolve 行为）。
 *  查不到官方列表的平台返回 []（完全靠 fetchModels 兜底，不编造）。
 *  模型名月级腐化，运行时必靠 fetchModels 拉取真实可用集。 */
function getDefaultModelList(protocol: Protocol, codingPlan?: boolean): string[] {
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
    doubao: ["doubao-seed-2-0-pro", "doubao-seed-2-0-code-preview", "doubao-seed-2-0-lite", "doubao-seed-2-0-mini"], // 短横线非点号
    doubao_seed: ["doubao-seed-2-0-pro", "doubao-seed-2-0-code-preview", "doubao-seed-2-0-lite", "doubao-seed-2-0-mini"],
    byteplus: ["doubao-seed-2-0-pro", "doubao-seed-2-0-code-preview", "doubao-seed-2-0-lite", "doubao-seed-2-0-mini"],
    // qianfan: 百度文档 JS-rendered 未拿到确切 chat id，留空靠 fetchModels
    xiaomi_mimo: ["mimo-v2.5-pro", "mimo-v2-pro", "mimo-v2.5", "mimo-v2-omni", "mimo-v2-flash"],
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

export const PROTOCOL_LABELS: Record<Protocol, string> = {
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
  opencode_zen: "OpenCode Zen (Free)",
  // ── 订阅透传 ──
  claude_code: "Claude Code 订阅",
  // ── 中转平台 ──
  newapi: "New API",
  // ── 测试 ──
  mock: "Mock",
};

const DEFAULT_NAMES = new Set(Object.values(PROTOCOL_LABELS));

// ③ 延迟档 quota 外部 HTTP 有界并发上限（仿 Groups.tsx BATCH_TEST_CONCURRENCY=3）。
const QUOTA_CONCURRENCY = 3;

export const PROTOCOL_COLORS: Record<string, string> = {
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
  opencode_zen: "#6E56CF",
  // ── 订阅透传 ──
  claude_code: "#D97757",
  // ── 中转平台 ──
  newapi: "#10A37F",
  // ── 测试 ──
  mock: "#8E8E93",
};

export const MODEL_SLOTS: { key: ModelSlot; labelKey: string }[] = [
  { key: "default", labelKey: "platform.modelDefault" },
  { key: "sonnet", labelKey: "platform.modelSonnet" },
  { key: "opus", labelKey: "platform.modelOpus" },
  { key: "haiku", labelKey: "platform.modelHaiku" },
  { key: "gpt", labelKey: "platform.modelGpt" },
];

/** 从 PlatformModels 中提取所有非空值（去重） */
export function allModelValues(models: Platform["models"]): string[] {
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
                      background: "var(--color-success, var(--color-success))20",
                      color: "var(--color-success, var(--color-success))",
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
export interface QuotaDisplay {
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

export function computeQuotaDisplay(p: Platform, q: PlatformQuota | undefined, preferRealCalibrated: boolean): QuotaDisplay {
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
export function tierLabel(name: string): string {
  if (name === "five_hour") return "5h";
  if (name === "weekly_limit") return "week";
  if (name === "mcp_monthly") return "MCP";
  return name;
}

/** ISO 8601 或 millis → 剩余时间人类可读字符串 */
export function formatResetCountdown(resetsAt: string | null): string {
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
export interface ManualBudgetDisplay {
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
export function computeManualBudgetDisplay(budgets: ManualBudget[] | undefined): ManualBudgetDisplay | null {
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

export function Platforms({ onNavigate, initialFilter }: { onNavigate?: (id: string, context?: { platformId?: number; platformName?: string }) => void; initialFilter?: { platformId?: number; platformName?: string } }) {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  // 渐进加载计数（来自 GroupsEmbedded 逐组流式回传 {total, active}），驱动页头「N / M active」增量更新。
  // null = 尚未回传 → 回退本页自身 platforms 派生值（如本页列表先于分组流加载完）。
  const [progressiveCount, setProgressiveCount] = useState<{ total: number; active: number } | null>(null);
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
        // 仅在未分组平台子集内重排（platDrag from/to 均为 standalone 索引）。
        const reordered = [...standalonePlatforms];
        const [moved] = reordered.splice(platDrag.from, 1);
        reordered.splice(effectiveTo, 0, moved);
        // 重建 platforms：已分组平台原位，未分组按新序填回（保 sort_order 全局一致）。
        let si = 0;
        setPlatforms(platforms.map(p => platformMembership.has(p.id) ? p : reordered[si++]));
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
  // ④ 延迟档 quota 待回标志：load 时对所有需查 quota 的平台置 true，HTTP 结算（成功/失败）后置 false。
  //    余额区据此显骨架而非 est 旧值，避免闪烁回填。
  const [quotaPending, setQuotaPending] = useState<Record<number, boolean>>({});
  // ④ 渐进档 usage 批量待回标志：load 时 true，批量 usageStatsAll 到达后 false → 用量区先骨架后数据。
  const [usageLoading, setUsageLoading] = useState(false);
  const [testResults, setTestResults] = useState<Record<number, "ok" | "fail">>({});
  // 平台「最近一次测试结果」徽章数据（proxy_log source_protocol='test' 最新一条），随 load() 拉取 + 监听 aidog-platform-test-completed 单卡刷新
  const [lastTestMap, setLastTestMap] = useState<Record<number, LastTestResult>>({});
  // ③⑤ quota 调度：待领取队列（按可视优先顺序入队）、已调度去重集合、需查 quota 的平台快照。
  //    IntersectionObserver 决定入队时机/优先级，有界 worker pool 控并发上限。用 ref 不触发渲染。
  const quotaQueueRef = useRef<Platform[]>([]);
  // 局部刷新守卫：每次本地乐观写操作（保存/删除/清理）自增 epoch；在途的 load()/refreshStats
  //   captureEpoch 后异步返回时若 epoch 已变，跳过 setPlatforms(list) 整列表覆盖，防慢后端晚到回弹
  //   （mount-fetch-late-resolve-overwrites-optimistic 坑）。
  const platformsEpochRef = useRef(0);
  const quotaScheduledRef = useRef<Set<number>>(new Set());
  const quotaPoolActiveRef = useRef(0);
  const quotaWantMapRef = useRef<Map<number, Platform>>(new Map());
  const platformIObserverRef = useRef<IntersectionObserver | null>(null);
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
  // GroupsEmbedded「添加分组」弹窗触发 ref（按钮上移到本页页头）。
  const openCreateGroupRef = useRef<(() => void) | null>(null);
  // GroupsEmbedded 跨组件刷新入口（全局 purge 删平台后，触发分组卡内重建）。
  const groupsReloadRef = useRef<(() => void) | null>(null);
  const [showPaste, setShowPaste] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");
  const [saveError, setSaveError] = useState("");
const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);
  const [toast, setToast] = useState<{ text: string; ok: boolean } | null>(null);
  // GroupsEmbedded 进入全屏视图态（创建/编辑分组）时为 true：隐藏下方分隔线 + 未分组平台列表，避免与全屏视图并列。
  const [groupFullscreen, setGroupFullscreen] = useState(false);
  // pointer 拖拽（未分组平台 → 分组）；HTML5 DnD 跨区域在 WKWebView 失效，改 pointer events
  const [groupDrag, setGroupDrag] = useState<{ pid: number; pname: string; x: number; y: number } | null>(null);
  const groupHighlightEl = useRef<HTMLElement | null>(null);
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
  // 熔断阈值覆盖（0/空 = 继承全局默认；编辑表单态）。空字符串表示继承。
  const [breakerFailureThreshold, setBreakerFailureThreshold] = useState<string>("");
  const [breakerOpenSecs, setBreakerOpenSecs] = useState<string>("");
  const [breakerHalfOpenMax, setBreakerHalfOpenMax] = useState<string>("");
  // 全局调度+熔断默认（用于展示「继承默认 N」），只读消费。
  const [breakerDefaults, setBreakerDefaults] = useState<SchedulingBreakerSettings | null>(null);
  // 分组归属选项：auto_group（是否建默认分组，默认勾）+ join_group_ids（加入的已有分组）。
  // groupDetails 供 multi-select 渲染 + 编辑态反查平台当前手动组成员 + 平台归属映射构建。
  const [autoGroup, setAutoGroup] = useState(true);
  const [joinGroupIds, setJoinGroupIds] = useState<number[]>([]);
  const [groupDetails, setGroupDetails] = useState<GroupDetail[]>([]);
  // 锁定分组：从某分组 ➕ 触发创建平台时，预绑该分组且禁止修改归属。
  const [lockedGroupId, setLockedGroupId] = useState<number | null>(null);
  // 平台归属映射：platformId → groupNames[]（用于平台卡片显示所属分组 badge）
  const [platformMembership, setPlatformMembership] = useState<Map<number, string[]>>(new Map());
  // 未归属任何分组的平台（主列表独立展示）；已分组平台只在 GroupsEmbedded 内展示，避免重复。
  const standalonePlatforms = useMemo(
    () => platforms.filter(p => !platformMembership.has(p.id)),
    [platforms, platformMembership],
  );

  const isMock = protocol === "mock";
  // Claude Code 订阅纯透传：客户端自带订阅 OAuth 认证，aidog 原样转发。
  // 仅需 base_url（host 根），api_key 可空，隐藏 endpoints/models 编辑。
  const isPassthrough = protocol === "claude_code";
  // OpenCode Zen：免费匿名访问（api_key 留空时 proxy 兜底 $opencode），全程不校验 key 存在。
  const keyOptional = protocol === "opencode_zen";
  // 需要 api_key 但未填（keyOptional 平台不要求）—— fetch/列模型按钮共用的禁用判定。
  const apiKeyMissing = !keyOptional && !apiKey;

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
    // Auto-fill 默认模型预设（与 endpoints 同步随协议切换）。
    // 仅填预设有值的槽位，其余保持空；未覆盖平台返回空对象 = 不改动。
    const defaultModels = getDefaultModels(newProtocol, cp);
    setModels({
      default: "", sonnet: "", opus: "", haiku: "", gpt: "",
      ...defaultModels,
    });
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

  /** 智能识别弹窗确认后，将解析结果填入添加表单。 */
  const applyPaste = (r: SmartPasteApplyResult) => {
    // 匹配到内置平台 → 走协议切换（设置 name + 默认 endpoints + client_type）。
    // 未匹配 → 不改平台选择（保持当前 protocol/endpoints），仅填 base_url/apiKey。
    // codingPlan flag 必传：同 value 的普通/coding 两 preset（如 xiaomi_mimo）命中后，
    // 不传 flag 则 getDefaultEndpoints 拿普通 endpoints（base_url 取错）。
    if (r.platform) {
      handleProtocolChange(r.platform.value as Protocol, r.platform.codingPlan);
    }
    if (r.baseUrls.length > 0) {
      // 多类型 base_url 多选：每个选中 url（按协议去重，每协议最多一个）→ 一个 endpoint。
      // 同协议 endpoint 存在则覆盖 base_url，否则新增；支持 anthropic + openai 双端点平台（如 glm）。
      setEndpoints((prev) => {
        const eps = prev.map((e) => ({ ...e }));
        for (const b of r.baseUrls) {
          const epProto: Protocol = b.protocol === "unknown" ? "openai" : b.protocol;
          let idx = eps.findIndex((e) => e.protocol === epProto);
          if (idx >= 0) {
            eps[idx] = { ...eps[idx], base_url: b.url };
          } else {
            eps.push({ protocol: epProto, base_url: b.url, client_type: defaultClientForProtocol(epProto) });
          }
        }
        return eps;
      });
    }
    if (r.apiKey) setApiKey(r.apiKey);
    setShowPaste(false);
    // 弹窗可能从主列表「添加平台」直达（表单尚未挂载），apply 后显式拉起表单展示已填字段。
    setShowForm(true);
  };

  /** 纯函数：从 groupDetails 构建 platformId → groupNames[] */
  function buildMembership(gds: GroupDetail[]): Map<number, string[]> {
    const m = new Map<number, string[]>();
    for (const g of gds) {
      for (const gp of g.platforms) {
        const arr = m.get(gp.platform.id) ?? [];
        arr.push(g.group.name);
        m.set(gp.platform.id, arr);
      }
    }
    return m;
  }

  /** 分组变更：refetch groupDetails，effect 自动重建 membership */
  const handleGroupsChanged = async () => {
    try {
      setGroupDetails(await groupDetailApi.list());
    } catch { /* ignore */ }
  };

  // ── pointer 拖拽未分组平台到分组（绕开 WKWebView HTML5 跨区域 DnD 失效）──
  const clearGroupHighlight = () => {
    if (groupHighlightEl.current) {
      groupHighlightEl.current.style.outline = "";
      groupHighlightEl.current.style.outlineOffset = "";
      groupHighlightEl.current = null;
    }
  };
  const findGroupAt = (x: number, y: number): { el: HTMLElement; gid: number } | null => {
    const el = document.elementFromPoint(x, y) as HTMLElement | null;
    const groupEl = el?.closest("[data-group-id]") as HTMLElement | null;
    if (!groupEl) return null;
    const gid = Number(groupEl.getAttribute("data-group-id"));
    return Number.isFinite(gid) && gid > 0 ? { el: groupEl, gid } : null;
  };
  const onStandaloneGroupPointerDown = (e: React.PointerEvent, p: Platform) => {
    if (e.button !== 0) return;
    const tgt = e.target as HTMLElement;
    // 让位：reorder handle（pointer 排序）+ 交互元素（按钮/输入）
    if (tgt.closest(".drag-handle-inline, button, a, input, [role=button]")) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setGroupDrag({ pid: p.id, pname: p.name, x: e.clientX, y: e.clientY });
  };
  const onStandaloneGroupPointerMove = (e: React.PointerEvent) => {
    setGroupDrag(d => d ? { ...d, x: e.clientX, y: e.clientY } : d);
    if (!groupDrag) return;
    clearGroupHighlight();
    const found = findGroupAt(e.clientX, e.clientY);
    if (found) {
      found.el.style.outline = "2px solid var(--accent)";
      found.el.style.outlineOffset = "2px";
      groupHighlightEl.current = found.el;
    }
  };
  const onStandaloneGroupPointerUp = (e: React.PointerEvent) => {
    if (!groupDrag) return;
    const pid = groupDrag.pid;
    const found = findGroupAt(e.clientX, e.clientY);
    clearGroupHighlight();
    setGroupDrag(null);
    if (!found) return;
    groupDetailApi.movePlatform(pid, 0, found.gid)
      .then(() => {
        setToast({ text: "已加入分组", ok: true });
        // 拖到分组只改归属：平台行本身不变，仅刷 groupDetails 重建 membership（卡片即移到目标组），
        // 无需整页 load()。保留事件广播供 GroupsEmbedded 等跨组件同步。
        handleGroupsChanged();
        window.dispatchEvent(new Event("aidog-groups-changed"));
      })
      .catch(err => setToast({ text: `加入分组失败: ${err}`, ok: false }));
  };

  /** 该平台是否需要外部 quota 查询（mock/claude_code 无配额；无 key / 无 base_url 不可查）。 */
  const platformWantsQuota = useCallback((p: Platform): boolean => {
    if (p.platform_type === "mock" || p.platform_type === "claude_code") return false;
    if (!p.api_key) return false;
    return !!getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
  }, []);

  /** 单平台 quota 查询（成功填 quotaMap），结束后清 pending。供有界并发池 worker 调用。 */
  const fetchQuotaForPlatform = useCallback(async (p: Platform) => {
    const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
    try {
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) setQuotaMap(prev => ({ ...prev, [p.id]: q }));
    } catch { /* ignore */ }
    finally {
      setQuotaPending(prev => { const n = { ...prev }; delete n[p.id]; return n; });
    }
  }, []);

  // ③ 有界并发池：共享队列 quotaQueueRef + 至多 QUOTA_CONCURRENCY 个 worker 循环领取。
  //    入队由 ⑤ IntersectionObserver（可视/未折叠优先）+ 兜底全量补齐触发；scheduled 去重防重复拉。
  const pumpQuotaPool = useCallback(() => {
    const spawn = async () => {
      quotaPoolActiveRef.current++;
      try {
        for (;;) {
          const p = quotaQueueRef.current.shift();
          if (!p) break;
          await fetchQuotaForPlatform(p);
        }
      } finally {
        quotaPoolActiveRef.current--;
      }
    };
    while (quotaPoolActiveRef.current < QUOTA_CONCURRENCY && quotaQueueRef.current.length > 0) {
      void spawn();
    }
  }, [fetchQuotaForPlatform]);

  /** 把平台入 quota 队列（去重），并尝试启动 worker 领取。 */
  const enqueueQuota = useCallback((p: Platform) => {
    if (quotaScheduledRef.current.has(p.id)) return;
    if (!quotaWantMapRef.current.has(p.id)) return; // 非本轮需查平台（已结算/不需查）忽略
    quotaScheduledRef.current.add(p.id);
    quotaQueueRef.current.push(p);
    pumpQuotaPool();
  }, [pumpQuotaPool]);

  /** 局部刷新（新建/编辑平台）专用 quota 调度：不走 load() 重置 wantMap 路径，
   *  故新平台不在 quotaWantMapRef，无法经 enqueueQuota 入队。这里把单平台注入 wantMap + pending
   *  后入队，确保不走整页 load 的平台余额仍会被查（风险④：load 重置 quota 状态耦合）。 */
  const scheduleQuotaFor = useCallback((p: Platform) => {
    if (!platformWantsQuota(p)) return;
    quotaWantMapRef.current.set(p.id, p);
    setQuotaPending(prev => ({ ...prev, [p.id]: true }));
    // 已调度过则先放行重查（编辑可能改了 key/base_url）。
    quotaScheduledRef.current.delete(p.id);
    enqueueQuota(p);
  }, [platformWantsQuota, enqueueQuota]);

  const load = async () => {
    setLoading(true);
    const epoch = platformsEpochRef.current;
    let list: Platform[] = [];
    try {
      list = (await platformApi.list()) || [];
    } catch (e) { console.error(e); }
    // 在途期间发生本地乐观写（删除/保存/清理）则放弃整列表覆盖，避免晚到 resolve 回弹。
    if (epoch !== platformsEpochRef.current) { setLoading(false); return; }

    // ③⑤ quota 调度状态必须在 setPlatforms（→ DOM 提交 → IntersectionObserver 初次回调）之前同步就绪，
    //     否则 observer 初次 fire 时 quotaWantMapRef 仍为空 → enqueueQuota 早退 → 首屏卡片 quota 永不查
    //     （cards 已 intersecting，无后续 isIntersecting 跳变可再触发）。这是「余额/coding plan 全不展示」根因。
    quotaQueueRef.current = [];
    quotaScheduledRef.current = new Set();
    const wantMap = new Map<number, Platform>();
    const pending: Record<number, boolean> = {};
    for (const p of list) {
      if (platformWantsQuota(p)) { wantMap.set(p.id, p); pending[p.id] = true; }
    }
    quotaWantMapRef.current = wantMap;
    setQuotaPending(pending);

    setPlatforms(list);
    // 平台列表到手即渲染，余额/用量改后台渐进填充，禁止外部 quota HTTP 阻塞整页
    setLoading(false);

    // ① 渐进档：usage stats 单次批量（GROUP BY platform_id，含 platform_id=0 回溯），替换逐平台 N+1。
    setUsageLoading(true);
    try {
      const all = await platformApi.usageStatsAll();
      setUsageMap(all || {});
    } catch { /* ignore */ }
    finally {
      setUsageLoading(false);
    }

    // 平台「最近一次测试」徽章数据：并行拉取每平台最新 test 日志，有值才填（null 不填 = 不渲染徽章）
    Promise.all(list.map(p => platformApi.lastTestResult(p.id).catch(() => null)))
      .then(results => {
        const map: Record<number, LastTestResult> = {};
        results.forEach((r, i) => {
          if (r && list[i]) map[list[i].id] = r;
        });
        setLastTestMap(map);
      })
      .catch(() => { /* ignore */ });
  };

  /** 轻量刷新：按 id 局部 merge 派生统计字段（est_balance/est_coding_plan 等）+ usage stats 批量，
   *  不拉 quota HTTP、不整列表替换。高频被动触发（proxy log 订阅），整列表替换会打断 memo / 拖拽态
   *  并与乐观操作竞争回弹，故改为：仅更新已存在平台的字段，新增/删除的行交由显式写操作或 load() 处理。 */
  const refreshStats = async () => {
    const epoch = platformsEpochRef.current;
    try {
      const list = await platformApi.list();
      if (list && epoch === platformsEpochRef.current) {
        const byId = new Map(list.map(p => [p.id, p]));
        setPlatforms(prev => {
          let changed = false;
          const next = prev.map(p => {
            const fresh = byId.get(p.id);
            // 只 merge 后台派生的统计字段，保留前端排序/乐观态；字段相同则保引用（利于 memo）。
            if (!fresh) return p;
            if (
              fresh.est_balance_remaining === p.est_balance_remaining &&
              fresh.est_coding_plan === p.est_coding_plan &&
              fresh.last_real_query_at === p.last_real_query_at &&
              fresh.estimate_count === p.estimate_count
            ) return p;
            changed = true;
            return {
              ...p,
              est_balance_remaining: fresh.est_balance_remaining,
              est_coding_plan: fresh.est_coding_plan,
              last_real_query_at: fresh.last_real_query_at,
              estimate_count: fresh.estimate_count,
            };
          });
          return changed ? next : prev;
        });
      }
      const all = await platformApi.usageStatsAll();
      setUsageMap(all || {});
    } catch { /* ignore */ }
  };

  useEffect(() => { load(); }, []);

  // ⑤ 可视区优先 quota 调度：IntersectionObserver 观察每张卡片（data-platform-id），
  //    进入视口即入队（enqueueQuota 去重 + 池控并发）；滚动到更多平台时触发其余。
  //    折叠/隐藏卡片不进视口→不触发；卡片复用（DnD/重排）由 platforms 依赖重建 observer 兜底。
  useEffect(() => {
    if (platforms.length === 0) return;
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const idAttr = (entry.target as HTMLElement).dataset.platformId;
        if (!idAttr) continue;
        const pid = Number(idAttr);
        const p = quotaWantMapRef.current.get(pid);
        if (p) enqueueQuota(p);
      }
    }, { root: null, rootMargin: "200px", threshold: 0 });
    platformIObserverRef.current = observer;
    const el = platListRef.current;
    if (el) el.querySelectorAll<HTMLElement>("[data-platform-id]").forEach(card => observer.observe(card));
    return () => { observer.disconnect(); platformIObserverRef.current = null; };
  }, [platforms, enqueueQuota]);

  // 外部导航上下文（如分组展开区点「编辑」→ onNavigate("platforms",{platformId})）打开对应平台编辑页。
  // 用 ref 记录已消费的 platformId，避免后续 load/reload 重复触发；平台列表到手后再匹配，否则等下一次列表更新。
  const consumedEditPidRef = useRef<number | null>(null);
  useEffect(() => {
    const pid = initialFilter?.platformId;
    if (!pid || consumedEditPidRef.current === pid) return;
    const target = platforms.find(p => p.id === pid);
    if (!target) return;  // 列表尚未加载到该平台，待 platforms 更新后重试
    consumedEditPidRef.current = pid;
    handleEdit(target);
  }, [initialFilter?.platformId, platforms]);

  // 分组列表（multi-select 数据源 + 编辑态反查手动组归属 + 平台归属映射）。本地查询，失败不阻断编辑。
  useEffect(() => {
    groupDetailApi.list().then(setGroupDetails).catch(() => {});
  }, []);

  // groupDetails 变化时重建 membership（初始加载 + 所有 setGroupDetails 路径都覆盖）
  useEffect(() => { setPlatformMembership(buildMembership(groupDetails)); }, [groupDetails]);

  // 全局调度+熔断默认（展示「继承默认 N」用），读失败不阻断编辑。
  useEffect(() => {
    (async () => {
      try {
        setBreakerDefaults(await schedulingApi.getSettings());
      } catch (e) {
        console.error("get scheduling settings failed", e);
      }
    })();
  }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  /** 刷新单个平台 quota（合查 balance + coding_plan） */
  const refreshQuota = async (p: Platform) => {
    if (!p.api_key) {
      setToast({ text: `${p.name}: ${t("platform.quotaNoKey", "缺少 API Key")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
      return;
    }
    // 手动刷新接管该平台 quota：清初始 pending（避免与 refreshing 旋转图标骨架重叠），显式调度去重也标记。
    setQuotaPending(prev => { const n = { ...prev }; delete n[p.id]; return n; });
    quotaScheduledRef.current.add(p.id);
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
    setBreakerFailureThreshold(""); setBreakerOpenSecs(""); setBreakerHalfOpenMax("");
    setAutoGroup(true); setJoinGroupIds([]); setLockedGroupId(null);
    // 关闭表单时复位「已消费的外部编辑导航 platformId」一次性 ref：否则经 onNavigate 进来的同一
    // 平台第二次编辑会被 consumedEditPidRef 短路（initialFilter.platformId 值不变，effect 亦不重跑）。
    consumedEditPidRef.current = null;
  };

  /** 打开平台创建表单（顶部「添加平台」或分组卡片 ➕ 触发）。
   *  lockGid 提供时预绑该分组并锁定、关闭 auto_group；否则用默认（建默认分组）。 */
  const openCreatePlatform = (presetGroupIds?: number[], lockGid?: number) => {
    resetForm();
    if (lockGid != null) {
      setAutoGroup(false);
      setJoinGroupIds(presetGroupIds && presetGroupIds.length > 0 ? presetGroupIds : [lockGid]);
      setLockedGroupId(lockGid);
    }
    // chips 渲染依赖 groupDetails，确保已加载。
    groupDetailApi.list().then(setGroupDetails).catch(() => {});
    setShowForm(true);
  };

  // 跳转该平台的日志（带 platformId 筛选上下文）。
  const handleViewLogs = (p: Platform) => {
    onNavigate?.("logs", { platformId: p.id, platformName: p.name });
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
    // 熔断覆盖现存于 extra.breaker：0 = 继承 → 显示空
    {
      const brk = parsePlatformBreaker(p.extra ?? "");
      setBreakerFailureThreshold(brk.failure_threshold > 0 ? String(brk.failure_threshold) : "");
      setBreakerOpenSecs(brk.open_secs > 0 ? String(brk.open_secs) : "");
      setBreakerHalfOpenMax(brk.half_open_max > 0 ? String(brk.half_open_max) : "");
    }
    setLockedGroupId(null);
    // 反查该平台当前手动组成员（排除其 auto 分组），作为「加入已有分组」初始值。
    try {
      const gds = await groupDetailApi.list();
      setGroupDetails(gds);
      setJoinGroupIds(gds
        .filter(gd => gd.group.auto_from_platform !== String(p.id)
          && gd.platforms.some(gp => gp.platform.id === p.id))
        .map(gd => gd.group.id));
    } catch {
      setJoinGroupIds([]);
    }

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
    // opencode_zen /v1/models 无 auth 可列模型，api_key 可留空（后端兜底 $opencode）。
    if (!fetchUrl || apiKeyMissing) return;
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
      // 熔断覆盖现写入 extra.breaker：空 = 继承（写 0 → 移除 breaker 键）；负值钳为 0。
      const toBreakerNum = (s: string) => Math.max(0, Math.floor(Number(s) || 0));
      extraPayload = serializePlatformBreaker(extraPayload, {
        failure_threshold: toBreakerNum(breakerFailureThreshold),
        open_secs: toBreakerNum(breakerOpenSecs),
        half_open_max: toBreakerNum(breakerHalfOpenMax),
      });
      const extraArg = extraPayload ? extraPayload : undefined;
      // 手动预算：所有平台可设（含 mock / 有上游配额支持的平台），仅透传订阅强制清空。
      const manualBudgetsPayload: ManualBudget[] = isPassthrough ? [] : manualBudgets;
      let savedId: number | undefined;
      let saved: Platform | undefined;
      const wasEditing = !!editing;
      if (editing) {
        saved = await platformApi.update({
          id: editing.id, name, platform_type: protocol, base_url: baseUrl, api_key: apiKey,
          extra: extraArg,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
          manual_budgets: manualBudgetsPayload,
          join_group_ids: joinGroupIds,
        });
        savedId = editing.id;
      } else {
        saved = await platformApi.create({
          name, platform_type: protocol, base_url: baseUrl, api_key: apiKey,
          extra: extraArg,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
          manual_budgets: manualBudgetsPayload.length > 0 ? manualBudgetsPayload : undefined,
          auto_group: autoGroup,
          join_group_ids: joinGroupIds,
        });
        savedId = saved.id;
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

      resetForm();
      // 局部刷新：用返回的完整 Platform 单项 setState（编辑=replace / 新建=append），不整页 load()。
      // 自增 epoch 让任何在途的 load()/refreshStats 放弃覆盖，防晚到 resolve 回弹乐观结果。
      if (saved) {
        const savedPlatform = saved;
        platformsEpochRef.current++;
        if (wasEditing) {
          setPlatforms(prev => prev.map(x => x.id === savedPlatform.id ? savedPlatform : x));
        } else {
          setPlatforms(prev => prev.some(x => x.id === savedPlatform.id) ? prev : [...prev, savedPlatform]);
        }
        // 不走 load() → 不会重置 quota 调度，须显式为该平台补 quota 查询（风险④）。
        scheduleQuotaFor(savedPlatform);
        // 单项变更后补刷该平台 usage / 最近测试徽章（load() 原本顺带刷，局部路径须自补）。
        platformApi.usageStats(savedPlatform.id)
          .then(u => setUsageMap(prev => ({ ...prev, [savedPlatform.id]: u })))
          .catch(() => {});
        platformApi.lastTestResult(savedPlatform.id)
          .then(r => { if (r) setLastTestMap(prev => ({ ...prev, [savedPlatform.id]: r })); })
          .catch(() => {});
      }
      // 保存可能改变分组归属（join_group_ids / auto_group 建默认组），
      // 必须刷新 groupDetails 重建 membership，否则已分组平台漏判为未分组、误现于底部未分配区。
      handleGroupsChanged();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch (e: any) {
      const msg = e?.toString() || "Unknown error";
      console.error(msg);
      setSaveError(msg);
    }
  };

  const handleDelete = async (id: number) => {
    // 删平台后端会清理 group_platform 关联并可能删孤儿 auto 组，
    // 故须刷新 groupDetails（重建 membership chips + 已分组/未分组归属），仅刷平台列表会留陈旧分组态。
    // 局部刷新：乐观从列表按 id 移除（不整页 load），失败回滚。
    let removed: Platform | undefined;
    let removedIndex = -1;
    platformsEpochRef.current++;
    setPlatforms(prev => {
      removedIndex = prev.findIndex(x => x.id === id);
      if (removedIndex >= 0) removed = prev[removedIndex];
      return prev.filter(x => x.id !== id);
    });
    try {
      await platformApi.delete(id);
      handleGroupsChanged();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch (e) {
      console.error(e);
      // 回滚：把被删平台插回原位。
      if (removed) {
        const r = removed; const idx = removedIndex;
        setPlatforms(prev => {
          if (prev.some(x => x.id === r.id)) return prev;
          const next = [...prev];
          next.splice(idx >= 0 && idx <= next.length ? idx : next.length, 0, r);
          return next;
        });
      }
      setToast({ text: `${t("platform.deleteFail", "删除失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  const handleToggle = async (p: Platform) => {
    // 三态切换：enabled → disabled；disabled / auto_disabled → enabled（恢复并清退避）。
    const nextStatus: PlatformStatus = p.status === "enabled" ? "disabled" : "enabled";
    // 乐观更新：立即本地置换该平台 status，UI 即时响应、不调 load() 全量重拉（避免整页 loading 闪烁）。
    // status 切换不改分组归属（membership 由 groupDetails 决定），故无需广播 aidog-groups-changed。
    setPlatforms(prev => prev.map(x =>
      x.id === p.id ? { ...x, status: nextStatus, enabled: nextStatus === "enabled" } : x));
    try {
      const updated = await platformApi.update({ id: p.id, status: nextStatus });
      // 用后端返回值校正单个 item（含清退避后的派生字段），仍不动其他平台、不重拉列表。
      setPlatforms(prev => prev.map(x => x.id === p.id ? updated : x));
    } catch (e) {
      console.error(e);
      // 失败回滚该 item 到原状态 + 报错。
      setPlatforms(prev => prev.map(x => x.id === p.id ? p : x));
      setToast({ text: `${p.name}: ${t("platform.toggleFail", "切换失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  const handleQuickTest = async (p: Platform) => {
    setTestingId(p.id);
    let success = false;
    try {
      const defaultModel = p.models.default || p.available_models[0] || "";
      const r = await modelTestApi.test({ platform_id: p.id, model: defaultModel });
      success = r.success;
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
    // 派发全局事件：跨页（Groups 批量测 / ModelTestPanel 自定义）跑测后切到本页，本页卡片徽章 + health 据此即时刷新
    window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: p.id, success } }));
  };

  // 拉取某平台最近一次 test 日志，刷新 lastTestMap 对应项（供 aidog-platform-test-completed 监听后调用）
  const refreshLastTest = useCallback(async (platformId: number) => {
    try {
      const r = await platformApi.lastTestResult(platformId);
      setLastTestMap(prev => {
        const next = { ...prev };
        if (r) next[platformId] = r; else delete next[platformId];
        return next;
      });
    } catch { /* ignore */ }
  }, []);

  // 监听全局测试完成事件：单卡刷新「最近测试」徽章 + 写 testResults（驱动 health 走 manual 分支，
  // Groups 批量测 / ModelTestPanel 的成功失败信号即时反映到本页健康点）（事件来自本页快速测 / Groups 批量测 / ModelTestPanel）
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent<{ platformId: number; success?: boolean }>;
      const pid = ce.detail?.platformId;
      if (pid == null) return;
      refreshLastTest(pid);
      if (ce.detail.success != null) {
        setTestResults(prev => ({ ...prev, [pid]: ce.detail.success ? "ok" : "fail" }));
      }
    };
    window.addEventListener("aidog-platform-test-completed", handler);
    return () => window.removeEventListener("aidog-platform-test-completed", handler);
  }, [refreshLastTest]);

  // 卡片操作集合：用 latest-ref 持有最新闭包，对外暴露稳定引用，保证 PlatformCard memo 生效
  const actionsRef = useRef({
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    toggleExpanded, refreshQuota, handleToggle, handleEdit, handleDelete, handleViewLogs,
    handleQuickTest, setTestingPlatform, setFaviconFailed,
  });
  actionsRef.current = {
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    toggleExpanded, refreshQuota, handleToggle, handleEdit, handleDelete, handleViewLogs,
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
    onViewLogs: (p) => actionsRef.current.handleViewLogs(p),
    onQuickTest: (p) => actionsRef.current.handleQuickTest(p),
    onCustomTest: (p) => actionsRef.current.setTestingPlatform(p),
    onFaviconFailed: (id) => actionsRef.current.setFaviconFailed(prev => new Set(prev).add(id)),
  }), []);

  // 列表头部「启用 / 总数」派生值：仅随 platforms 变化，避免每次轮询/拖拽重渲染时重扫全列表
  const enabledCount = useMemo(() => platforms.filter(p => p.enabled).length, [platforms]);
  // 页头徽章计数：优先用 GroupsEmbedded 渐进回传值（随各组平台逐组流入增量更新），
  // 回退本页自身 platforms 派生值（progressiveCount 尚未回传 / 被重置时）。
  const headerActive = progressiveCount ? progressiveCount.active : enabledCount;
  const headerTotal = progressiveCount ? progressiveCount.total : platforms.length;

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
            {!editing && (
              <button className="btn" onClick={() => setShowPaste(true)}>
                {t("platform.paste.title", "智能识别")}
              </button>
            )}
            <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleSave}
              disabled={!name || (isPassthrough ? endpoints.length === 0 : (!isMock && !keyOptional && (endpoints.length === 0 || !apiKey)))}>
              {editing ? t("action.save") : t("action.create")}
            </button>
          </div>
        </div>

        {showPaste && (
          <SmartPasteModal
            presets={PROTOCOLS}
            onApply={applyPaste}
            onClose={() => setShowPaste(false)}
          />
        )}

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
                    color: ep.coding_plan ? "var(--color-success, var(--color-success))" : "var(--text-tertiary)",
                    background: ep.coding_plan ? "var(--color-success, var(--color-success))15" : "transparent",
                    border: `1px solid ${ep.coding_plan ? "var(--color-success, var(--color-success))40" : "var(--border)"}`,
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
                  disabled={apiKeyMissing || endpoints.length === 0 || fetching}
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
              // 下拉源：fetchModels 成功用 available_models，否则用内置候选列表（冷启动兜底）
              const dropdownSource = availableModels.length > 0
                ? availableModels
                : getDefaultModelList(protocol, codingPlan);
              const hasDropdown = dropdownSource.length > 0;
              const filtered = hasDropdown
                ? (query
                  ? dropdownSource.filter(m => pinyinMatch(query, m))
                  : dropdownSource)
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
                    style={{ width: "100%", paddingRight: hasDropdown ? 28 : undefined }}
                    placeholder={t(labelKey)}
                    value={models[key]}
                    onChange={(e) => {
                      handleModelChange(key, e.target.value);
                      if (hasDropdown) setActiveDropdown(key);
                    }}
                    onFocus={() => {
                      if (hasDropdown) setActiveDropdown(key);
                    }}
                  />
                  {hasDropdown && (
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

          {/* Circuit Breaker 熔断覆盖（仅编辑态可配；空 = 继承全局默认） */}
          {editing && !isPassthrough && (
            <FormSection
              title={t("platform.breakerTitle", "熔断阈值")}
              desc={t("platform.breakerDesc", "连续失败达阈值后临时摘除该平台，冷却后半开探测恢复。留空 = 继承系统设置的全局默认值。")}
            >
              <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: "10px 12px" }}>
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerFailureThreshold", "失败阈值")}</span>
                <input
                  className="input" type="number" min={0} style={{ width: 140 }}
                  placeholder={breakerDefaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(breakerDefaults.breaker_failure_threshold)) : t("platform.breakerInheritGeneric", "继承默认")}
                  value={breakerFailureThreshold}
                  onChange={e => setBreakerFailureThreshold(e.target.value)}
                />
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerOpenSecs", "熔断时长(秒)")}</span>
                <input
                  className="input" type="number" min={0} style={{ width: 140 }}
                  placeholder={breakerDefaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(breakerDefaults.breaker_open_secs)) : t("platform.breakerInheritGeneric", "继承默认")}
                  value={breakerOpenSecs}
                  onChange={e => setBreakerOpenSecs(e.target.value)}
                />
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerHalfOpenMax", "半开探测数")}</span>
                <input
                  className="input" type="number" min={0} style={{ width: 140 }}
                  placeholder={breakerDefaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(breakerDefaults.breaker_half_open_max)) : t("platform.breakerInheritGeneric", "继承默认")}
                  value={breakerHalfOpenMax}
                  onChange={e => setBreakerHalfOpenMax(e.target.value)}
                />
              </div>
            </FormSection>
          )}

          {/* 分组归属：是否建默认分组 + 加入已有分组（多选 chips）。
              可同时勾；都不选 = 平台不在任何分组（游离，ensure 永不补建）。 */}
          {!isPassthrough && (
            <FormSection
              title={t("platform.groupAssignTitle", "分组归属")}
              desc={t("platform.groupAssignDesc", "可同时创建默认分组并加入其他已有分组；都不选则该平台不在任何分组。")}
            >
              {lockedGroupId != null ? (
                <div style={{ fontSize: 12, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 6 }}>
                  <span className="badge badge-muted" style={{ padding: "0 6px" }}>
                    {groupDetails.find(g => g.group.id === lockedGroupId)?.group.name ?? `#${lockedGroupId}`}
                  </span>
                  {t("platform.groupLocked", "已锁定到此分组")}
                </div>
              ) : !editing ? (
                // 创建默认分组是「创建时一次性判断」，仅创建表单显示；编辑表单不再判断建组。
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                  <span style={{ fontSize: 13 }}>{t("platform.groupAssignAuto", "创建默认分组")}</span>
                  <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
                    <input type="checkbox" checked={autoGroup} onChange={e => setAutoGroup(e.target.checked)} style={{ display: "none" }} />
                    <span className={`toggle ${autoGroup ? "active" : ""}`} />
                  </label>
                </div>
              ) : null}
              {lockedGroupId == null && groupDetails.length > 0 && (
                <>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)", margin: "10px 0 6px" }}>
                    {t("platform.groupAssignJoin", "加入已有分组")}
                  </div>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                    {groupDetails
                      // 编辑态隐藏该平台自己的 auto 分组（由上方 toggle 管理）。
                      .filter(gd => !editing || gd.group.auto_from_platform !== String(editing.id))
                      .map(gd => {
                        const checked = joinGroupIds.includes(gd.group.id);
                        return (
                          <button
                            key={gd.group.id}
                            type="button"
                            onClick={() => setJoinGroupIds(prev => checked
                              ? prev.filter(id => id !== gd.group.id)
                              : [...prev, gd.group.id])}
                            style={{
                              display: "inline-flex", alignItems: "center",
                              padding: "4px 12px", borderRadius: 999, fontSize: 12, fontWeight: 500,
                              cursor: "pointer",
                              border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                              background: checked ? "var(--accent-subtle)" : "var(--bg-glass)",
                              color: checked ? "var(--accent)" : "var(--text-secondary)",
                              transition: "all 200ms cubic-bezier(0.4, 0, 0.2, 1)",
                            }}
                          >
                            {gd.group.name}
                          </button>
                        );
                      })}
                  </div>
                </>
              )}
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

          {/* Middleware rules (platform scope) — 需已有 platform_id */}
          {editing && (
            <FormSection
              title={t("middleware.platformRules", "平台中间件规则")}
              desc={t("middleware.platformRulesHint", "仅本平台生效，就近覆盖分组 / 全局同类型规则")}
            >
              <MiddlewareRulesPanel scope="platform" scopeRef={String(editing.id)} embedded />
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
            {headerTotal > 0 ? `${headerActive} / ${headerTotal} active` : t("platform.empty")}
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button className="btn btn-primary" onClick={() => openCreateGroupRef.current?.()}>
            + {t("group.add", "添加分组")}
          </button>
          <button className="btn btn-primary" onClick={() => { resetForm(); setShowForm(true); }}>
            + {t("platform.add")}
          </button>
          <button
            className="btn btn-ghost"
            onClick={async () => {
              if (!window.confirm(t("platform.purgeDisabledConfirm", "将永久删除全库失效(自动禁用)平台，不可恢复，确定？"))) return;
              try {
                const r = await platformApi.purgeDisabled();
                if (r.deletedIds.length === 0) {
                  setToast({ text: t("platform.purgeDisabledNone", "暂无失效平台"), ok: true });
                } else {
                  setToast({ text: t("platform.purgeDisabledDone", "已删除 {{count}} 个失效平台", { count: r.deletedIds.length }), ok: true });
                }
                setTimeout(() => setToast(null), 3000);
                // 局部刷新：按 deletedIds 批量移除被永久删除的平台（不整页 load）；
                // unassignedIds（仅移除分组关联，平台行保留）的归属变化由 handleGroupsChanged 重建 membership。
                if (r.deletedIds.length > 0) {
                  const del = new Set(r.deletedIds);
                  platformsEpochRef.current++;
                  setPlatforms(prev => prev.filter(x => !del.has(x.id)));
                }
                handleGroupsChanged();
                groupsReloadRef.current?.();
              } catch (err) {
                setToast({ text: `${t("platform.purgeDisabled", "清理失效平台")}: ${err}`, ok: false });
                setTimeout(() => setToast(null), 3000);
              }
            }}
            title={t("platform.purgeDisabled", "清理失效平台")}
          >
            {t("platform.purgeDisabled", "清理失效平台")}
          </button>
        </div>
      </div>

      {/* 分组段（内嵌） */}
      <GroupsEmbedded onNavigate={onNavigate} onGroupsChanged={handleGroupsChanged} onCreatePlatform={openCreatePlatform} onEditPlatform={handleEdit} onToast={setToast} onViewModeChange={setGroupFullscreen} openCreateGroupRef={openCreateGroupRef} reloadRef={groupsReloadRef} onCountChange={setProgressiveCount} />

      {/* 全屏视图态（创建/编辑分组）时隐藏分隔线 + 未分组平台列表，避免与全屏视图并列 */}
      {!groupFullscreen && (<>
      {/* 分隔线 */}
      <div style={{ height: 1, background: "var(--border)", margin: "0 0 10px 0" }} />

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
          {standalonePlatforms.map((p, i) => {
            const isDragging = platDrag?.from === i;
            const draggedPlat = platDrag ? standalonePlatforms[platDrag.from] : null;
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
                {/* 未分组平台 pointer 拖拽加入分组（按住卡片空白区拖到分组）；HTML5 DnD 跨区域在 WKWebView 失效故用 pointer events */}
                <div
                  onPointerDown={(e) => onStandaloneGroupPointerDown(e, p)}
                  onPointerMove={onStandaloneGroupPointerMove}
                  onPointerUp={onStandaloneGroupPointerUp}
                  style={{ cursor: groupDrag?.pid === p.id ? "grabbing" : undefined }}
                >
                <PlatformCard
                  platform={p}
                  index={i}
                  isDragging={isDragging}
                  dragActive={!!platDrag}
                  quota={computeQuotaDisplay(p, quotaMap[p.id], !!quotaRealIds[p.id])}
                  refreshing={!!quotaRefreshing[p.id]}
                  quotaPending={!!quotaPending[p.id]}
                  usagePending={usageLoading && !usageMap[p.id]}
                  usage={usageMap[p.id]}
                  expanded={expandedIds.has(p.id)}
                  manualResult={testResults[p.id]}
                  testing={testingId === p.id}
                  faviconFailed={faviconFailed.has(p.id)}
                  actions={cardActions}
                  platformMembership={platformMembership.get(p.id)}
                  lastTest={lastTestMap[p.id]}
                />
                </div>
              </React.Fragment>
            );
          })}
          {platDrag && (() => {
            if (platDrag.to !== standalonePlatforms.length) return null;
            const dp = standalonePlatforms[platDrag.from];
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
      </>)}
    </div>

      {/* Custom test overlay — ModelTestPanel 自带 overlay 且经 createPortal 挂 body, 此处不再包外层遮罩。 */}
      {testingPlatform !== null && (
        <ModelTestPanel
          platform={testingPlatform as Platform}
          onClose={() => setTestingPlatform(null)}
          onResult={(success) => { if (testingPlatform) setTestResults(prev => ({ ...prev, [testingPlatform.id]: success ? "ok" : "fail" })); }}
        />
      )}

      {/* Test result toast — Portal 到 body, 脱离页面 transform 祖先(animate-fade-in 等)确保 fixed 相对窗口顶部 */}
      {groupDrag && createPortal(
        <div style={{
          position: "fixed", left: groupDrag.x + 14, top: groupDrag.y + 14,
          pointerEvents: "none", zIndex: 3000,
          padding: "6px 12px", borderRadius: 8,
          background: "var(--accent)", color: "#fff",
          fontSize: 12, fontWeight: 600,
          boxShadow: "0 4px 12px rgba(0,0,0,0.35)", opacity: 0.92,
        }}>
          {groupDrag.pname}
        </div>,
        document.body,
      )}
      {toast && createPortal(
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
        </div>,
        document.body,
      )}
    </>
  );
}

