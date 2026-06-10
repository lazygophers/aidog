import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  proxyLogApi,
  type ProxyLogSummary,
  type ProxyLogDetail,
} from "../services/api";

const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;
const PAGE_SIZE = 50;

export function Logs() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<ProxyLogSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(true);
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  const [copied, setCopied] = useState(false);

  const copyDetail = async (d: ProxyLogDetail) => {
    const fj = (s: string) => {
      try { return JSON.stringify(JSON.parse(s), null, 2); } catch { return s; }
    };
    const lines = [
      `# Proxy Log ${d.id}`,
      ``,
      `## Meta`,
      `- ID: ${d.id}`,
      `- Group: ${d.group_name}`,
      `- Model: ${d.model || "-"}`,
      `- Actual Model: ${d.actual_model || "-"}`,
      `- Source Protocol: ${d.source_protocol || "-"}`,
      `- Target Protocol: ${d.target_protocol || "-"}`,
      `- Status: ${d.status_code}`,
      `- Duration: ${d.duration_ms} ms`,
      `- Input Tokens: ${d.input_tokens}`,
      `- Output Tokens: ${d.output_tokens}`,
      `- Cache Tokens: ${d.cache_tokens}`,
      `- Time: ${d.created_at}`,
      ``,
      `## User Request (Client → Proxy)`,
      `- URL: ${d.request_url || "-"}`,
      `- Status Code: ${d.status_code}`,
      `### Request Headers`,
      fj(d.request_headers),
      ``,
      `### Request Body`,
      fj(d.request_body),
      ``,
      `### Response Headers`,
      fj(d.user_response_headers || "{}"),
      ``,
      `### Response Body`,
      d.user_response_body === "[stream]" ? "(streaming)" : fj(d.user_response_body || d.response_body),
      ``,
      `## Upstream Request (Proxy → Platform)`,
      `- URL: ${d.upstream_request_url || "-"}`,
      `- Status Code: ${d.upstream_status_code || "-"}`,
      `### Request Headers`,
      fj(d.upstream_request_headers),
      ``,
      `### Request Body`,
      d.upstream_request_body ? fj(d.upstream_request_body) : "(not captured)",
      ``,
      `### Response Headers`,
      fj(d.upstream_response_headers || "{}"),
      ``,
      `### Response Body`,
      d.response_body === "[stream]" ? "(streaming)" : fj(d.response_body),
    ];
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  };

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      const [items, count] = await Promise.all([
        proxyLogApi.list(PAGE_SIZE, offset),
        proxyLogApi.count(),
      ]);
      setLogs(items || []);
      setTotal(count);
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [offset]);

  useEffect(() => { load(); }, [load]);

  // Auto-refresh every 3s on list view
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    const id = setInterval(() => {
      if (mountedRef.current && !detail) load(true);
    }, 3000);
    return () => { mountedRef.current = false; clearInterval(id); };
  }, [load, detail]);

  const handleClear = async () => {
    if (!confirm(t("logs.clearConfirm", "确认清除所有日志？此操作不可撤销。"))) return;
    try {
      await proxyLogApi.clear();
      setOffset(0);
      load();
    } catch (e) { console.error(e); }
  };

  const openDetail = async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) setDetail(d);
    } catch (e) { console.error(e); }
  };

  // Auto-refresh detail every 2s while viewing
  useEffect(() => {
    if (!detail) return;
    const id = setInterval(() => {
      proxyLogApi.get(detail.id)
        .then(d => { if (d) setDetail(d); })
        .catch(() => {});
    }, 2000);
    return () => clearInterval(id);
  }, [detail?.id]);

  // ── Detail view ──
  if (detail) {
    const reqHeaders = safeParseJson(detail.request_headers);
    const reqBody = safeParseJson(detail.request_body);
    const upstreamHeaders = safeParseJson(detail.upstream_request_headers);
    const upstreamBody = detail.upstream_request_body
      ? safeParseJson(detail.upstream_request_body)
      : null;
    const upstreamRespHeaders = safeParseJson(detail.upstream_response_headers || "{}");
    const upstreamRespBody = detail.response_body === "[stream]"
      ? t("logs.streamResponse", "(流式响应，内容未记录)")
      : safeParseJson(detail.response_body);
    const userRespHeaders = safeParseJson(detail.user_response_headers || "{}");
    const userRespBody = detail.user_response_body === "[stream]" || !detail.user_response_body
      ? detail.user_response_body === "[stream]"
        ? t("logs.streamResponse", "(流式响应，内容未记录)")
        : detail.response_body === "[stream]"
          ? t("logs.streamResponse", "(流式响应，内容未记录)")
          : safeParseJson(detail.response_body)
      : safeParseJson(detail.user_response_body);

    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 800, width: "100%" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon" onClick={() => setDetail(null)}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M19 12H5M12 19l-7-7 7-7" />
            </svg>
          </button>
          <button className="btn btn-ghost btn-icon" onClick={() => openDetail(detail.id)} title={t("logs.refresh", "刷新")}>
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
          </button>
          <button className="btn btn-ghost btn-icon" onClick={() => copyDetail(detail)} title={t("logs.copyAll", "复制完整信息")}>
            {copied ? (
              <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, #34c759)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
            ) : (
              <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
            )}
          </button>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: F.title, fontWeight: 700 }}>{t("logs.detail", "请求详情")}</div>
            <div className="text-secondary" style={{ fontSize: F.hint }}>{detail.id.slice(0, 8)}</div>
          </div>
        </div>

        {/* Meta */}
        <div className="glass-surface" style={{ padding: 20, display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))", gap: 14 }}>
          <MetaItem label={t("logs.requestId", "请求 ID")} value={detail.id} />
          <MetaItem label={t("logs.group", "分组")} value={detail.group_name} />
          <MetaItem label={t("logs.model", "模型")} value={detail.model || "-"} />
          <MetaItem label={t("logs.actualModel", "实际模型")} value={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : "-"} />
          <MetaItem label={t("logs.sourceProtocol", "用户格式")} value={detail.source_protocol || "-"} />
          <MetaItem label={t("logs.targetProtocol", "请求格式")} value={detail.target_protocol || "-"} />
          <MetaItem label={t("logs.status", "状态")} value={`${detail.status_code}`} highlight={detail.status_code === 200 ? "ok" : "err"} />
          <MetaItem label={t("logs.duration", "耗时")} value={`${detail.duration_ms} ms`} />
          <MetaItem label={t("logs.inputTokens", "输入 Token")} value={`${detail.input_tokens}`} />
          <MetaItem label={t("logs.outputTokens", "输出 Token")} value={`${detail.output_tokens}`} />
          <MetaItem label={t("logs.cacheTokens", "缓存 Token")} value={`${detail.cache_tokens}`} />
          <MetaItem label={t("logs.time", "时间")} value={new Date(detail.created_at).toLocaleString()} />
        </div>

        {/* ── 用户请求 ── */}
        <RequestSection
          title={t("logs.userRequest", "用户请求")}
          subtitle="Client → Proxy"
          protocol={detail.source_protocol?.toUpperCase()}
          url={detail.request_url}
          statusCode={detail.status_code}
          reqHeaders={reqHeaders}
          reqBody={reqBody}
          respHeaders={userRespHeaders}
          respBody={userRespBody}
          t={t}
        />

        {/* ── 上游请求 ── */}
        <RequestSection
          title={t("logs.upstreamRequest", "上游请求")}
          subtitle="Proxy → Platform"
          protocol={detail.target_protocol?.toUpperCase()}
          url={detail.upstream_request_url}
          statusCode={detail.upstream_status_code}
          reqHeaders={upstreamHeaders}
          reqBody={upstreamBody}
          respHeaders={upstreamRespHeaders}
          respBody={upstreamRespBody}
          t={t}
        />
      </div>
    );
  }

  // ── List view ──
  const totalPages = Math.ceil(total / PAGE_SIZE);
  const currentPage = Math.floor(offset / PAGE_SIZE) + 1;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 900, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.logs", "请求日志")}</div>
          <div className="section-desc">
            {total > 0 ? `${total} ${t("logs.total", "条记录")}` : t("logs.empty", "暂无日志")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn" onClick={() => load()} disabled={loading} style={{ fontSize: F.hint }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
          </button>
          {total > 0 && (
            <button className="btn btn-danger" onClick={handleClear} style={{ fontSize: F.hint }}>
              {t("logs.clear", "清除全部")}
            </button>
          )}
        </div>
      </div>

      {/* Log Table */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : logs.length === 0 ? (
        <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
          <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("logs.empty")}</div>
        </div>
      ) : (
        <>
          <div className="glass-surface" style={{ overflow: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: F.hint }}>
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border)" }}>
                  <ThCell>{t("logs.time")}</ThCell>
                  <ThCell>{t("logs.group")}</ThCell>
                  <ThCell>{t("logs.model")}</ThCell>
                  <ThCell>{t("logs.status")}</ThCell>
                  <ThCell>{t("logs.duration")}</ThCell>
                  <ThCell>{t("logs.inputTokens")}</ThCell>
                  <ThCell>{t("logs.outputTokens")}</ThCell>
                  <ThCell>{""}</ThCell>
                </tr>
              </thead>
              <tbody>
                {logs.map((log) => (
                  <tr key={log.id}
                    className="log-row"
                    onClick={() => openDetail(log.id)}
                    style={{ cursor: "pointer", borderBottom: "1px solid var(--border)" }}>
                    <TdCell>{new Date(log.created_at).toLocaleString()}</TdCell>
                    <TdCell><span className="badge badge-accent" style={{ fontSize: 11 }}>{log.group_name}</span></TdCell>
                    <TdCell><span style={{ fontWeight: 500 }}>{log.actual_model || log.model || "-"}</span></TdCell>
                    <TdCell>
                      <span style={{ color: log.status_code === 200 ? "var(--color-success, #34c759)" : "var(--color-danger, #ff3b30)" }}>
                        {log.status_code}
                      </span>
                    </TdCell>
                    <TdCell>{log.duration_ms}ms</TdCell>
                    <TdCell>{log.input_tokens || "-"}</TdCell>
                    <TdCell>{log.output_tokens || "-"}</TdCell>
                    <TdCell>
                      <button
                        className="btn btn-ghost btn-icon"
                        style={{ padding: 2 }}
                        title={t("logs.copyAll", "复制完整信息")}
                        onClick={async (e) => {
                          e.stopPropagation();
                          try {
                            const d = await proxyLogApi.get(log.id);
                            if (d) await copyDetail(d);
                          } catch (err) { console.error(err); }
                        }}
                      >
                        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
                      </button>
                    </TdCell>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div style={{ display: "flex", justifyContent: "center", alignItems: "center", gap: 12 }}>
              <button className="btn" disabled={currentPage <= 1}
                onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>
                ←
              </button>
              <span className="text-secondary" style={{ fontSize: F.hint }}>
                {currentPage} / {totalPages}
              </span>
              <button className="btn" disabled={currentPage >= totalPages}
                onClick={() => setOffset(offset + PAGE_SIZE)}>
                →
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

// ── Helpers ──

function safeParseJson(str: string): any {
  try { return JSON.parse(str); } catch { return str; }
}

function MetaItem({ label, value, highlight }: { label: string; value: string; highlight?: "ok" | "err" }) {
  return (
    <div>
      <div style={{ fontSize: F.small, color: "var(--text-tertiary)", marginBottom: 2 }}>{label}</div>
      <div style={{
        fontSize: F.body, fontWeight: 600,
        color: highlight === "ok" ? "var(--color-success, #34c759)" : highlight === "err" ? "var(--color-danger, #ff3b30)" : "var(--text-primary)",
      }}>
        {value}
      </div>
    </div>
  );
}

/** 请求区块组件 — 显示完整的 URL / Status / Req Headers / Req Body / Resp Headers / Resp Body */
function RequestSection({
  title, subtitle, protocol, url, statusCode,
  reqHeaders, reqBody, respHeaders, respBody, t,
}: {
  title: string;
  subtitle: string;
  protocol?: string;
  url?: string;
  statusCode?: number;
  reqHeaders: any;
  reqBody: any;
  respHeaders: any;
  respBody: any;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const [open, setOpen] = useState(true);
  const bodyStr = (v: any) => typeof v === "string" ? v : JSON.stringify(v, null, 2);
  const emptyBody = !reqBody && !respBody;

  return (
    <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 12 }}>
      {/* Section header */}
      <div style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}
        onClick={() => setOpen(!open)}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor"
          strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"
          style={{ transform: open ? "rotate(90deg)" : "rotate(0)", transition: "transform 0.15s" }}>
          <path d="M5 3l4 4-4 4" />
        </svg>
        <div style={{ fontSize: F.hint, fontWeight: 700 }}>{title}</div>
        <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{subtitle}</span>
        {protocol && <span className="badge" style={{ fontSize: 11, padding: "2px 6px" }}>{protocol}</span>}
        {statusCode != null && statusCode > 0 && (
          <span style={{
            fontSize: F.small, fontWeight: 600, marginLeft: "auto",
            color: statusCode >= 200 && statusCode < 300 ? "var(--color-success, #34c759)" : "var(--color-danger, #ff3b30)",
          }}>
            HTTP {statusCode}
          </span>
        )}
      </div>

      {open && !emptyBody && (
        <>
          {/* URL */}
          {url && (
            <div>
              <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>URL</div>
              <pre className="code-block" style={{ maxHeight: 60, overflow: "auto", wordBreak: "break-all", whiteSpace: "pre-wrap" }}>{url}</pre>
            </div>
          )}

          {/* Request Headers */}
          <div>
            <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
              {t("logs.requestHeaders", "请求头")}
            </div>
            <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>
              {typeof reqHeaders === "string" ? reqHeaders : JSON.stringify(reqHeaders, null, 2)}
            </pre>
          </div>

          {/* Request Body */}
          <div>
            <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
              {t("logs.requestBody", "请求体")}
            </div>
            {reqBody
              ? <pre className="code-block" style={{ maxHeight: 300, overflow: "auto" }}>{bodyStr(reqBody)}</pre>
              : <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>-</div>
            }
          </div>

          {/* Response Headers */}
          <div>
            <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
              {t("logs.responseHeaders", "响应头")}
            </div>
            <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>
              {typeof respHeaders === "string" ? respHeaders : JSON.stringify(respHeaders, null, 2)}
            </pre>
          </div>

          {/* Response Body */}
          <div>
            <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
              {t("logs.responseBody", "响应体")}
            </div>
            <pre className="code-block" style={{ maxHeight: 400, overflow: "auto" }}>{bodyStr(respBody)}</pre>
          </div>
        </>
      )}

      {open && emptyBody && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>
          {t("logs.noUpstream", "(未捕获)")}
        </div>
      )}
    </div>
  );
}

function ThCell({ children }: { children: React.ReactNode }) {
  return (
    <th style={{
      padding: "10px 14px", textAlign: "left", fontWeight: 600,
      color: "var(--text-secondary)", whiteSpace: "nowrap", fontSize: F.small,
    }}>
      {children}
    </th>
  );
}

function TdCell({ children }: { children: React.ReactNode }) {
  return (
    <td style={{ padding: "10px 14px", whiteSpace: "nowrap" }}>
      {children}
    </td>
  );
}
