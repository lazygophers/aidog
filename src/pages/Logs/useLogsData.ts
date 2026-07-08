import { useState, useEffect, useCallback, useMemo } from "react";
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
} from "../../services/api";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { usePolling } from "../../hooks/usePolling";
import { timePresetToRange, NO_GROUP_SENTINEL, type TimePreset } from "./types";

const DEFAULT_PAGE_SIZE = 20;

/**
 * Logs 页全部 state + data actions（自原 Logs.tsx L43-263 外迁）。
 * 18 useState + 数据加载/轮询/事件订阅/复制/详情，无逻辑变更。
 */
export function useLogsData(initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string }) {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<ProxyLogSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState<number>(DEFAULT_PAGE_SIZE);
  const [loading, setLoading] = useState(true);
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  const [copied, setCopied] = useState(false);
  const [copiedId, setCopiedId] = useState(false);
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [cleanupMessage, setCleanupMessage] = useState<string>("");

  // ── Filter state ──
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [filterPlatform, setFilterPlatform] = useState<string>(initialFilter?.platformId ? String(initialFilter.platformId) : "");
  const [filterGroup, setFilterGroup] = useState<string>(initialFilter?.groupKey ?? "");
  const [filterStatus, setFilterStatus] = useState<string>("");
  const [filterTime, setFilterTime] = useState<TimePreset>("all");
  const [filterModelType, setFilterModelType] = useState<"original" | "actual">("actual");
  const [filterModelText, setFilterModelText] = useState<string>("");
  const [filterPath, setFilterPath] = useState<string>("");

  useEffect(() => {
    platformApi.list().then(setPlatforms).catch(() => {});
    groupDetailApi.list().then(setGroups).catch(() => {});
  }, []);

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

  const hasFilter = !!(filterPlatform || filterGroup || filterStatus || filterTime !== "all" || filterModelText.trim() || filterPath.trim());

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
  useEffect(() => { setOffset(0); }, [hasFilter, activeFilter, pageSize]);

  const refreshList = useCallback(() => { load(true); }, [load]);
  usePolling(refreshList, 30_000, !detail);
  useEffect(() => onProxyLogUpdated(() => { refreshList(); }, 500), [refreshList]);

  const handleClear = async () => {
    try {
      await proxyLogApi.clear();
      setShowClearConfirm(false);
      setOffset(0);
      load();
    } catch (e) { console.error(e); }
  };

  const handleCleanupExpired = async () => {
    try {
      await proxyLogApi.cleanupExpired();
      setOffset(0);
      load();
      setCleanupMessage(t("logs.cleanupExpiredDone", "已清理过期日志"));
      setTimeout(() => setCleanupMessage(""), 3000);
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

  const copyRow = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) await copyDetail(d);
    } catch (err) { console.error(err); }
  }, [copyDetail]);

  const refreshDetail = useCallback(() => {
    if (!detail) return;
    proxyLogApi.get(detail.id)
      .then(d => { if (d) setDetail(d); })
      .catch(() => {});
  }, [detail]);
  usePolling(refreshDetail, 5_000, !!detail);
  useEffect(() => onProxyLogUpdated(() => { refreshDetail(); }, 1000), [refreshDetail]);

  const platformMap = useMemo(() => {
    const m = new Map<number, string>();
    platforms.forEach(p => m.set(p.id, p.name));
    return m;
  }, [platforms]);

  const groupNameMap = useMemo(() => {
    const m = new Map<string, string>();
    groups.forEach(g => m.set(g.group.group_key, g.group.name));
    return m;
  }, [groups]);
  const groupName = (k: string) => (k && groupNameMap.get(k)) || k;

  return {
    t,
    // list state
    logs, total, offset, pageSize, loading, setOffset, setPageSize, load,
    // filter state
    platforms, groups, filterPlatform, filterGroup, filterStatus, filterTime, filterModelType, filterModelText, filterPath,
    setFilterPlatform, setFilterGroup, setFilterStatus, setFilterTime, setFilterModelType, setFilterModelText, setFilterPath,
    modelOptions, hasFilter, clearFilter, handleClear, handleCleanupExpired,
    showClearConfirm, setShowClearConfirm, cleanupMessage,
    // detail state
    detail, setDetail, copied, copiedId, setCopiedId, openDetail, copyDetail, copyRow,
    // maps
    platformMap, groupName,
  };
}

export type LogsData = ReturnType<typeof useLogsData>;
