// clientUsage.ts — 日志统计 → 客户端统计（issue #65 票 02b）。
// 类型逐字段对齐 aidog_core::client_usage_cmd 与 aidog_db::client_usage 的 serde camelCase。

import { invoke } from "../transport";

export interface ClientUsageConsent {
  asked: boolean;
  enabled: Record<string, boolean>;
}

/** 导入进度（内存态快照）。 */
export interface ClientUsageImportState {
  running: boolean;
  clientCode: string;
  startedAtMs: number;
  filesTotal: number;
  filesDone: number;
  callsImported: number;
  parseErrors: number;
  finishedAtMs: number;
  error: string;
}

export interface ClientUsageStatus {
  consent: ClientUsageConsent;
  import: ClientUsageImportState;
  persistedParseErrors: number;
}

export interface ClientUsageSummary {
  calls: number;
  unpricedCalls: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
  cost: number;
}

export interface ClientUsageTrendRow {
  dayMs: number;
  calls: number;
  inputTokens: number;
  outputTokens: number;
  cost: number;
  unpricedCalls: number;
}

export interface ClientUsageModelRow {
  model: string;
  calls: number;
  inputTokens: number;
  outputTokens: number;
  cost: number;
  unpricedCalls: number;
}

export interface ClientUsageSessionRow {
  clientCode: string;
  sessionId: string;
  title: string;
  workdir: string;
  firstCallMs: number;
  lastCallMs: number;
  calls: number;
  cost: number;
  unpricedCalls: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
  /** 子代理调用数。 */
  sidechainCalls: number;
}

/** 模型链一段：按调用时间相邻同模型合并。 */
export interface ClientUsageModelSegment {
  model: string;
  calls: number;
  firstCallMs: number;
  lastCallMs: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
}

/** token 时间序列一点（每次调用一点）。 */
export interface ClientUsageTokenPoint {
  callTimeMs: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
}

/** 详情里主线 / 子代理一组；Claude Code 子代理带 agentId，agentType 读自 meta.json（可缺）。 */
export interface ClientUsageAgentGroup {
  kind: "main" | "subagent";
  agentId: string | null;
  agentType: string | null;
  calls: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  cost: number;
  unpricedCalls: number;
}

export interface ClientUsageSessionDetail {
  modelChain: ClientUsageModelSegment[];
  points: ClientUsageTokenPoint[];
  agents: ClientUsageAgentGroup[];
}

export const clientUsageApi = {
  consentGet: () => invoke<ClientUsageConsent>("client_usage_consent_get"),
  consentSet: (consent: ClientUsageConsent) =>
    invoke<void>("client_usage_consent_set", { consent }),
  status: () => invoke<ClientUsageStatus>("client_usage_status"),
  summary: (fromMs: number, toMs: number, clientCode?: string) =>
    invoke<ClientUsageSummary>("client_usage_summary", { fromMs, toMs, clientCode }),
  trend: (fromMs: number, toMs: number, clientCode?: string) =>
    invoke<ClientUsageTrendRow[]>("client_usage_trend", { fromMs, toMs, clientCode }),
  byModel: (fromMs: number, toMs: number, clientCode?: string) =>
    invoke<ClientUsageModelRow[]>("client_usage_by_model", { fromMs, toMs, clientCode }),
  sessions: (fromMs: number, toMs: number, clientCode?: string, limit?: number) =>
    invoke<ClientUsageSessionRow[]>("client_usage_sessions", { fromMs, toMs, clientCode, limit }),
  sessionDetail: (clientCode: string, sessionId: string) =>
    invoke<ClientUsageSessionDetail>("client_usage_session_detail", { clientCode, sessionId }),
  refresh: () => invoke<ClientUsageImportState>("client_usage_refresh"),
  /** 保留天数（0 = 永久，缺省 90）。 */
  retentionGet: () => invoke<number>("client_usage_retention_get"),
  retentionSet: (days: number) => invoke<void>("client_usage_retention_set", { days }),
  /** 「允许读取对话原文」开关（缺省 false）。 */
  transcriptConsentGet: () => invoke<boolean>("client_usage_transcript_consent_get"),
  transcriptConsentSet: (enabled: boolean) =>
    invoke<void>("client_usage_transcript_consent_set", { enabled }),
};
