import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  requestLogApi,
  proxyLogApi,
  platformApi,
  cliProxyApi,
  onProxyLogUpdated,
  type RequestLogSummary,
  type ProxyLogDetail,
  type ProxyLogFilter,
  type Platform,
  type CliProxyProvider,
} from "../services/api";
import { usePolling } from "../hooks/usePolling";
import { F } from "../domains/shared/tokens";
import { LogRow, Pagination, FilterSelect, ThCell } from "./Logs/primitives";
import { DetailPanel } from "./Logs/DetailPanel";
import { timePresetToRange, type TimePreset } from "./Logs/types";
import { formatDateTime } from "../utils/formatters";

// ponytail: RequestLog 自管 list + filter + detail state；复用 Logs/primitives (LogRow/Pagination/ThCell/FilterSelect)
// + Logs/DetailPanel（ProxyLogDetail 经 proxyLogApi.get 取回 — request_log_list 仅摘要行）。
// 筛选维度: 类型(test/quota) / 平台 / cli-proxy provider / 状态 / 时间 — 独立于 Logs 主页。
// 后端 request_log_list 默认 sources=[test,quota]（db 兜底），前端 filter.sources 显式覆盖。

const DEFAULT_PAGE_SIZE = 20;

/** 类型筛选值 → 映射到后端 sources 字段 */
type TypeFilter = "all" | "test" | "quota";

function typeToSources(t: TypeFilter): string[] | undefined {
  if (t === "all") return undefined; // 后端默认 [test,quota]
  return [t];
}

function buildMarkdown(d: ProxyLogDetail): string {
  const fj = (s: string) => { try { return JSON.stringify(JSON.parse(s), null, 2); } catch { return s; } };
  return [
    `# Request Log ${d.id}`,
    ``,
    `## Meta`,
    `- Group: ${d.group_key}`,
    `- Model: ${d.model || "-"}`,
    `- Actual Model: ${d.actual_model || "-"}`,
    `- Source Protocol: ${d.source_protocol || "-"}`,
    `- Target Protocol: ${d.target_protocol || "-"}`,
    `- Status: ${d.status_code}`,
    `- Duration: ${d.duration_ms} ms`,
    `- Time: ${formatDateTime(d.created_at)}`,
    ``,
    `## User Request`,
    `- URL: ${d.request_url || "-"}`,
    `### Request Headers`, fj(d.request_headers), ``,
    `### Request Body`, fj(d.request_body), ``,
    `### Response Body`,
    (d.user_response_body && d.user_response_body !== "[stream]")
      ? fj(d.user_response_body)
      : (d.response_body && d.response_body !== "[stream]")
        ? fj(d.response_body)
        : "(streaming, not captured)",
    ``,
    `## Upstream Request`,
    `- URL: ${d.upstream_request_url || "-"}`,
    `### Request Headers`, fj(d.upstream_request_headers), ``,
    `### Request Body`, d.upstream_request_body ? fj(d.upstream_request_body) : "(not captured)", ``,
    `### Response Body`,
    (d.response_body && d.response_body !== "[stream]") ? fj(d.response_body) : "(streaming, not captured)",
  ].join("\n");
}

export function RequestLog() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<RequestLogSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
  const [loading, setLoading] = useState(true);

  // ── Filter state ──
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [providers, setProviders] = useState<CliProxyProvider[]>([]);
  const [filterType, setFilterType] = useState<TypeFilter>("all");
  const [filterPlatform, setFilterPlatform] = useState<string>("");
  const [filterProvider, setFilterProvider] = useState<string>("");
  const [filterStatus, setFilterStatus] = useState<string>("");
  const [filterTime, setFilterTime] = useState<TimePreset>("all");

  // ── Detail state ──
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  const [copied, setCopied] = useState(false);
  const [copiedId, setCopiedId] = useState(false);

  useEffect(() => {
    platformApi.list().then(setPlatforms).catch(() => {});
    cliProxyApi.list().then(setProviders).catch(() => {});
  }, []);

  const activeFilter: ProxyLogFilter = useMemo(() => {
    const f: ProxyLogFilter = {};
    const srcs = typeToSources(filterType);
    if (srcs) f.sources = srcs;
    if (filterPlatform) f.platform_id = Number(filterPlatform);
    if (filterProvider) f.cli_proxy_provider_id = Number(filterProvider);
    if (filterStatus === "success") f.status = 200;
    else if (filterStatus === "error") f.status = -1;
    const tr = timePresetToRange(filterTime);
    if (tr.start) f.time_start = tr.start;
    if (tr.end) f.time_end = tr.end;
    return f;
  }, [filterType, filterPlatform, filterProvider, filterStatus, filterTime]);

  const hasFilter = !!(filterType !== "all" || filterPlatform || filterProvider || filterStatus || filterTime !== "all");

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      // ponytail: request_log_list 不返 count；复用 proxy_log_count_filtered（同 build_filter_where）
      // 作精确 total。filter 等价（sources/exclude_sources/platform_id/... 同语义）。
      const countFilter: ProxyLogFilter = { ...activeFilter };
      if (!countFilter.sources) countFilter.sources = ["test", "quota"];
      const [items, count] = await Promise.all([
        requestLogApi.list(activeFilter, pageSize, offset),
        proxyLogApi.countFiltered(countFilter),
      ]);
      setLogs(items || []);
      setTotal(count);
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [activeFilter, pageSize, offset]);

  useEffect(() => { load(); }, [load]);
  useEffect(() => { setOffset(0); }, [hasFilter, activeFilter, pageSize]);

  const refreshList = useCallback(() => { load(true); }, [load]);
  usePolling(refreshList, 30_000, !detail);
  useEffect(() => onProxyLogUpdated(() => { refreshList(); }, 500), [refreshList]);

  const openDetail = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) setDetail(d);
    } catch (e) { console.error(e); }
  }, []);

  const copyDetail = useCallback(async (d: ProxyLogDetail) => {
    try {
      await writeText(buildMarkdown(d));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  }, []);

  const copyRow = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) await copyDetail(d);
    } catch (err) { console.error(err); }
  }, [copyDetail]);

  const refreshDetail = useCallback(() => {
    if (!detail) return;
    proxyLogApi.get(detail.id).then(d => { if (d) setDetail(d); }).catch(() => {});
  }, [detail]);
  usePolling(refreshDetail, 5_000, !!detail);
  useEffect(() => onProxyLogUpdated(() => { refreshDetail(); }, 1000), [refreshDetail]);

  const clearFilter = () => {
    setFilterType("all");
    setFilterPlatform("");
    setFilterProvider("");
    setFilterStatus("");
    setFilterTime("all");
  };

  const platformMap = useMemo(() => {
    const m = new Map<number, string>();
    platforms.forEach(p => m.set(p.id, p.name));
    return m;
  }, [platforms]);

  // ponytail: RequestLog 无 group 维度（test/quota 走 cli-proxy 不经 group 路由）→ groupName 直返 group_key。
  const groupName = (k: string) => k || "-";

  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const currentPage = Math.floor(offset / pageSize) + 1;

  // DetailPanel 复用：传结构兼容 LogsData 子集（DetailPanel 仅读这几个字段）
  const detailData = useMemo(() => ({
    t, detail, copied, copiedId, setCopiedId,
    openDetail, copyDetail, platformMap, groupName,
    setDetail,
  }), [t, detail, copied, copiedId, openDetail, copyDetail, platformMap]);

  if (detail) return <DetailPanel d={detailData as any} />;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.requestLog", "请求日志（测试 / 余额）")}</div>
          <div className="section-desc">
            {total > 0 ? `${total} ${t("logs.total", "条记录")}` : t("requestLog.empty", "暂无请求记录")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <button className="btn" onClick={() => load()} disabled={loading} style={{ fontSize: F.hint }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
          </button>
        </div>
      </div>

      {/* ── Filter bar ── */}
      <div className="glass-surface" style={{ padding: "12px 16px", display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" }}>
        {/* Type: all / test / quota */}
        <FilterSelect
          value={filterType}
          onChange={v => setFilterType(v as TypeFilter)}
          options={[
            { value: "test", label: t("requestLog.typeTest", "测试") },
            { value: "quota", label: t("requestLog.typeQuota", "余额") },
          ]}
          placeholder={t("requestLog.filterType", "类型")}
        />
        {/* Provider */}
        <FilterSelect
          value={filterProvider}
          onChange={setFilterProvider}
          options={providers.map(p => ({ value: String(p.id), label: p.name }))}
          placeholder={t("requestLog.filterProvider", "Provider")}
        />
        {/* Platform */}
        <FilterSelect
          value={filterPlatform}
          onChange={setFilterPlatform}
          options={platforms.map(p => ({ value: String(p.id), label: p.name }))}
          placeholder={t("logs.filterPlatform", "平台")}
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
        {/* Time */}
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
        {hasFilter && (
          <button className="btn btn-ghost" onClick={clearFilter} style={{ fontSize: F.small, padding: "2px 8px", color: "var(--text-tertiary)" }}>
            {t("logs.clearFilter", "清除")}
          </button>
        )}
      </div>

      {/* Table */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : logs.length === 0 ? (
        <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
          <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("requestLog.empty", "暂无请求记录")}</div>
        </div>
      ) : (
        <>
          <div className="glass-surface" style={{ overflow: "auto", contain: "paint" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: F.hint }}>
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border)" }}>
                  <ThCell>{t("logs.time")}</ThCell>
                  <ThCell>{t("logs.group")}</ThCell>
                  <ThCell>{t("logs.platform", "平台")}</ThCell>
                  <ThCell>{t("requestLog.colProvider", "Provider")}</ThCell>
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
                    providerName={log.cli_proxy_provider_name ?? null}
                    onOpen={openDetail}
                    onCopy={copyRow}
                    t={t}
                  />
                ))}
              </tbody>
            </table>
          </div>
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
