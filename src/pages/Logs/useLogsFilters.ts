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
export function useLogsFilters(initialFilter?: { platformId?: number; groupKey?: string }) {
  const { t } = useTranslation();

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
    // ponytail: Logs 主页默认排除 test/quota 两类（已迁到 RequestLog 新页），
    // 徽章链 get_last_test_result 是独立 query，不经 list_proxy_logs，不受此影响。
    const f: ProxyLogFilter = { exclude_sources: ["test", "quota"] };
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
        // 模型下拉同样排除 test/quota，避免列出主列表不存在的模型
        const { items } = await proxyLogApi.listFiltered({ exclude_sources: ["test", "quota"] }, 200, 0);
        const col = filterModelType === "actual" ? "actual_model" : "model";
        const set = new Set<string>();
        (items || []).forEach(l => { if ((l as any)[col]) set.add((l as any)[col]); });
        setModelOptions(Array.from(set).sort());
      } catch { /* ignore */ }
    })();
  }, [filterModelType]);

  const clearFilter = () => {
    setFilterPlatform("");
    setFilterGroup("");
    setFilterStatus("");
    setFilterTime("all");
    setFilterModelText("");
    setFilterModelType("actual");
    setFilterPath("");
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
    platforms, groups, filterPlatform, filterGroup, filterStatus, filterTime, filterModelType, filterModelText, filterPath,
    setFilterPlatform, setFilterGroup, setFilterStatus, setFilterTime, setFilterModelType, setFilterModelText, setFilterPath,
    activeFilter, hasFilter, clearFilter, modelOptions,
    platformMap, groupName,
  };
}

export type LogsFiltersData = ReturnType<typeof useLogsFilters>;
