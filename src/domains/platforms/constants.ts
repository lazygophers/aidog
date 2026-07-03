import type { Protocol, ClientType, ModelSlot } from "../../services/api";

/** 支持的协议选项（含 coding plan 变体） */
export type ProtocolOption = { value: Protocol; label: string; codingPlan?: boolean; keywords?: string[]; hosts?: string[]; codingKeyPrefixes?: string[] };

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
  { value: "doubao", label: "火山引擎", keywords: ["火山", "volcengine", "agentplan", "豆包", "doubao", "seed", "volces"] },
  { value: "byteplus", label: "BytePlus", keywords: ["byteplus", "字节国际"] },
  { value: "qianfan", label: "百度千帆", keywords: ["baidu", "百度", "千帆"] },
  { value: "qianfan", label: "百度千帆 Coding Plan Lite", codingPlan: true, keywords: ["baidu", "百度", "千帆", "qianfan", "coding"] },
  { value: "xiaomi_mimo", label: "小米 MiMo", keywords: ["xiaomi", "小米", "mimo"] },
  { value: "xiaomi_mimo", label: "小米 MiMo Coding Plan", codingPlan: true, keywords: ["xiaomi coding", "小米编程", "mimo token plan", "token plan"], codingKeyPrefixes: ["tp-"] },
  { value: "bailing", label: "百灵", keywords: ["bailing", "百灵", "tbox"] },
  { value: "longcat", label: "Longcat", keywords: ["longcat", "龙猫"] },
  { value: "sensenova", label: "商汤 SenseNova（日日新）", keywords: ["sensenova", "商汤", "日日新", "token.sensenova"] },
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
export const ENDPOINT_PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "openai", label: "OpenAI Chat" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "openai_completions", label: "OpenAI Completions" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Gemini" },
];

/** 客户端模拟选项：用于通过上游客户端校验 */
export const CLIENT_TYPES: { value: ClientType; labelKey?: string; label?: string; group: string }[] = [
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
  doubao: "火山引擎",
  byteplus: "BytePlus",
  qianfan: "百度千帆",
  xiaomi_mimo: "小米 MiMo",
  bailing: "百灵",
  longcat: "Longcat",
  sensenova: "商汤 SenseNova",
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

export const DEFAULT_NAMES = new Set(Object.values(PROTOCOL_LABELS));

// ③ 延迟档 quota 外部 HTTP 有界并发上限（仿 Groups.tsx BATCH_TEST_CONCURRENCY=3）。
export const QUOTA_CONCURRENCY = 3;

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

export const MOCK_ERROR_MODES: { value: import("../../services/api").MockErrorMode; labelKey: string }[] = [
  { value: "none", labelKey: "platform.mockErrorNone" },
  { value: "http_error", labelKey: "platform.mockErrorHttp" },
  { value: "rate_limit_429", labelKey: "platform.mockErrorRateLimit" },
  { value: "timeout", labelKey: "platform.mockErrorTimeout" },
];

export type HealthStatus = "healthy" | "warning" | "error" | "unknown";

export const HEALTH_COLORS: Record<HealthStatus, string> = {
  healthy: "var(--color-success, var(--color-success))",
  warning: "var(--color-warning, #ff9500)",
  error: "var(--color-danger, #ff3b30)",
  unknown: "var(--text-tertiary, #8e8e93)",
};
