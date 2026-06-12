import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  proxyLogApi,
  platformApi,
  groupDetailApi,
  type ProxyLogSummary,
  type ProxyLogDetail,
  type ProxyLogFilter,
  type Platform,
  type GroupDetail,
} from "../services/api";
import { IconClose } from "../components/icons";
import { usePolling } from "../hooks/usePolling";

const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;
const PAGE_SIZE = 50;

/** 时间范围预设 */
type TimePreset = "all" | "1h" | "6h" | "24h" | "7d" | "30d";

function timePresetToRange(preset: TimePreset): { start?: number; end?: number } {
  if (preset === "all") return {};
  const now = Date.now();
  const ms: Record<string, number> = { "1h": 3600000, "6h": 21600000, "24h": 86400000, "7d": 604800000, "30d": 2592000000 };
  return { start: now - (ms[preset] ?? 0), end: now };
}

export function Logs() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<ProxyLogSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(true);
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  const [copied, setCopied] = useState(false);

  // ── Filter state ──
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [filterPlatform, setFilterPlatform] = useState<string>("");   // platform_id or ""
  const [filterGroup, setFilterGroup] = useState<string>("");
  const [filterStatus, setFilterStatus] = useState<string>("");       // "" | "success" | "error"
  const [filterTime, setFilterTime] = useState<TimePreset>("all");
  const [filterModelType, setFilterModelType] = useState<"original" | "actual">("actual");
  const [filterModelText, setFilterModelText] = useState<string>("");

  // Load platforms & groups for filter dropdowns
  useEffect(() => {
    platformApi.list().then(setPlatforms).catch(() => {});
    groupDetailApi.list().then(setGroups).catch(() => {});
  }, []);

  // Build filter object
  const activeFilter: ProxyLogFilter = useMemo(() => {
    const f: ProxyLogFilter = {};
    if (filterPlatform) f.platform_id = Number(filterPlatform);
    if (filterGroup) f.group_name = filterGroup;
    if (filterStatus === "success") f.status = 200;
    else if (filterStatus === "error") f.status = -1;
    const tr = timePresetToRange(filterTime);
    if (tr.start) f.time_start = tr.start;
    if (tr.end) f.time_end = tr.end;
    if (filterModelText.trim()) {
      f.model = filterModelText.trim();
      f.model_type = filterModelType;
    }
    return f;
  }, [filterPlatform, filterGroup, filterStatus, filterTime, filterModelText, filterModelType]);

  // Check if any filter is active
  const hasFilter = !!(filterPlatform || filterGroup || filterStatus || filterTime !== "all" || filterModelText.trim());

  // Collect unique models from a large unfiltered query so options stay stable
  // regardless of the current filter selection.
  const [modelOptions, setModelOptions] = useState<string[]>([]);
  useEffect(() => {
    (async () => {
      try {
        const items = await proxyLogApi.list(200, 0);
        const col = filterModelType === "actual" ? "actual_model" : "model";
        const set = new Set<string>();
        (items || []).forEach(l => { if ((l as any)[col]) set.add((l as any)[col]); });
        setModelOptions(Array.from(set).sort());
      } catch { /* ignore */ }
    })();
  }, [filterModelType]);

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
      if (hasFilter) {
        const [items, count] = await Promise.all([
          proxyLogApi.listFiltered(activeFilter, PAGE_SIZE, offset),
          proxyLogApi.countFiltered(activeFilter),
        ]);
        setLogs(items || []);
        setTotal(count);
      } else {
        const [items, count] = await Promise.all([
          proxyLogApi.list(PAGE_SIZE, offset),
          proxyLogApi.count(),
        ]);
        setLogs(items || []);
        setTotal(count);
      }
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [offset, hasFilter, activeFilter]);

  useEffect(() => { load(); }, [load]);

  // Reset offset when filter changes
  useEffect(() => { setOffset(0); }, [hasFilter, activeFilter]);

  // Auto-refresh every 3s on list view（页面不可见时暂停）
  const refreshList = useCallback(() => { load(true); }, [load]);
  usePolling(refreshList, 3000, !detail);

  const handleClear = async () => {
    if (!confirm(t("logs.clearConfirm", "确认清除所有日志？此操作不可撤销。"))) return;
    try {
      await proxyLogApi.clear();
      setOffset(0);
      load();
    } catch (e) { console.error(e); }
  };

  const clearFilter = () => {
    setFilterPlatform("");
    setFilterGroup("");
    setFilterStatus("");
    setFilterTime("all");
    setFilterModelText("");
    setFilterModelType("actual");
  };

  const openDetail = async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) setDetail(d);
    } catch (e) { console.error(e); }
  };

  // Auto-refresh detail every 2s while viewing（页面不可见时暂停）
  const refreshDetail = useCallback(() => {
    if (!detail) return;
    proxyLogApi.get(detail.id)
      .then(d => { if (d) setDetail(d); })
      .catch(() => {});
  }, [detail]);
  usePolling(refreshDetail, 2000, !!detail);

  // Build platform lookup — must run before any conditional return to keep hook order stable
  const platformMap = useMemo(() => {
    const m = new Map<number, string>();
    platforms.forEach(p => m.set(p.id, p.name));
    return m;
  }, [platforms]);

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
      <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
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
          </div>
        </div>

        {/* Request ID — full row */}
        <div className="glass-surface" style={{ padding: "12px 20", display: "flex", alignItems: "center", gap: 10 }}>
          <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontWeight: 600 }}>{t("logs.requestId", "请求 ID")}</span>
          <span style={{ fontSize: F.hint, fontFamily: "monospace", color: "var(--text-primary)" }}>{detail.id}</span>
        </div>

        {/* Meta grid */}
        <div className="glass-surface" style={{ padding: 20, display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))", gap: 14 }}>
          <MetaItem label={t("logs.group", "分组")} value={detail.group_name} />
          <MetaItem label={t("logs.model", "原始模型")} value={detail.model || "-"} />
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
      </div>
    );
  }

  // ── List view ──
  const totalPages = Math.ceil(total / PAGE_SIZE);
  const currentPage = Math.floor(offset / PAGE_SIZE) + 1;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
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

      {/* ── Filter bar ── */}
      <div className="glass-surface" style={{ padding: "12px 16", display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" }}>
        {/* Platform */}
        <FilterSelect
          value={filterPlatform}
          onChange={setFilterPlatform}
          options={platforms.map(p => ({ value: String(p.id), label: p.name }))}
          placeholder={t("logs.filterPlatform", "平台")}
        />
        {/* Group */}
        <FilterSelect
          value={filterGroup}
          onChange={setFilterGroup}
          options={groups.map(g => ({ value: g.group.name, label: g.group.name }))}
          placeholder={t("logs.filterGroup", "分组")}
        />
        {/* Status */}
        <FilterSelect
          value={filterStatus}
          onChange={setFilterStatus}
          options={[
            { value: "success", label: t("logs.statusSuccess", "成功") },
            { value: "error", label: t("logs.statusError", "失败") },
          ]}
          placeholder={t("logs.filterStatus", "状态")}
        />
        {/* Time range */}
        <FilterSelect
          value={filterTime}
          onChange={v => setFilterTime(v as TimePreset)}
          options={[
            { value: "1h", label: "1h" },
            { value: "6h", label: "6h" },
            { value: "24h", label: "24h" },
            { value: "7d", label: "7d" },
            { value: "30d", label: "30d" },
          ]}
          placeholder={t("logs.filterTime", "时间")}
        />
        {/* Model type toggle */}
        <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: F.small }}>
          <button
            className={`btn btn-ghost ${filterModelType === "actual" ? "active" : ""}`}
            style={{ padding: "2px 8px", fontSize: F.small, fontWeight: filterModelType === "actual" ? 700 : 400, opacity: filterModelType === "actual" ? 1 : 0.6 }}
            onClick={() => setFilterModelType("actual")}
          >{t("logs.actualModel", "实际模型")}</button>
          <button
            className={`btn btn-ghost ${filterModelType === "original" ? "active" : ""}`}
            style={{ padding: "2px 8px", fontSize: F.small, fontWeight: filterModelType === "original" ? 700 : 400, opacity: filterModelType === "original" ? 1 : 0.6 }}
            onClick={() => setFilterModelType("original")}
          >{t("logs.model", "原始模型")}</button>
        </div>
        {/* Model dropdown — options from unfiltered query */}
        <FilterSelect
          value={filterModelText}
          onChange={setFilterModelText}
          options={modelOptions.map(m => ({ value: m, label: m }))}
          placeholder={t("logs.filterModel", "模型")}
        />
        {/* Clear */}
        {hasFilter && (
          <button className="btn btn-ghost" onClick={clearFilter} style={{ fontSize: F.small, padding: "2px 8px", color: "var(--text-tertiary)" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}><IconClose size={11} /> {t("logs.clearFilter", "清除")}</span>
          </button>
        )}
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
                  <ThCell>{t("logs.platform", "平台")}</ThCell>
                  <ThCell>{t("logs.model", "原始模型")}</ThCell>
                  <ThCell>{t("logs.actualModel", "实际模型")}</ThCell>
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
                    <TdCell><span style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{platformMap.get(log.platform_id) || "-"}</span></TdCell>
                    <TdCell><span style={{ fontWeight: 500, fontSize: F.small }}>{log.model || "-"}</span></TdCell>
                    <TdCell><span style={{ fontWeight: 500, fontSize: F.small }}>{log.actual_model || "-"}</span></TdCell>
                    <TdCell>
                      <span style={{ color: log.status_code >= 200 && log.status_code < 300 ? "var(--color-success, #34c759)" : "var(--color-danger, #ff3b30)" }}>
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
          {total > 0 && (
            <Pagination
              currentPage={currentPage}
              totalPages={totalPages}
              total={total}
              pageSize={PAGE_SIZE}
              onPageChange={page => setOffset((page - 1) * PAGE_SIZE)}
            />
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

/** 分页导航：首页/上一页/页码按钮/下一页/末页 + 总数 */
function Pagination({
  currentPage, totalPages, total, pageSize, onPageChange,
}: {
  currentPage: number;
  totalPages: number;
  total: number;
  pageSize: number;
  onPageChange: (page: number) => void;
}) {
  const rangeStart = (currentPage - 1) * pageSize + 1;
  const rangeEnd = Math.min(currentPage * pageSize, total);

  // Generate page buttons: show at most 7 buttons with ellipsis
  const pages: (number | "ellipsis")[] = [];
  if (totalPages <= 7) {
    for (let i = 1; i <= totalPages; i++) pages.push(i);
  } else {
    pages.push(1);
    if (currentPage > 3) pages.push("ellipsis");
    const start = Math.max(2, currentPage - 1);
    const end = Math.min(totalPages - 1, currentPage + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (currentPage < totalPages - 2) pages.push("ellipsis");
    pages.push(totalPages);
  }

  const btnStyle: React.CSSProperties = {
    fontSize: 12, padding: "4px 8px", minWidth: 28, textAlign: "center",
  };

  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
      <span className="text-tertiary" style={{ fontSize: 12 }}>
        {rangeStart}–{rangeEnd} / {total}
      </span>
      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(1)} title="First">⟪</button>
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(currentPage - 1)}>←</button>
        {pages.map((p, i) =>
          p === "ellipsis" ? (
            <span key={`e${i}`} className="text-tertiary" style={{ fontSize: 12, padding: "0 4px" }}>…</span>
          ) : (
            <button key={p} className={`btn ${p === currentPage ? "" : "btn-ghost"}`}
              style={{
                ...btnStyle,
                ...(p === currentPage ? { fontWeight: 700, color: "var(--accent)" } : {}),
              }}
              onClick={() => onPageChange(p)}>{p}</button>
          ),
        )}
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(currentPage + 1)}>→</button>
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(totalPages)} title="Last">⟫</button>
      </div>
    </div>
  );
}

/** 通用筛选下拉 */
function FilterSelect({
  value,
  onChange,
  options,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  placeholder: string;
}) {
  return (
    <select
      value={value}
      onChange={e => onChange(e.target.value)}
      style={{
        fontSize: F.small,
        padding: "4px 8px",
        borderRadius: 6,
        border: "1px solid var(--border)",
        background: "var(--bg-secondary, rgba(255,255,255,0.05))",
        color: "var(--text-primary)",
        cursor: "pointer",
        maxWidth: 140,
      }}
    >
      <option value="">{placeholder}</option>
      {options.map(o => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  );
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

/** Tab 切换：用户请求 / 上游请求 */
function RequestTabs({
  userTab, upstreamTab, t,
}: {
  userTab: { title: string; subtitle: string; protocol?: string; url?: string; statusCode?: number; reqHeaders: any; reqBody: any; respHeaders: any; respBody: any };
  upstreamTab: { title: string; subtitle: string; protocol?: string; url?: string; statusCode?: number; reqHeaders: any; reqBody: any; respHeaders: any; respBody: any };
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const [active, setActive] = useState<"user" | "upstream">("user");
  const tab = active === "user" ? userTab : upstreamTab;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 0 }}>
      {/* Tab bar */}
      <div style={{ display: "flex", gap: 0, borderBottom: "1px solid var(--border)" }}>
        {(["user", "upstream"] as const).map((key) => {
          const item = key === "user" ? userTab : upstreamTab;
          const isActive = active === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => setActive(key)}
              style={{
                padding: "10px 20px", fontSize: F.hint, fontWeight: isActive ? 700 : 400,
                color: isActive ? "var(--accent)" : "var(--text-secondary)",
                background: "transparent", border: "none", cursor: "pointer",
                borderBottom: isActive ? "2px solid var(--accent)" : "2px solid transparent",
                transition: "all 0.15s ease",
                display: "flex", alignItems: "center", gap: 8,
              }}
            >
              {item.title}
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontWeight: 400 }}>{item.subtitle}</span>
              {item.protocol && <span className="badge" style={{ fontSize: 10, padding: "1px 5px" }}>{item.protocol}</span>}
              {item.statusCode != null && item.statusCode > 0 && (
                <span style={{
                  fontSize: F.small, fontWeight: 600,
                  color: item.statusCode >= 200 && item.statusCode < 300 ? "var(--color-success, #34c759)" : "var(--color-danger, #ff3b30)",
                }}>
                  {item.statusCode}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* Tab content */}
      <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 12 }}>
        <RequestSectionContent {...tab} t={t} />
      </div>
    </div>
  );
}

/** 请求内容渲染（无折叠，纯内容） */
function RequestSectionContent({
  url, reqHeaders, reqBody, respHeaders, respBody, t,
}: {
  url?: string;
  reqHeaders: any;
  reqBody: any;
  respHeaders: any;
  respBody: any;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const bodyStr = (v: any) => typeof v === "string" ? v : JSON.stringify(v, null, 2);
  const emptyBody = !reqBody && !respBody;

  if (emptyBody) {
    return (
      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>
        {t("logs.noUpstream", "(未捕获)")}
      </div>
    );
  }

  return (
    <>
      {url && (
        <div>
          <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>URL</div>
          <pre className="code-block" style={{ maxHeight: 60, overflow: "auto", wordBreak: "break-all", whiteSpace: "pre-wrap" }}>{url}</pre>
        </div>
      )}
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.requestHeaders", "请求头")}
        </div>
        <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>
          {typeof reqHeaders === "string" ? reqHeaders : JSON.stringify(reqHeaders, null, 2)}
        </pre>
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.requestBody", "请求体")}
        </div>
        {reqBody
          ? <pre className="code-block" style={{ maxHeight: 300, overflow: "auto" }}>{bodyStr(reqBody)}</pre>
          : <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>-</div>
        }
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.responseHeaders", "响应头")}
        </div>
        <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>
          {typeof respHeaders === "string" ? respHeaders : JSON.stringify(respHeaders, null, 2)}
        </pre>
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.responseBody", "响应体")}
        </div>
        <pre className="code-block" style={{ maxHeight: 400, overflow: "auto" }}>{bodyStr(respBody)}</pre>
      </div>
    </>
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
