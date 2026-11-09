import { useTranslation } from "react-i18next";
import { writeText } from "../../services/platform";
import { F } from "../../domains/shared/tokens";
import { CopyButton, MetaItem, RequestTabs } from "./primitives";
import { safeParseJson } from "./types";
import type { LogsDetailData } from "./useLogsDetail";
import { formatDateTime } from "../../utils/formatters";
import { makeRipple } from "../../components/shared";
import { CopyCurlButtons } from "./CopyCurlButtons";
import { RoutingCanvas } from "./RoutingCanvas";
import { useProtocolLabel } from "../../domains/platforms/useProtocolMeta";
import type { Protocol } from "../../services/api";
import { requestClassLabel, compactionLabel, parseToolDurations } from "../../utils/requestClass";
import { Button } from "@/components/ui/button";

/**
 * 日志详情视图：独立全页（占满内容区），由调用方在 detail 非空时替换列表渲染；
 * 左上「返回」把 detail 置空回到列表。曾是居中 Dialog，2026-11-04 改独立页。
 */

/** DetailPanel 消费的字段：detail 段 + platformMap/groupName（list/detail 共用，来自 filters 段）。 */
export type DetailPanelData = LogsDetailData & {
  /** 跳转导航（隧道详情的 MITM 设置链接用） */
  onNavigate?: (id: string) => void;
  platformMap: Map<number, string>;
  groupName: (k: string) => string;
};

export function DetailPanel({ d }: { d: DetailPanelData }) {
  const { detail, t, copied, copiedId, setCopiedId, openDetail, copyDetail, platformMap, groupName, setDetail, onNavigate } = d;

  if (!detail) return null;
  return (
    <div data-testid="detail-page" style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      <DetailBody
        detail={detail}
        t={t}
        copied={copied}
        copiedId={copiedId}
        setCopiedId={setCopiedId}
        openDetail={openDetail}
        copyDetail={copyDetail}
        platformMap={platformMap}
        groupName={groupName}
        onBack={() => setDetail(null)}
        onNavigate={onNavigate}
      />
    </div>
  );
}

interface DetailBodyProps {
  detail: NonNullable<DetailPanelData["detail"]>;
  t: DetailPanelData["t"];
  copied: boolean;
  copiedId: boolean;
  setCopiedId: (v: boolean) => void;
  openDetail: (id: string) => void;
  copyDetail: (d: NonNullable<DetailPanelData["detail"]>) => void;
  platformMap: DetailPanelData["platformMap"];
  groupName: (k: string) => string;
  onBack: () => void;
  onNavigate?: (id: string) => void;
}

function DetailBody({ detail, t, copied, copiedId, setCopiedId, openDetail, copyDetail, platformMap, groupName, onBack, onNavigate }: DetailBodyProps) {
  const reqHeaders = safeParseJson(detail.request_headers);
  const reqBody = safeParseJson(detail.request_body);
  const upstreamHeaders = safeParseJson(detail.upstream_request_headers);
  const upstreamBody = detail.upstream_request_body
    ? safeParseJson(detail.upstream_request_body)
    : null;
  const upstreamRespHeaders = safeParseJson(detail.upstream_response_headers || "{}");
  // 票 06：哨兵已废；空串 = 流式中 / 日志开关关闭 / 内容未捕获。
  const upstreamRespBody = !detail.response_body
    ? t("logs.streamResponse", "(流式响应，内容未记录)")
    : safeParseJson(detail.response_body);
  const userRespHeaders = safeParseJson(detail.user_response_headers || "{}");
  const userRespBody = detail.user_response_body
    ? safeParseJson(detail.user_response_body)
    : detail.response_body
      ? safeParseJson(detail.response_body)
      : t("logs.streamResponse", "(流式响应，内容未记录)");

  // 协议直观名（共享 hook，fallback locale→en→key; copyText 仍保裸值供审计）
  const { i18n } = useTranslation();
  const sourceLabel = useProtocolLabel(detail.source_protocol as Protocol | undefined, i18n.language);
  const targetLabel = useProtocolLabel(detail.target_protocol as Protocol | undefined, i18n.language);

  const requestClassText = requestClassLabel(t, detail.request_class);
  const toolDurations = parseToolDurations(detail.prev_tool_durations ?? "");

  return (
    <>
      {/* Header（工具栏：返回/刷新/复制） */}
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button variant="ghost" className="btn-icon" data-testid="detail-back" onClick={onBack} title={t("action.back", "返回")}>
          <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M8.5 2.5L4 7l4.5 4.5" /></svg>
        </Button>
        <Button variant="ghost" className="btn-icon" onClick={() => openDetail(detail.id)} title={t("logs.refresh", "刷新")}>
          <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
        </Button>
        <Button variant="ghost" className="btn-icon ripple" onClick={(e) => { makeRipple(e); copyDetail(detail); }} title={t("logs.copyAll", "复制完整信息")}>
          {copied ? (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
          )}
        </Button>
        {/* 票 passthrough-log-fixes/04：一键复制用户侧/上游侧 curl */}
        <CopyCurlButtons detail={detail} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.title, fontWeight: 700 }}>{t("logs.detail", "请求详情")}</div>
        </div>
      </div>

      {/* Request ID — full row */}
      <div className="glass-surface" style={{ padding: "12px 20px", display: "flex", alignItems: "center", gap: 10 }}>
        <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontWeight: 600 }}>{t("logs.requestId", "请求 ID")}</span>
        <span style={{ fontSize: F.hint, fontFamily: "monospace", color: "var(--text-primary)" }}>{detail.id}</span>
        <Button
          variant="ghost"
          className="btn-icon"
          style={{ marginLeft: "auto" }}
          onClick={async () => {
            await writeText(`request_id=${detail.id}`);
            setCopiedId(true);
            setTimeout(() => setCopiedId(false), 2000);
          }}
          title={t("logs.copyRequestId", "复制请求 ID")}
        >
          {copiedId ? (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
          )}
        </Button>
      </div>

      {/* Meta grid */}
      <div className="glass-surface hover-lift" style={{ padding: 20, display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))", gap: 14 }}>
        <MetaItem label={t("logs.group", "分组")} value={groupName(detail.group_key)} copyText={detail.group_key} t={t} />
        <MetaItem label={t("logs.platform", "平台")} value={platformMap.get(detail.platform_id) || "-"} copyText={platformMap.get(detail.platform_id)} t={t} />
        <MetaItem label={t("logs.model", "请求模型")} value={detail.model || "-"} copyText={detail.model} t={t} />
        <MetaItem label={t("logs.actualModel", "实际模型")} value={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : "-"} copyText={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : undefined} t={t} />
        <MetaItem label={t("logs.client", "客户端")} value={detail.client || "-"} copyText={detail.client || undefined} testId="detail-client" t={t} />
        <MetaItem label={t("logs.userAgent", "UA")} value={detail.user_agent || "-"} copyText={detail.user_agent || undefined} testId="detail-user-agent" t={t} />
        <MetaItem label={t("logs.requestClassLabel", "请求类型")} value={requestClassText} copyText={detail.request_class || undefined} testId="detail-request-class" t={t} />
        <MetaItem label={t("logs.agentType", "子代理类型")} value={detail.agent_type || "-"} copyText={detail.agent_type || undefined} testId="detail-agent-type" t={t} />
        <MetaItem label={t("logs.promptId", "提示词 ID")} value={detail.prompt_id || "-"} copyText={detail.prompt_id || undefined} testId="detail-prompt-id" t={t} />
        <MetaItem label={t("logs.claudeSessionId", "会话 ID")} value={detail.session_id || "-"} copyText={detail.session_id || undefined} testId="detail-session-id" t={t} />
        <MetaItem label={t("logs.agentId", "子代理 ID")} value={detail.agent_id || "-"} copyText={detail.agent_id || undefined} testId="detail-agent-id" t={t} />
        <MetaItem label={t("logs.parentAgentId", "父代理 ID")} value={detail.parent_agent_id || "-"} copyText={detail.parent_agent_id || undefined} testId="detail-parent-agent-id" t={t} />
        <MetaItem label={t("logs.compactionLabel", "压缩原因")} value={compactionLabel(t, detail.compaction ?? "")} copyText={detail.compaction || undefined} testId="detail-compaction" t={t} />
        <MetaItem label={t("logs.contextCompacted", "压缩后首请求")} value={compactionLabel(t, detail.context_compacted ?? "")} copyText={detail.context_compacted || undefined} testId="detail-context-compacted" t={t} />
        {toolDurations.length > 0 && (
          <div style={{ gridColumn: "1 / -1" }} data-testid="detail-tool-durations">
            <div style={{ fontSize: F.small, color: "var(--text-tertiary)", marginBottom: 4 }}>{t("logs.prevToolDurations", "上轮工具耗时")}</div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
              {toolDurations.map((d, i) => (
                <span key={i} data-testid="detail-tool-duration" style={{ display: "inline-flex", gap: 6, fontSize: F.small, padding: "2px 8px", borderRadius: 6, background: "var(--accent-subtle)" }}>
                  <span style={{ fontFamily: "monospace" }}>{d.name}</span>
                  <span>{d.ms} ms</span>
                </span>
              ))}
            </div>
          </div>
        )}
        <MetaItem label={t("logs.sourceProtocol", "用户格式")} value={sourceLabel || detail.source_protocol || "-"} copyText={detail.source_protocol} t={t} />
        <MetaItem label={t("logs.targetProtocol", "请求格式")} value={targetLabel || detail.target_protocol || "-"} copyText={detail.target_protocol} t={t} />
        <MetaItem
          label={t("logs.status", "状态")}
          value={
            detail.status_code === 0
              ? t("logs.statusIncomplete", "未完成")
              : detail.status_code === 499
                ? t("logs.statusInterrupted", "已中断")
                : `${detail.status_code}`
          }
          highlight={detail.status_code >= 200 && detail.status_code < 300 ? "ok" : "err"}
          copyText={`${detail.status_code}`}
          t={t}
        />
        <MetaItem
          label={t("logs.upstreamStatus", "上游状态")}
          value={
            detail.upstream_status_code === 0 || detail.upstream_status_code == null
              ? t("logs.notCaptured", "未捕获")
              : `${detail.upstream_status_code}`
          }
          highlight={
            detail.upstream_status_code === 0 || detail.upstream_status_code == null
              ? undefined
              : detail.upstream_status_code >= 200 && detail.upstream_status_code < 300 ? "ok" : "err"
          }
          copyText={
            detail.upstream_status_code === 0 || detail.upstream_status_code == null
              ? undefined
              : `${detail.upstream_status_code}`
          }
          t={t}
        />
        <MetaItem label={t("logs.stream", "传输")} value={detail.is_stream ? t("logs.streaming", "流式") : t("logs.nonStreaming", "非流式")} copyText={detail.is_stream ? t("logs.streaming", "流式") : t("logs.nonStreaming", "非流式")} t={t} />
        <MetaItem label={t("logs.duration", "耗时")} value={`${detail.duration_ms} ms`} copyText={`${detail.duration_ms} ms`} t={t} />
        <MetaItem label={t("logs.inputTokens", "输入 Token")} value={`${detail.input_tokens}`} copyText={`${detail.input_tokens}`} t={t} />
        <MetaItem label={t("logs.outputTokens", "输出 Token")} value={`${detail.output_tokens}`} copyText={`${detail.output_tokens}`} t={t} />
        <MetaItem label={t("logs.cacheTokens", "缓存 Token")} value={`${detail.cache_tokens}`} copyText={`${detail.cache_tokens}`} t={t} />
        {detail.body_omitted ? (
          <MetaItem label={t("logs.bodyOmitted", "正文已省略")} value={t("logs.bodyOmittedHint", "写入过载，仅元数据")} t={t} />
        ) : null}
        <MetaItem label={t("logs.time", "时间")} value={formatDateTime(detail.created_at) || "-"} copyText={formatDateTime(detail.created_at) || "-"} t={t} />
      </div>

      {/* 未解密隧道（代理日志-MITM隧道 票 02）：说明 + 跳 MITM 解密名单设置（不提供一键加入） */}
      {detail.source_protocol === "http-connect" && detail.blocked_reason === "mitm_opaque" && (
        <div className="glass-surface" data-testid="detail-tunnel-notice" style={{ padding: "12px 20px", display: "flex", alignItems: "center", gap: 10, fontSize: F.hint }}>
          <span>{t("logs.tunnelOpaqueNotice", "目标地址不在 MITM 解密名单，内容未解密")}</span>
          <Button variant="ghost" type="button" data-testid="detail-tunnel-mitm-link" onClick={() => onNavigate?.("settings/mitm")} style={{ fontSize: F.small, height: "auto", padding: "2px 8px", color: "var(--accent)" }}>
            {t("logs.tunnelGoMitm", "前往 MITM 设置")}
          </Button>
        </div>
      )}

      {/* ── 路由决策画布（票 02：请求→路由→平台→响应 workflow；快照为空显示占位）── */}
      <RoutingCanvas
        routing={detail.routing}
        attempts={detail.attempts}
        platformMap={platformMap}
        durationMs={detail.duration_ms}
        statusCode={detail.status_code}
        actualModel={detail.actual_model || undefined}
        requestModel={detail.model || undefined}
        groupName={groupName(detail.group_key)}
        client={detail.client || undefined}
        inputTokens={detail.input_tokens}
        outputTokens={detail.output_tokens}
        estCost={detail.est_cost}
        retryCount={detail.retry_count}
        blockedBy={detail.blocked_by || undefined}
        blockedReason={detail.blocked_reason || undefined}
        t={t}
      />

      {/* ── 平台尝试时序（多平台重试时展示每次尝试：平台/状态码/耗时/错误）── */}
      {detail.attempts && detail.attempts.length >= 1 && (
        <div className="glass-surface hover-lift" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 10 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontSize: F.body, fontWeight: 600 }}>{t("logs.attempts", "尝试记录")}</span>
            <span className="badge" style={{ fontSize: 11, padding: "1px 6px", background: "color-mix(in srgb, var(--color-warning) 16%, transparent)", color: "var(--color-warning)" }}>
              {t("logs.attemptCount", "{{n}} 次").replace("{{n}}", String(detail.attempts.length))}
            </span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {detail.attempts.map((a, i) => {
              const ok = a.status_code >= 200 && a.status_code < 300;
              const platName = a.platform_name || platformMap.get(a.platform_id) || `#${a.platform_id}`;
              // 摘要串：平台名 | 状态码 | 耗时ms | 错误(若有)
              const summary = [platName, String(a.status_code), `${a.duration_ms}ms`, a.error].filter(Boolean).join(" | ");
              return (
                <div key={i} style={{
                  position: "relative",
                  display: "grid", gridTemplateColumns: "24px 1fr auto auto", alignItems: "center", gap: 10,
                  padding: "6px 28px 6px 10px", borderRadius: 8,
                  background: ok ? "color-mix(in srgb, var(--color-success) 8%, transparent)" : "color-mix(in srgb, var(--color-danger) 8%, transparent)",
                  border: `1px solid ${ok ? "color-mix(in srgb, var(--color-success) 25%, transparent)" : "color-mix(in srgb, var(--color-danger) 25%, transparent)"}`,
                }}>
                  <span style={{ fontSize: 11, color: "var(--text-tertiary)", fontFamily: "monospace" }}>#{i + 1}</span>
                  <span style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
                    <span style={{ fontSize: F.small, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {platName}
                      {a.model && (
                        <span data-testid="attempt-model" style={{ marginLeft: 6, fontSize: 11, fontWeight: 400, color: "var(--text-tertiary)" }}>
                          {a.model}
                        </span>
                      )}
                    </span>
                    {a.error && (
                      <span style={{ fontSize: 11, color: "var(--color-danger)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={a.error}>
                        {a.error}
                      </span>
                    )}
                  </span>
                  <span style={{ fontSize: F.small, fontWeight: 600, color: ok ? "var(--color-success)" : "var(--color-danger)" }}>
                    {a.status_code === 0 ? t("logs.connFailed", "连接失败") : a.status_code}
                  </span>
                  <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{a.duration_ms}ms</span>
                  <CopyButton text={summary} title={t("logs.copy", "复制")} />
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* ── 用户请求 / 上游请求 Tab ── */}
      <RequestTabs
        userTab={{
          title: t("logs.userRequest", "用户请求"),
          subtitle: "Client → Proxy",
          protocol: detail.source_protocol?.toUpperCase(),
          url: detail.request_url,
          statusCode: detail.status_code,
          reqHeaders,
          reqBody,
          respHeaders: userRespHeaders,
          respBody: userRespBody,
        }}
        upstreamTab={{
          title: t("logs.upstreamRequest", "上游请求"),
          subtitle: "Proxy → Platform",
          protocol: detail.target_protocol?.toUpperCase(),
          url: detail.upstream_request_url,
          statusCode: detail.upstream_status_code,
          reqHeaders: upstreamHeaders,
          reqBody: upstreamBody,
          respHeaders: upstreamRespHeaders,
          respBody: upstreamRespBody,
        }}
        t={t}
      />
    </>
  );
}
