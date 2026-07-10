import type { Protocol, ModelSlot } from "../../services/api";

/** 支持的协议选项（含 coding plan 变体）。
 *  派生自 platform-presets.json（见 defaults.ts::buildProtocolsFromPresets）。 */
export type ProtocolOption = { value: Protocol; label: string; codingPlan?: boolean; keywords?: string[]; hosts?: string[]; codingKeyPrefixes?: string[] };

/** Endpoint 协议：只有 AI 请求协议（非平台类型）。
 *  请求格式协议层（openai/openai_responses/openai_completions/anthropic/gemini）——
 *  非 platform 类型，不迁 JSON；与 PROTOCOL_LABELS 5 条同集（Record vs array 不同 shape，
 *  服务不同 consumer）。 */
export const ENDPOINT_PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "openai", label: "OpenAI Chat" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "openai_completions", label: "OpenAI Completions" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Gemini" },
];

/** 客户端模拟选项已迁移到 JSON 派生层（见 defaults.ts::buildClientTypesFromPresets）。
 *  真值源 `src-tauri/defaults/client-types.json`（12 entry：1 默认 + 5 Claude Code
 *  + 4 Codex + 2 IDE），前端 invoke `get_client_types_json` 后按 locale 派生 label，
 *  禁直读 github / 文件系统。
 *  旧模块级 CLIENT_TYPES 常量已删除（见 prd `07-10-client-types-json-sync`）。 */

/** 请求格式协议 label（仅 5 条 endpoint 协议；平台类型 label 由 JSON name 经
 *  getProtocolLabelMap 派生）。PROTOCOL_LABELS 5 条与 ENDPOINT_PROTOCOLS 5 条同集，
 *  Record vs array 不同 shape，服务不同 consumer。
 *  - 请求格式展示处（如 endpoint badge）用本常量；
 *  - 平台类型展示处用 labelMap（JSON name 派生，覆盖 60+ platform）。 */
export const PROTOCOL_LABELS: Partial<Record<Protocol, string>> = {
  // ── AI 请求协议（endpoint 协议，非平台类型）──
  openai: "OpenAI",
  openai_responses: "OpenAI Responses",
  openai_completions: "OpenAI Completions",
  anthropic: "Anthropic",
  gemini: "Gemini",
};

/** 默认平台名集合（用于 handleProtocolChange 判断「name 仍是协议默认名 → 切协议时自动覆盖」）。
 *  派生自 JSON name（运行时 getProtocolLabelMap 返回的 value 集合）；此处仅 5 请求格式协议兜底，
 *  完整集合由 usePlatformForm 运行时拉取 getProtocolLabelMap 后并入。 */
export const DEFAULT_NAMES = new Set(Object.values(PROTOCOL_LABELS));

// ③ 延迟档 quota 外部 HTTP 有界并发上限（仿 Groups.tsx BATCH_TEST_CONCURRENCY=3）。
export const QUOTA_CONCURRENCY = 3;

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
