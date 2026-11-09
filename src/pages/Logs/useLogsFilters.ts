import { useState, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  proxyLogApi,
  platformApi,
  groupDetailApi,
  type ProxyLogFilter,
  type Platform,
  type GroupDetail,
} from "../../services/api";
import { timePresetToRange, NO_GROUP_SENTINEL, type TimePreset } from "./types";

/**
 * Logs 页筛选态：平台/分组下拉数据源 + 各筛选字段 + 派生的 activeFilter/hasFilter/modelOptions +
 * platformMap/groupName（platform/group 展示名映射，list 与 detail 两侧共用）。
 * 自 useLogsData 拆出（c8 三段切分），行为不变。
 */
export function useLogsFilters(initialFilter?: { platformId?: number; groupKey?: string; sessionKey?: string }) {
  const { t } = useTranslation();

  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [filterPlatform, setFilterPlatform] = useState<string>(initialFilter?.platformId ? String(initialFilter.platformId) : "");
  const [filterGroup, setFilterGroup] = useState<string>(initialFilter?.groupKey ?? "");
  // 会话归组键（magpie 票 09 会话统计下钻）：仅导航入口带入，无独立下拉
  const [filterSession, setFilterSession] = useState<string>(initialFilter?.sessionKey ?? "");
  const [filterStatus, setFilterStatus] = useState<string>("");
  const [filterTime, setFilterTime] = useState<TimePreset>("all");
  const [filterModel, setFilterModel] = useState<string>("");
  const [filterActualModel, setFilterActualModel] = useState<string>("");
  const [filterPath, setFilterPath] = useState<string>("");
  // 中间件观察模式命中（票 04）："" = 全部；"observed" = 仅 blocked_reason='observe' 的行
  const [filterObserved, setFilterObserved] = useState<string>("");
  // 日志类型（票 02）："" = 全部（含隧道连接）；"request" = 仅 AI 请求；"tunnel" = 仅隧道连接
  const [filterType, setFilterType] = useState<string>("");

  useEffect(() => {
    platformApi.list().then(setPlatforms).catch(() => {});
    groupDetailApi.list().then(setGroups).catch(() => {});
  }, []);

  const activeFilter: ProxyLogFilter = useMemo(() => {
    // ponytail: Logs 主页默认排除 test/quota 两类（已迁到 RequestLog 新页），
    // 徽章链 get_last_test_result 是独立 query，不经 list_proxy_logs，不受此影响。
    const f: ProxyLogFilter = { exclude_sources: ["test", "quota", "fetch-models"] };
    if (filterPlatform) f.platform_id = Number(filterPlatform);
    if (filterGroup) f.group_key = filterGroup === NO_GROUP_SENTINEL ? "" : filterGroup;
    if (filterStatus === "success") f.status = 200;
    else if (filterStatus === "error") f.status = -1;
    const tr = timePresetToRange(filterTime);
    if (tr.start) f.time_start = tr.start;
    if (tr.end) f.time_end = tr.end;
    if (filterModel.trim()) f.model = filterModel.trim();
    if (filterActualModel.trim()) f.actual_model = filterActualModel.trim();
    if (filterPath.trim()) f.path = filterPath.trim();
    if (filterObserved === "observed") f.observed = true;
    if (filterType === "tunnel") f.sources = ["http-connect"];
    else if (filterType === "request") f.exclude_sources = [...f.exclude_sources!, "http-connect"];
    if (filterSession.trim()) f.session_key = filterSession.trim();
    return f;
  }, [filterPlatform, filterGroup, filterStatus, filterTime, filterModel, filterActualModel, filterPath, filterObserved, filterType, filterSession]);

  const hasFilter = !!(filterPlatform || filterGroup || filterStatus || filterTime !== "all" || filterModel.trim() || filterActualModel.trim() || filterPath.trim() || filterObserved || filterType || filterSession.trim());

  const [modelOptions, setModelOptions] = useState<string[]>([]);
  const [actualModelOptions, setActualModelOptions] = useState<string[]>([]);
  useEffect(() => {
    // 模型下拉同样排除 test/quota，避免列出主列表不存在的模型。
    // 后端 DISTINCT 直出选项（logs-query-ipc-slimming s4），请求模型 / 实际模型各取各列。
    const f: ProxyLogFilter = { exclude_sources: ["test", "quota", "fetch-models"] };
    proxyLogApi.distinctModels(f, false, 200).then(setModelOptions).catch(() => {});
    proxyLogApi.distinctModels(f, true, 200).then(setActualModelOptions).catch(() => {});
  }, []);

  const clearFilter = () => {
    setFilterPlatform("");
    setFilterGroup("");
    setFilterStatus("");
    setFilterTime("all");
    setFilterModel("");
    setFilterActualModel("");
    setFilterPath("");
    setFilterObserved("");
    setFilterType("");
    setFilterSession("");
  };

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
    platforms, groups, filterPlatform, filterGroup, filterStatus, filterTime, filterModel, filterActualModel, filterPath, filterObserved, filterType, filterSession,
    setFilterPlatform, setFilterGroup, setFilterStatus, setFilterTime, setFilterModel, setFilterActualModel, setFilterPath, setFilterObserved, setFilterType, setFilterSession,
    activeFilter, hasFilter, clearFilter, modelOptions, actualModelOptions,
    platformMap, groupName,
  };
}

export type LogsFiltersData = ReturnType<typeof useLogsFilters>;
