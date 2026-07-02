import { useState, useEffect, useCallback, useMemo, memo } from "react";
import { useTranslation } from "react-i18next";
import {
  proxyLogApi,
  platformApi,
  groupDetailApi,
  onProxyLogUpdated,
  type ProxyLogSummary,
  type ProxyLogDetail,
  type ProxyLogFilter,
  type Platform,
  type GroupDetail,
} from "../services/api";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { IconClose } from "../components/icons";
import { usePolling } from "../hooks/usePolling";
import { FilterDropdown } from "../components/shared";
import { F } from "../domains/shared/tokens";

const PAGE_SIZE_OPTIONS = [20, 50, 100] as const;
const DEFAULT_PAGE_SIZE = 20;

// ── 行内固定 style 提模块级常量（避免每行每次渲染重建对象，且让 LogRow memo 不被 inline 对象击穿）──
const ROW_STYLE: React.CSSProperties = { cursor: "pointer", borderBottom: "1px solid var(--border)" };
const INLINE_FLEX_STYLE: React.CSSProperties = { display: "inline-flex", alignItems: "center", gap: 6 };
const PLATFORM_NAME_STYLE: React.CSSProperties = { fontSize: F.small, color: "var(--text-secondary)" };
const RETRY_BADGE_STYLE: React.CSSProperties = { fontSize: 10, padding: "1px 5px", background: "color-mix(in srgb, var(--color-warning) 16%, transparent)", color: "var(--color-warning)" };
const MODEL_NAME_STYLE: React.CSSProperties = { fontWeight: 500, fontSize: F.small };
const SSE_BADGE_STYLE: React.CSSProperties = { fontSize: 10, padding: "1px 5px", background: "var(--accent-subtle)", color: "var(--accent, #007aff)" };
const ACTION_BTN_STYLE: React.CSSProperties = { padding: 2 };
const GROUP_BADGE_STYLE: React.CSSProperties = { fontSize: 11 };

/** 时间范围预设 */
type TimePreset = "all" | "1h" | "6h" | "24h" | "7d" | "30d";

function timePresetToRange(preset: TimePreset): { start?: number; end?: number } {
  if (preset === "all") return {};
  const now = Date.now();
  const ms: Record<string, number> = { "1h": 3600000, "6h": 21600000, "24h": 86400000, "7d": 604800000, "30d": 2592000000 };
  return { start: now - (ms[preset] ?? 0), end: now };
}

export function Logs({ initialFilter }: { initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string } }) {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<ProxyLogSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState<number>(DEFAULT_PAGE_SIZE);
  const [loading, setLoading] = useState(true);
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  const [copied, setCopied] = useState(false);
  const [copiedId, setCopiedId] = useState(false);

  // ── Filter state ──
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [filterPlatform, setFilterPlatform] = useState<string>(initialFilter?.platformId ? String(initialFilter.platformId) : "");   // platform_id or ""
  const [filterGroup, setFilterGroup] = useState<string>(initialFilter?.groupKey ?? "");
  const [filterStatus, setFilterStatus] = useState<string>("");       // "" | "success" | "error"
  const [filterTime, setFilterTime] = useState<TimePreset>("all");
  const [filterModelType, setFilterModelType] = useState<"original" | "actual">("actual");
  const [filterModelText, setFilterModelText] = useState<string>("");
  const [filterPath, setFilterPath] = useState<string>("");

  // Load platforms & groups for filter dropdowns
  useEffect(() => {
    platformApi.list().then(setPlatforms).catch(() => {});
    groupDetailApi.list().then(setGroups).catch(() => {});
  }, []);

  // Build filter object
  // 「无平台」= platform_id 0（隧道请求 host 未命中任何平台）；值 "0" truthy 故 if 命中。
  // 「无分组」= group_key ''（隧道请求无 apikey）；"__none__" sentinel 映射到空串，避与「全部」("") 撞。
  const NO_GROUP_SENTINEL = "__none__";
  const activeFilter: ProxyLogFilter = useMemo(() => {
    const f: ProxyLogFilter = {};
    if (filterPlatform) f.platform_id = Number(filterPlatform);
    if (filterGroup) f.group_key = filterGroup === NO_GROUP_SENTINEL ? "" : filterGroup;
    if (filterStatus === "success") f.status = 200;
    else if (filterStatus === "error") f.status = -1;
    const tr = timePresetToRange(filterTime);
    if (tr.start) f.time_start = tr.start;
    if (tr.end) f.time_end = tr.end;
    if (filterModelText.trim()) {
      f.model = filterModelText.trim();
      f.model_type = filterModelType;
    }
    if (filterPath.trim()) f.path = filterPath.trim();
    return f;
  }, [filterPlatform, filterGroup, filterStatus, filterTime, filterModelText, filterModelType, filterPath]);

  // Check if any filter is active
  const hasFilter = !!(filterPlatform || filterGroup || filterStatus || filterTime !== "all" || filterModelText.trim() || filterPath.trim());

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

  const copyDetail = useCallback(async (d: ProxyLogDetail) => {
    const fj = (s: string) => {
      try { return JSON.stringify(JSON.parse(s), null, 2); } catch { return s; }
    };
    const lines = [
      `# Proxy Log ${d.id}`,
      ``,
      `## Meta`,
      `- ID: ${d.id}`,
      `- Group: ${d.group_key}`,
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
      (d.user_response_body && d.user_response_body !== "[stream]")
        ? fj(d.user_response_body)
        : (d.response_body && d.response_body !== "[stream]")
          ? fj(d.response_body)
          : "(streaming, not captured)",
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
      (d.response_body && d.response_body !== "[stream]") ? fj(d.response_body) : "(streaming, not captured)",
    ];
    try {
      await writeText(lines.join("\n"));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  }, []);

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      if (hasFilter) {
        const [items, count] = await Promise.all([
          proxyLogApi.listFiltered(activeFilter, pageSize, offset),
          proxyLogApi.countFiltered(activeFilter),
        ]);
        setLogs(items || []);
        setTotal(count);
      } else {
        const [items, count] = await Promise.all([
          proxyLogApi.list(pageSize, offset),
          proxyLogApi.count(),
        ]);
        setLogs(items || []);
        setTotal(count);
      }
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [offset, pageSize, hasFilter, activeFilter]);

  useEffect(() => { load(); }, [load]);

  // Reset offset when filter or page size changes (avoid out-of-range empty page)
  useEffect(() => { setOffset(0); }, [hasFilter, activeFilter, pageSize]);

  // 兜底轮询 30s（防事件丢失 + 流式收敛；空闲页可见时 0 IPC，事件不来不刷）
  const refreshList = useCallback(() => { load(true); }, [load]);
  usePolling(refreshList, 30_000, !detail);
  // 后端 emit "proxy-log-updated" → 500ms debounce 聚合高频 emit 后实时刷新列表
  useEffect(() => onProxyLogUpdated(() => { refreshList(); }, 500), [refreshList]);

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
    setFilterPath("");
  };

  const openDetail = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) setDetail(d);
    } catch (e) { console.error(e); }
  }, []);

  // 复制单行完整信息（按 id 拉详情后复用 copyDetail）。稳定引用，保 LogRow memo 生效。
  const copyRow = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) await copyDetail(d);
    } catch (err) { console.error(err); }
  }, [copyDetail]);

  // 兜底轮询 5s（防流式事件丢失；详情打开时页面可见才跑）
  const refreshDetail = useCallback(() => {
    if (!detail) return;
    proxyLogApi.get(detail.id)
      .then(d => { if (d) setDetail(d); })
      .catch(() => {});
  }, [detail]);
  usePolling(refreshDetail, 5_000, !!detail);
  // 后端 emit "proxy-log-updated" → 1000ms debounce（流式单条 log 多次 emit，避免高频 reload 详情）
  useEffect(() => onProxyLogUpdated(() => { refreshDetail(); }, 1000), [refreshDetail]);

  // Build platform lookup — must run before any conditional return to keep hook order stable
  const platformMap = useMemo(() => {
    const m = new Map<number, string>();
    platforms.forEach(p => m.set(p.id, p.name));
    return m;
  }, [platforms]);

  // group_key → 显示名 name（日志按 group_key 归属，展示反查人类可读名）
  const groupNameMap = useMemo(() => {
    const m = new Map<string, string>();
    groups.forEach(g => m.set(g.group.group_key, g.group.name));
    return m;
  }, [groups]);
  const groupName = (k: string) => (k && groupNameMap.get(k)) || k;

  // ── Detail view ──
  if (detail) {
    const reqHeaders = safeParseJson(detail.request_headers);
    const reqBody = safeParseJson(detail.request_body);
    const upstreamHeaders = safeParseJson(detail.upstream_request_headers);
    const upstreamBody = detail.upstream_request_body
      ? safeParseJson(detail.upstream_request_body)
      : null;
    const upstreamRespHeaders = safeParseJson(detail.upstream_response_headers || "{}");
    // 流式日志现已聚合真实 SSE 内容；仅当仍为 "[stream]" 占位（日志开关关闭 / 内容未捕获）才显示提示。
    const upstreamRespBody = detail.response_body === "[stream]" || !detail.response_body
      ? t("logs.streamResponse", "(流式响应，内容未记录)")
      : safeParseJson(detail.response_body);
    const userRespHeaders = safeParseJson(detail.user_response_headers || "{}");
    const userRespBody = detail.user_response_body && detail.user_response_body !== "[stream]"
      ? safeParseJson(detail.user_response_body)
      : detail.response_body && detail.response_body !== "[stream]"
        ? safeParseJson(detail.response_body)
        : t("logs.streamResponse", "(流式响应，内容未记录)");

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
              <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, var(--color-success))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
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
          <button
            className="btn btn-ghost btn-icon"
            style={{ marginLeft: "auto" }}
            onClick={async () => {
              await writeText(`request_id=${detail.id}`);
              setCopiedId(true);
              setTimeout(() => setCopiedId(false), 2000);
            }}
            title={t("logs.copyRequestId", "复制请求 ID")}
          >
            {copiedId ? (
              <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, var(--color-success))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
            ) : (
              <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
            )}
          </button>
        </div>

        {/* Meta grid */}
        <div className="glass-surface" style={{ padding: 20, display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))", gap: 14 }}>
          <MetaItem label={t("logs.group", "分组")} value={groupName(detail.group_key)} copyText={detail.group_key} t={t} />
          <MetaItem label={t("logs.platform", "平台")} value={platformMap.get(detail.platform_id) || "-"} copyText={platformMap.get(detail.platform_id)} t={t} />
          <MetaItem label={t("logs.model", "原始模型")} value={detail.model || "-"} copyText={detail.model} t={t} />
          <MetaItem label={t("logs.actualModel", "实际模型")} value={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : "-"} copyText={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : undefined} t={t} />
          <MetaItem label={t("logs.sourceProtocol", "用户格式")} value={detail.source_protocol || "-"} copyText={detail.source_protocol} t={t} />
          <MetaItem label={t("logs.targetProtocol", "请求格式")} value={detail.target_protocol || "-"} copyText={detail.target_protocol} t={t} />
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
          <MetaItem label={t("logs.time", "时间")} value={new Date(detail.created_at).toLocaleString()} copyText={new Date(detail.created_at).toLocaleString()} t={t} />
        </div>

        {/* ── 平台尝试时序（多平台重试时展示每次尝试：平台/状态码/耗时/错误）── */}
        {detail.attempts && detail.attempts.length >= 1 && (
          <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{ fontSize: F.body, fontWeight: 600 }}>{t("logs.attempts", "尝试记录")}</span>
              <span className="badge" style={{ fontSize: 10, padding: "1px 6px", background: "color-mix(in srgb, var(--color-warning) 16%, transparent)", color: "var(--color-warning)" }}>
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
                      </span>
                      {a.error && (
                        <span style={{ fontSize: 10, color: "var(--color-danger)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={a.error}>
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
      </div>
    );
  }

  // ── List view ──
  const totalPages = Math.ceil(total / pageSize);
  const currentPage = Math.floor(offset / pageSize) + 1;

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
        <FilterDropdown
          width={140}
          value={filterPlatform}
          onChange={setFilterPlatform}
          options={[
            ...platforms.map(p => ({ value: String(p.id), label: p.name })),
            // 隧道请求 host 未命中任何平台 → platform_id=0
            { value: "0", label: t("logs.noPlatform", "无平台") },
          ]}
          allLabel={t("logs.filterPlatform", "平台")}
          searchPlaceholder={t("stats.searchPlatform", "搜索平台")}
          emptyLabel={t("stats.noMatch", "无匹配结果")}
        />
        {/* Group */}
        <FilterDropdown
          width={140}
          value={filterGroup}
          onChange={setFilterGroup}
          options={[
            ...groups.map(g => ({ value: g.group.group_key, label: g.group.name })),
            // 隧道请求无 apikey → group_key=''（sentinel 映射见 activeFilter）
            { value: NO_GROUP_SENTINEL, label: t("logs.noGroup", "无分组") },
          ]}
          allLabel={t("logs.filterGroup", "分组")}
          searchPlaceholder={t("stats.searchGroup", "搜索分组")}
          emptyLabel={t("stats.noMatch", "无匹配结果")}
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
        <FilterDropdown
          width={170}
          value={filterModelText}
          onChange={setFilterModelText}
          options={modelOptions.map(m => ({ value: m, label: m }))}
          allLabel={t("logs.filterModel", "模型")}
          searchPlaceholder={t("stats.searchModel", "搜索模型")}
          emptyLabel={t("stats.noMatch", "无匹配结果")}
        />
        {/* Path search — LIKE match on request_url */}
        <input
          type="text"
          value={filterPath}
          onChange={e => setFilterPath(e.target.value)}
          placeholder={t("logs.filterPath", "搜索路径（如 /v1/messages）")}
          style={{
            fontSize: F.small,
            padding: "4px 8px",
            borderRadius: 6,
            border: "1px solid var(--border)",
            background: "var(--bg-secondary, rgba(255,255,255,0.05))",
            color: "var(--text-primary)",
            maxWidth: 180,
            minWidth: 120,
          }}
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
                  <ThCell sticky>{""}</ThCell>
                </tr>
              </thead>
              <tbody>
                {logs.map((log) => (
                  <LogRow
                    key={log.id}
                    log={log}
                    platformName={platformMap.get(log.platform_id) || "-"}
                    groupName={groupName(log.group_key)}
                    onOpen={openDetail}
                    onCopy={copyRow}
                    t={t}
                  />
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
              pageSize={pageSize}
              onPageChange={page => setOffset((page - 1) * pageSize)}
              onPageSizeChange={setPageSize}
              t={t}
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

/**
 * 日志列表单行。React.memo + 稳定 props（platformName 传字符串、onOpen/onCopy 传 useCallback、style 用模块级常量）
 * → 父组件因轮询/筛选频繁重渲染时，未变化的行跳过重渲染、不重建行内 style 对象。
 */
interface LogRowProps {
  log: ProxyLogSummary;
  platformName: string;
  groupName: string;
  onOpen: (id: string) => void;
  onCopy: (id: string) => void;
  t: ReturnType<typeof useTranslation>["t"];
}

const LogRow = memo(function LogRow({ log, platformName, groupName, onOpen, onCopy, t }: LogRowProps) {
  return (
    <tr
      className="log-row"
      onClick={() => onOpen(log.id)}
      style={ROW_STYLE}>
      <TdCell>{new Date(log.created_at).toLocaleString()}</TdCell>
      <TdCell><span className="badge badge-accent" style={GROUP_BADGE_STYLE}>{groupName}</span></TdCell>
      <TdCell>
        <span style={INLINE_FLEX_STYLE}>
          <span style={PLATFORM_NAME_STYLE}>{platformName}</span>
          {log.retry_count > 0 && (
            <span className="badge" style={RETRY_BADGE_STYLE}
              title={t("logs.retriedHint", "经过 {{n}} 次重试").replace("{{n}}", String(log.retry_count))}>
              ↻{log.retry_count}
            </span>
          )}
        </span>
      </TdCell>
      <TdCell>
        <span style={INLINE_FLEX_STYLE}>
          <span style={MODEL_NAME_STYLE}>{log.model || "-"}</span>
          {log.is_stream && (
            <span className="badge" style={SSE_BADGE_STYLE} title={t("logs.streaming", "流式")}>SSE</span>
          )}
        </span>
      </TdCell>
      <TdCell><span style={MODEL_NAME_STYLE}>{log.actual_model || "-"}</span></TdCell>
      <TdCell>
        <span style={{ color: log.status_code >= 200 && log.status_code < 300 ? "var(--color-success, var(--color-success))" : "var(--color-danger, #ff3b30)" }}>
          {log.status_code === 0
            ? t("logs.statusIncomplete", "未完成")
            : log.status_code === 499
              ? t("logs.statusInterrupted", "已中断")
              : log.status_code}
        </span>
      </TdCell>
      <TdCell>{log.duration_ms}ms</TdCell>
      <TdCell>{log.input_tokens || "-"}</TdCell>
      <TdCell>{log.output_tokens || "-"}</TdCell>
      <TdCell sticky>
        <button
          className="btn btn-ghost btn-icon"
          style={ACTION_BTN_STYLE}
          title={t("logs.copyAll", "复制完整信息")}
          onClick={(e) => { e.stopPropagation(); onCopy(log.id); }}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
        </button>
      </TdCell>
    </tr>
  );
});

/** 分页导航：首页/上一页/页码按钮/下一页/末页 + 总数 */
function Pagination({
  currentPage, totalPages, total, pageSize, onPageChange, onPageSizeChange, t,
}: {
  currentPage: number;
  totalPages: number;
  total: number;
  pageSize: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
  t: ReturnType<typeof useTranslation>["t"];
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
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span className="text-tertiary" style={{ fontSize: 12 }}>
          {rangeStart}–{rangeEnd} / {total}
        </span>
        <label style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
          <span className="text-tertiary" style={{ fontSize: 12 }}>{t("logs.pageSize", "每页")}</span>
          <select
            aria-label={t("logs.pageSize", "每页")}
            value={pageSize}
            onChange={e => onPageSizeChange(Number(e.target.value))}
            style={{
              fontSize: F.small,
              padding: "4px 8px",
              borderRadius: 6,
              border: "1px solid var(--border)",
              background: "var(--bg-secondary, rgba(255,255,255,0.05))",
              color: "var(--text-primary)",
              cursor: "pointer",
            }}
          >
            {PAGE_SIZE_OPTIONS.map(size => (
              <option key={size} value={size}>{size}</option>
            ))}
          </select>
        </label>
      </div>
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

// ── 单元素复制按钮（GitHub 风：右上角浮动图标，copied ✓ 2s 反馈）──
// ponytail: 自管 copied state（不挤占父级 copied/copiedId），复用已 import 的 writeText + 现有 copy/check svg 对
const COPY_ICON_STYLE: React.CSSProperties = {
  position: "absolute", top: 4, right: 4, zIndex: 2,
  display: "inline-flex", alignItems: "center", justifyContent: "center",
  width: 24, height: 24, padding: 0,
  background: "color-mix(in srgb, var(--bg-surface) 70%, transparent)",
  border: "1px solid var(--border)", borderRadius: 6, cursor: "pointer",
  color: "var(--text-secondary)", opacity: 0.55, transition: "opacity 0.15s ease",
};
function CopyButton({ text, title }: { text: string; title?: string }) {
  const [copied, setCopied] = useState(false);
  // 空 / 占位串不渲染（未捕获 / streaming / 空 body）
  if (!text) return null;
  return (
    <button
      type="button"
      className="copy-btn"
      style={COPY_ICON_STYLE}
      title={title}
      onClick={async (e) => {
        e.stopPropagation();
        try {
          await writeText(text);
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        } catch (err) { console.error(err); }
      }}
    >
      {copied ? (
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, var(--color-success))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
      ) : (
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
      )}
    </button>
  );
}

function MetaItem({ label, value, highlight, copyText, t }: { label: string; value: string; highlight?: "ok" | "err"; copyText?: string; t?: ReturnType<typeof useTranslation>["t"] }) {
  return (
    <div style={{ position: "relative" }}>
      <div style={{ fontSize: F.small, color: "var(--text-tertiary)", marginBottom: 2 }}>{label}</div>
      <div style={{
        fontSize: F.body, fontWeight: 600,
        color: highlight === "ok" ? "var(--color-success, var(--color-success))" : highlight === "err" ? "var(--color-danger, #ff3b30)" : "var(--text-primary)",
        paddingRight: copyText ? 24 : undefined,
      }}>
        {value}
      </div>
      {copyText && copyText !== "-" && <CopyButton text={copyText} title={t?.("logs.copy", "复制")} />}
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
                  color: item.statusCode >= 200 && item.statusCode < 300 ? "var(--color-success, var(--color-success))" : "var(--color-danger, #ff3b30)",
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
  // ponytail: 占位串（未捕获 / 流式未记录）不复制的判定基准 — 对比当前 locale 翻译后的占位文本
  const streamPlaceholder = t("logs.streamResponse", "(流式响应，内容未记录)");
  const isPlaceholder = (s: string) => !s || s === streamPlaceholder;
  // headers 展示文本 + 空判定（空对象 "{}" / 空串不可复制）
  const headersText = (h: any) => typeof h === "string" ? h : JSON.stringify(h, null, 2);
  const headersEmpty = (h: any) => !h || headersText(h) === "{}";
  const copyTitle = t("logs.copy", "复制");
  // 包装器 position:relative 让 CopyButton 绝对定位贴右上，不受 <pre> 内部 overflow:auto 滚动影响（按钮挂外层容器非滚动内容）

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
          <div style={{ position: "relative" }}>
            <pre className="code-block" style={{ maxHeight: 60, overflow: "auto", wordBreak: "break-all", whiteSpace: "pre-wrap" }}>{url}</pre>
            <CopyButton text={url} title={copyTitle} />
          </div>
        </div>
      )}
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.requestHeaders", "请求头")}
        </div>
        <div style={{ position: "relative" }}>
          <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>{headersText(reqHeaders)}</pre>
          {!headersEmpty(reqHeaders) && <CopyButton text={headersText(reqHeaders)} title={copyTitle} />}
        </div>
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.requestBody", "请求体")}
        </div>
        {reqBody
          ? (
            <div style={{ position: "relative" }}>
              <pre className="code-block" style={{ maxHeight: 300, overflow: "auto" }}>{bodyStr(reqBody)}</pre>
              {!isPlaceholder(bodyStr(reqBody)) && <CopyButton text={bodyStr(reqBody)} title={copyTitle} />}
            </div>
          )
          : <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>-</div>
        }
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.responseHeaders", "响应头")}
        </div>
        <div style={{ position: "relative" }}>
          <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>{headersText(respHeaders)}</pre>
          {!headersEmpty(respHeaders) && <CopyButton text={headersText(respHeaders)} title={copyTitle} />}
        </div>
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.responseBody", "响应体")}
        </div>
        <div style={{ position: "relative" }}>
          <pre className="code-block" style={{ maxHeight: 400, overflow: "auto" }}>{bodyStr(respBody)}</pre>
          {!isPlaceholder(bodyStr(respBody)) && <CopyButton text={bodyStr(respBody)} title={copyTitle} />}
        </div>
      </div>
    </>
  );
}

function ThCell({ children, sticky }: { children: React.ReactNode; sticky?: boolean }) {
  return (
    <th style={{
      padding: "10px 14px", textAlign: "left", fontWeight: 600,
      color: "var(--text-secondary)", whiteSpace: "nowrap", fontSize: F.small,
      ...(sticky ? {
        position: "sticky" as const, right: 0, zIndex: 2,
        background: "var(--bg-surface)",
        boxShadow: "-4px 0 8px -4px var(--shadow-color, rgba(0,0,0,0.08))",
      } : {}),
    }}>
      {children}
    </th>
  );
}

function TdCell({ children, sticky }: { children: React.ReactNode; sticky?: boolean }) {
  return (
    <td style={{
      padding: "10px 14px", whiteSpace: "nowrap",
      ...(sticky ? {
        position: "sticky" as const, right: 0, zIndex: 2,
        background: "var(--bg-surface)",
        boxShadow: "-4px 0 8px -4px var(--shadow-color, rgba(0,0,0,0.08))",
      } : {}),
    }}>
      {children}
    </td>
  );
}
