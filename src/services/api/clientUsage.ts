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

/** 回收站行（票 09）：会话行 + 隐藏时刻。 */
export type ClientUsageHiddenSessionRow = ClientUsageSessionRow & { hiddenAt: number };

/** 会话运行状态（票 05）：执行中 / 空闲 / 已结束。 */
export type SessionRunState = "running" | "idle" | "stopped";

export interface ClientUsageSessionState {
  clientCode: string;
  sessionId: string;
  state: SessionRunState;
  /** pid_file = Claude Code pid 文件；gateway = 网关在途请求；activity = 最近活动时间阈值。 */
  source: "pid_file" | "gateway" | "activity";
  lastActivityMs: number;
}

/** `session_resume` 结果：launched=false 表示本平台不能开终端，command 供复制。 */
export interface SessionResumeResult {
  launched: boolean;
  command: string;
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
  /** 从列表隐藏 / 恢复（票 09）：只记标记，不删客户端文件。 */
  sessionHide: (clientCode: string, sessionId: string) =>
    invoke<void>("client_usage_session_hide", { clientCode, sessionId }),
  sessionUnhide: (clientCode: string, sessionId: string) =>
    invoke<void>("client_usage_session_unhide", { clientCode, sessionId }),
  hiddenSessions: (fromMs?: number, toMs?: number) =>
    invoke<ClientUsageHiddenSessionRow[]>("client_usage_hidden_sessions", { fromMs, toMs }),
  sessionStates: (sessionIds: string[], clientCode?: string) =>
    invoke<ClientUsageSessionState[]>("client_usage_session_states", { clientCode, sessionIds }),
  /** 继续会话（票 07）：先 cd 到工作目录再跑各家继续命令；launched=false 时前端复制 command。 */
  sessionResume: (clientCode: string, sessionId: string, workdir: string) =>
    invoke<SessionResumeResult>("session_resume", { clientCode, sessionId, workdir }),
  refresh: () => invoke<ClientUsageImportState>("client_usage_refresh"),
  /** 保留天数（0 = 永久，缺省 90）。 */
  retentionGet: () => invoke<number>("client_usage_retention_get"),
  retentionSet: (days: number) => invoke<void>("client_usage_retention_set", { days }),
  /** 「允许读取对话原文」开关（缺省 false）。 */
  transcriptConsentGet: () => invoke<boolean>("client_usage_transcript_consent_get"),
  transcriptConsentSet: (enabled: boolean) =>
    invoke<void>("client_usage_transcript_consent_set", { enabled }),
};
