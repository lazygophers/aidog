import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  statsApi,
  groupDetailApi,
  platformApi,
  onProxyLogUpdated,
  type StatsResult,
  type StatsQuery,
  type StatsOverview,
  type GroupDetail,
  type Platform,
} from "../services/api";
import { formatNumber, formatCost, successRate } from "../utils/formatters";
import {
  successRateLevel,
  costLevel,
  levelColor,
  type ColorLevel,
} from "../components/shared";

const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;

type TimePreset = "today" | "7d" | "30d";

function getTimeRange(preset: TimePreset): { start: number; end: number } {
  const now = new Date();
  const ms = (d: Date) => d.getTime();
  switch (preset) {
    case "today": {
      const s = new Date(now); s.setHours(0, 0, 0, 0);
      return { start: ms(s), end: ms(now) };
    }
    case "7d": {
      const s = new Date(now); s.setDate(s.getDate() - 7);
      return { start: ms(s), end: ms(now) };
    }
    case "30d": {
      const s = new Date(now); s.setDate(s.getDate() - 30);
      return { start: ms(s), end: ms(now) };
    }
  }
}

// ── 上一等长周期：把当前 [start,end] 整体往前平移一个窗口长度 ──
function previousRange(start: number, end: number): { start: number; end: number } {
  const span = end - start;
  return { start: start - span, end: start };
}

// ── 环比增减：(cur - prev) / prev，prev 为 0 时返回 null（无对比基准） ──
function delta(cur: number, prev: number): number | null {
  if (!(prev > 0)) return null;
  return ((cur - prev) / prev) * 100;
}

// ── 小箭头 SVG（环比方向 / 排序指示），就地渲染避免改动 shared icons ──
function ArrowUp({ size = 11, color = "currentColor" }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth={3}
      strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
      <path d="M12 19V5M5 12l7-7 7 7" />
    </svg>
  );
}
function ArrowDown({ size = 11, color = "currentColor" }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth={3}
      strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
      <path d="M12 5v14M19 12l-7 7-7-7" />
    </svg>
  );
}

// ── 排序 ──
type SortKey =
  | "name" | "total_requests" | "success_count" | "input_tokens"
  | "output_tokens" | "cache_tokens" | "avg_duration_ms" | "total_cost";
type SortDir = "asc" | "desc";

const PAGE_SIZE = 50;

export function Stats() {
  const { t } = useTranslation();
  const [data, setData] = useState<StatsResult | null>(null);
  const [prevOverview, setPrevOverview] = useState<StatsOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [preset, setPreset] = useState<TimePreset>("today");
  const [granularity, setGranularity] = useState<"daily" | "hourly">("hourly");
  // 切 preset 联动粒度：today→hourly（24 点），7d/30d→daily；手动 select 仍可覆盖
  const changePreset = (p: TimePreset) => {
    setPreset(p);
    setGranularity(p === "today" ? "hourly" : "daily");
  };
  const [groupBy, setGroupBy] = useState<"platform" | "model" | "group">("platform");
  const [filterGroup, setFilterGroup] = useState("");
  const [filterModel, setFilterModel] = useState("");
  const [filterPlatform, setFilterPlatform] = useState("");
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);

  // 维度表排序 / 分页
  const [sortKey, setSortKey] = useState<SortKey>("total_requests");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [page, setPage] = useState(0);

  // 趋势图 hover
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);
  const chartRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const range = getTimeRange(preset);
      const base: Omit<StatsQuery, "start" | "end"> = {
        granularity,
        group_by: groupBy,
        filter_group: filterGroup || undefined,
        filter_model: filterModel || undefined,
        filter_platform: filterPlatform || undefined,
      };
      const prevR = previousRange(range.start, range.end);
      // 当前周期 + 上一等长周期并行查询（上一周期仅用 overview 做环比）
      const [result, prev] = await Promise.all([
        statsApi.query({ ...base, start: range.start, end: range.end }),
        statsApi.query({ ...base, start: prevR.start, end: prevR.end }).catch(() => null),
      ]);
      setData(result);
      setPrevOverview(prev?.overview ?? null);
    } catch (e) {
      console.error(e);
    }
    setLoading(false);
  }, [preset, granularity, groupBy, filterGroup, filterModel, filterPlatform]);

  useEffect(() => { load(); }, [load]);

  // 请求完成后后端 emit "proxy-log-updated" → debounce 重载（依赖 load，刷新尊重当前时间范围/筛选）
  useEffect(() => onProxyLogUpdated(() => { load(); }), [load]);

  // Load filter options
  const loadFilterOptions = useCallback(() => {
    groupDetailApi.list().then(setGroups).catch(() => {});
    platformApi.list().then(setPlatforms).catch(() => {});
  }, []);

  useEffect(() => { loadFilterOptions(); }, [loadFilterOptions]);

  // 请求完成后同步刷新筛选选项（平台/分组列表可能已变）
  useEffect(() => onProxyLogUpdated(() => { loadFilterOptions(); }), [loadFilterOptions]);

  // 维度 / 筛选变化时重置分页
  useEffect(() => { setPage(0); }, [groupBy, filterGroup, filterModel, filterPlatform, preset]);

  // Collect unique models from groups (model_mappings) + platform available_models
  const allModels = useMemo(
    () => Array.from(new Set([
      ...groups.flatMap(g => g.model_mappings.map(m => m.target_model)),
      ...platforms.flatMap(p => p.available_models || []),
    ])).sort(),
    [groups, platforms],
  );
  const allPlatforms = useMemo(
    () => platforms.map(p => ({ value: String(p.id), label: p.name })),
    [platforms],
  );

  const overview = data?.overview;
  const buckets = data?.buckets ?? [];
  const dims = data?.dimension_data ?? [];

  // Chart scaling
  const maxReq = Math.max(1, ...buckets.map(b => b.total_requests));

  // 维度表排序结果
  const sortedDims = useMemo(() => {
    const arr = [...dims];
    arr.sort((a, b) => {
      let av: number | string;
      let bv: number | string;
      if (sortKey === "name") { av = a.name; bv = b.name; }
      else { av = a[sortKey]; bv = b[sortKey]; }
      let cmp: number;
      if (typeof av === "string" && typeof bv === "string") cmp = av.localeCompare(bv);
      else cmp = (av as number) - (bv as number);
      return sortDir === "asc" ? cmp : -cmp;
    });
    return arr;
  }, [dims, sortKey, sortDir]);

  const pageCount = Math.max(1, Math.ceil(sortedDims.length / PAGE_SIZE));
  const safePage = Math.min(page, pageCount - 1);
  const pagedDims = sortedDims.slice(safePage * PAGE_SIZE, safePage * PAGE_SIZE + PAGE_SIZE);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir(d => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir(key === "name" ? "asc" : "desc");
    }
    setPage(0);
  };

  const curSuccessRate = overview
    ? (overview.success_rate ?? 0)
    : 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {/* Header */}
      <div>
        <div className="section-title">{t("page.stats", "使用统计")}</div>
        <div className="section-desc">{t("stats.desc", "按平台、分组、模型维度查看请求量与 Token 消耗趋势")}</div>
      </div>

      {/* Filters */}
      <div className="glass-surface" style={{ padding: "14px 20px", display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
        {/* Time preset */}
        <div style={{ display: "flex", gap: 4 }}>
          {(["today", "7d", "30d"] as TimePreset[]).map(p => (
            <button
              key={p}
              className={preset === p ? "btn-active" : "btn"}
              style={{ fontSize: 12, padding: "4px 10px" }}
              onClick={() => changePreset(p)}
            >
              {t(`stats.${p}`, p === "today" ? "今天" : p === "7d" ? "近 7 天" : "近 30 天")}
            </button>
          ))}
        </div>

        {/* Granularity */}
        <select className="input" style={{ fontSize: 12, width: 80 }} value={granularity} onChange={e => setGranularity(e.target.value as "daily" | "hourly")}>
          <option value="daily">{t("stats.daily", "按天")}</option>
          <option value="hourly">{t("stats.hourly", "按小时")}</option>
        </select>

        {/* Group by */}
        <select className="input" style={{ fontSize: 12, width: 100 }} value={groupBy} onChange={e => setGroupBy(e.target.value as "platform" | "model" | "group")}>
          <option value="platform">{t("stats.byPlatform", "按平台")}</option>
          <option value="model">{t("stats.byModel", "按模型")}</option>
          <option value="group">{t("stats.byGroup", "按分组")}</option>
        </select>

        {/* Filter: group（带搜索） */}
        <SearchableFilter
          width={140}
          value={filterGroup}
          onChange={setFilterGroup}
          allLabel={t("stats.allGroups", "全部分组")}
          searchPlaceholder={t("stats.searchGroup", "搜索分组...")}
          options={groups.map(g => ({ value: g.group.name, label: g.group.name }))}
          emptyLabel={t("stats.noMatch", "无匹配")}
        />

        {/* Filter: model（带搜索，列表可能很长） */}
        <SearchableFilter
          width={170}
          value={filterModel}
          onChange={setFilterModel}
          allLabel={t("stats.allModels", "全部模型")}
          searchPlaceholder={t("stats.searchModel", "搜索模型...")}
          options={allModels.map(m => ({ value: m, label: m }))}
          emptyLabel={t("stats.noMatch", "无匹配")}
        />

        {/* Filter: protocol（带搜索） */}
        <SearchableFilter
          width={140}
          value={filterPlatform}
          onChange={setFilterPlatform}
          allLabel={t("stats.allPlatforms", "全部平台")}
          searchPlaceholder={t("stats.searchPlatform", "搜索平台...")}
          options={allPlatforms}
          emptyLabel={t("stats.noMatch", "无匹配")}
        />
      </div>

      {loading && !data ? (
        <div style={{ textAlign: "center", padding: 40, color: "var(--text-secondary)", fontSize: F.hint }}>
          {t("stats.loading", "加载中...")}
        </div>
      ) : overview ? (
        <>
          {/* Overview cards（含色编码 + 环比对比） */}
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
            <OverviewCard
              label={t("stats.totalRequests", "总请求")}
              value={formatNumber(overview.total_requests)}
              delta={delta(overview.total_requests, prevOverview?.total_requests ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.successRate", "成功率")}
              value={curSuccessRate.toFixed(1)}
              unit="%"
              level={successRateLevel(curSuccessRate, overview.total_requests)}
              delta={delta(curSuccessRate, prevOverview?.success_rate ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.inputTokens", "输入 Token")}
              value={formatNumber(overview.total_input_tokens)}
              delta={delta(overview.total_input_tokens, prevOverview?.total_input_tokens ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.outputTokens", "输出 Token")}
              value={formatNumber(overview.total_output_tokens)}
              delta={delta(overview.total_output_tokens, prevOverview?.total_output_tokens ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.cacheTokens", "缓存 Token")}
              value={formatNumber(overview.total_cache_tokens)}
              delta={delta(overview.total_cache_tokens, prevOverview?.total_cache_tokens ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.cacheRate", "缓存率")}
              value={overview.cache_rate.toFixed(1)}
              unit="%"
              delta={delta(overview.cache_rate, prevOverview?.cache_rate ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.avgLatency", "平均延迟")}
              value={overview.avg_duration_ms.toFixed(0)}
              unit="ms"
              delta={delta(overview.avg_duration_ms, prevOverview?.avg_duration_ms ?? 0)}
              deltaInverse
              t={t}
            />
            <OverviewCard
              label={t("stats.totalCost", "预估成本")}
              value={"$" + formatCost(overview.total_cost)}
              level={costLevel(overview.total_cost)}
              delta={delta(overview.total_cost, prevOverview?.total_cost ?? 0)}
              deltaInverse
              t={t}
            />
          </div>

          {/* Trend chart（hover tooltip） */}
          {buckets.length > 0 && (
            <div className="glass-surface" style={{ padding: "16px 20px" }}>
              <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 12 }}>{t("stats.requestTrend", "请求趋势")}</div>
              <div
                ref={chartRef}
                style={{ position: "relative", display: "flex", alignItems: "flex-end", gap: 2, height: 120 }}
                onMouseLeave={() => setHoverIdx(null)}
              >
                {buckets.map((b, i) => {
                  const h = Math.max(2, (b.total_requests / maxReq) * 100);
                  const errRatio = b.total_requests > 0 ? b.error_count / b.total_requests : 0;
                  const active = hoverIdx === i;
                  return (
                    <div
                      key={i}
                      style={{
                        flex: 1,
                        minWidth: 0,
                        display: "flex",
                        flexDirection: "column",
                        alignItems: "center",
                        gap: 2,
                        cursor: "default",
                      }}
                      onMouseEnter={() => setHoverIdx(i)}
                    >
                      <div style={{ fontSize: 9, color: "var(--text-tertiary)" }}>{b.total_requests > 0 ? formatNumber(b.total_requests) : ""}</div>
                      <div style={{
                        width: "100%",
                        height: `${h}%`,
                        borderRadius: 3,
                        background: `linear-gradient(to top, var(--accent), color-mix(in srgb, var(--accent) ${Math.round(errRatio * 100)}%, var(--danger)))`,
                        opacity: active ? 1 : 0.8,
                        outline: active ? "1px solid var(--accent)" : "none",
                        transition: "height 0.3s, opacity 0.15s",
                      }} />
                      {buckets.length <= 24 && (
                        <div style={{ fontSize: 8, color: "var(--text-tertiary)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", maxWidth: "100%" }}>
                          {b.time_bucket.slice(-5)}
                        </div>
                      )}
                    </div>
                  );
                })}

                {/* hover tooltip（绝对定位 glass-elevated） */}
                {hoverIdx !== null && buckets[hoverIdx] && (
                  <ChartTooltip
                    bucket={buckets[hoverIdx]}
                    pos={hoverIdx / Math.max(1, buckets.length - 1)}
                    t={t}
                  />
                )}
              </div>
            </div>
          )}

          {/* Dimension table（列排序 + 分页） */}
          {dims.length > 0 ? (
            <div className="glass-surface" style={{ padding: "16px 20px" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12, flexWrap: "wrap", gap: 8 }}>
                <div style={{ fontSize: F.label, fontWeight: 600 }}>
                  {t("stats.dimensionRank", "维度排行")} — {t(`stats.by${groupBy.charAt(0).toUpperCase() + groupBy.slice(1)}`, groupBy)}
                  <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginLeft: 8 }}>
                    {t("stats.totalRows", "共 {{count}} 条", { count: sortedDims.length })}
                  </span>
                </div>
                {pageCount > 1 && (
                  <Pager
                    page={safePage}
                    pageCount={pageCount}
                    onPrev={() => setPage(p => Math.max(0, p - 1))}
                    onNext={() => setPage(p => Math.min(pageCount - 1, p + 1))}
                    t={t}
                  />
                )}
              </div>
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: F.hint }}>
                <thead>
                  <tr style={{ borderBottom: "1px solid var(--border)" }}>
                    <SortableTh label={t("stats.dimName", "名称")} col="name" align="left" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.requests", "请求")} col="total_requests" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.success", "成功")} col="success_count" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.inputTokens", "输入")} col="input_tokens" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.outputTokens", "输出")} col="output_tokens" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.cacheTokens", "缓存")} col="cache_tokens" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.avgMs", "平均延迟")} col="avg_duration_ms" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.totalCost", "预估成本")} col="total_cost" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                  </tr>
                </thead>
                <tbody>
                  {pagedDims.map((d, i) => {
                    const rate = successRate(d.success_count, d.total_requests);
                    return (
                      <tr key={safePage * PAGE_SIZE + i} style={{ borderBottom: "1px solid var(--border)", opacity: 0.9 }}>
                        <td style={{ padding: "6px 8px", fontWeight: 500, maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{d.name}</td>
                        <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.total_requests)}</td>
                        <td style={{ textAlign: "right", padding: "6px 8px", color: levelColor(successRateLevel(rate, d.total_requests)) }}>{formatNumber(d.success_count)}</td>
                        <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.input_tokens)}</td>
                        <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.output_tokens)}</td>
                        <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.cache_tokens)}</td>
                        <td style={{ textAlign: "right", padding: "6px 8px" }}>{d.avg_duration_ms.toFixed(0)} ms</td>
                        <td style={{ textAlign: "right", padding: "6px 8px", color: levelColor(costLevel(d.total_cost)) }}>${formatCost(d.total_cost)}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : null}
        </>
      ) : (
        <div style={{ textAlign: "center", padding: 40, color: "var(--text-secondary)", fontSize: F.hint }}>
          {t("stats.noData", "暂无统计数据")}
        </div>
      )}
    </div>
  );
}

// ── Overview 卡片：值 + 可选单位 + 色编码 + 环比 delta ──
interface OverviewCardProps {
  label: string;
  value: string;
  unit?: string;
  level?: ColorLevel;
  /** 环比百分比，null 表示无对比基准（隐藏 delta，遵「无数据隐藏」约定）。 */
  delta: number | null;
  /** 反向指标（成本 / 延迟）：上升为「差」，箭头着 danger 色。 */
  deltaInverse?: boolean;
  t: TFunction;
}

function OverviewCard({ label, value, unit, level, delta, deltaInverse, t }: OverviewCardProps) {
  const valueColor = level ? levelColor(level) : "var(--text-primary)";
  let deltaNode = null;
  if (delta !== null && Math.abs(delta) >= 0.05) {
    const up = delta > 0;
    // 正常指标：上升=好(success)；反向指标：上升=差(danger)
    const good = deltaInverse ? !up : up;
    const color = good ? "var(--color-success)" : "var(--color-danger)";
    deltaNode = (
      <div style={{ display: "flex", alignItems: "center", gap: 2, fontSize: F.small, color, fontWeight: 600 }}
        title={t("stats.vsPrevPeriod", "对比上一周期")}>
        {up ? <ArrowUp color={color} /> : <ArrowDown color={color} />}
        {Math.abs(delta).toFixed(1)}%
      </div>
    );
  }
  return (
    <div className="glass-surface" style={{ flex: "1 1 120px", padding: "16px 20px", display: "flex", flexDirection: "column", gap: 4 }}>
      <div style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{label}</div>
      <div style={{ fontSize: F.title, fontWeight: 700, color: valueColor }}>
        {value}{unit && <span style={{ fontSize: F.label, fontWeight: 400, marginLeft: 2 }}>{unit}</span>}
      </div>
      {deltaNode}
    </div>
  );
}

// ── 趋势图 tooltip ──
interface ChartTooltipProps {
  bucket: {
    time_bucket: string;
    total_requests: number;
    success_count: number;
    error_count: number;
    total_cost: number;
    avg_duration_ms: number;
  };
  /** 0–1，桶在 x 轴的相对位置，用于左右对齐避免溢出。 */
  pos: number;
  t: TFunction;
}

function ChartTooltip({ bucket, pos, t }: ChartTooltipProps) {
  const rate = successRate(bucket.success_count, bucket.total_requests);
  const left = pos <= 0.5;
  return (
    <div
      className="glass-elevated"
      style={{
        position: "absolute",
        top: 0,
        [left ? "left" : "right"]: `${left ? pos * 100 : (1 - pos) * 100}%`,
        transform: left ? "translateX(8px)" : "translateX(-8px)",
        padding: "8px 12px",
        borderRadius: "var(--radius-sm)",
        pointerEvents: "none",
        zIndex: 10,
        minWidth: 150,
        display: "flex",
        flexDirection: "column",
        gap: 3,
        fontSize: F.small,
      }}
    >
      <div style={{ fontWeight: 700, marginBottom: 2 }}>{bucket.time_bucket}</div>
      <Row label={t("stats.requests", "请求")} value={formatNumber(bucket.total_requests)} />
      <Row label={t("stats.success", "成功")} value={formatNumber(bucket.success_count)}
        color={levelColor(successRateLevel(rate, bucket.total_requests))} />
      <Row label={t("stats.errors", "失败")} value={formatNumber(bucket.error_count)}
        color={bucket.error_count > 0 ? "var(--color-danger)" : undefined} />
      <Row label={t("stats.avgMs", "平均延迟")} value={`${bucket.avg_duration_ms.toFixed(0)} ms`} />
      <Row label={t("stats.totalCost", "预估成本")} value={"$" + formatCost(bucket.total_cost)} />
    </div>
  );
}

function Row({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", gap: 16 }}>
      <span style={{ color: "var(--text-secondary)" }}>{label}</span>
      <span style={{ fontWeight: 600, color: color ?? "var(--text-primary)" }}>{value}</span>
    </div>
  );
}

// ── 可排序表头 ──
interface SortableThProps {
  label: string;
  col: SortKey;
  align?: "left" | "right";
  sortKey: SortKey;
  sortDir: SortDir;
  onSort: (k: SortKey) => void;
}

function SortableTh({ label, col, align = "right", sortKey, sortDir, onSort }: SortableThProps) {
  const active = sortKey === col;
  return (
    <th
      onClick={() => onSort(col)}
      style={{
        textAlign: align,
        padding: "6px 8px",
        fontWeight: 600,
        cursor: "pointer",
        userSelect: "none",
        color: active ? "var(--accent)" : "var(--text-primary)",
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ display: "inline-flex", alignItems: "center", gap: 3, flexDirection: align === "left" ? "row" : "row-reverse" }}>
        {label}
        {active && (sortDir === "asc" ? <ArrowUp color="var(--accent)" /> : <ArrowDown color="var(--accent)" />)}
      </span>
    </th>
  );
}

// ── 分页器 ──
interface PagerProps {
  page: number;
  pageCount: number;
  onPrev: () => void;
  onNext: () => void;
  t: TFunction;
}

function Pager({ page, pageCount, onPrev, onNext, t }: PagerProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: F.small }}>
      <button className="btn" style={{ fontSize: 12, padding: "3px 10px" }} disabled={page <= 0} onClick={onPrev}>
        {t("stats.prevPage", "上一页")}
      </button>
      <span style={{ color: "var(--text-secondary)" }}>
        {t("stats.pageOf", "{{page}} / {{total}}", { page: page + 1, total: pageCount })}
      </span>
      <button className="btn" style={{ fontSize: 12, padding: "3px 10px" }} disabled={page >= pageCount - 1} onClick={onNext}>
        {t("stats.nextPage", "下一页")}
      </button>
    </div>
  );
}

// ── 带搜索的筛选下拉 ──
interface SearchableFilterProps {
  width: number;
  value: string;
  onChange: (v: string) => void;
  allLabel: string;
  searchPlaceholder: string;
  options: Array<{ value: string; label: string }>;
  emptyLabel: string;
}

function SearchableFilter({ width, value, onChange, allLabel, searchPlaceholder, options, emptyLabel }: SearchableFilterProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return options;
    return options.filter(o => o.label.toLowerCase().includes(q));
  }, [options, search]);

  const current = options.find(o => o.value === value);

  return (
    <div ref={ref} style={{ position: "relative", width }}>
      <button
        className="input"
        style={{ fontSize: 12, width: "100%", textAlign: "left", cursor: "pointer", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        onClick={() => setOpen(o => !o)}
      >
        {current ? current.label : allLabel}
      </button>
      {open && (
        <div
          className="glass-elevated"
          style={{
            position: "absolute",
            top: "calc(100% + 4px)",
            left: 0,
            width: Math.max(width, 320),
            zIndex: 20,
            borderRadius: "var(--radius-sm)",
            padding: 8,
            display: "flex",
            flexDirection: "column",
            gap: 6,
            maxHeight: 320,
          }}
        >
          <input
            className="input"
            autoFocus
            style={{ fontSize: 14 }}
            placeholder={searchPlaceholder}
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
          <div style={{ overflowY: "auto", maxHeight: 250, display: "flex", flexDirection: "column", gap: 2 }}>
            <FilterOption label={allLabel} active={value === ""} onClick={() => { onChange(""); setOpen(false); setSearch(""); }} />
            {filtered.length === 0 ? (
              <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "6px 8px" }}>{emptyLabel}</div>
            ) : (
              filtered.map(o => (
                <FilterOption
                  key={o.value}
                  label={o.label}
                  active={value === o.value}
                  onClick={() => { onChange(o.value); setOpen(false); setSearch(""); }}
                />
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function FilterOption({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "block",
        width: "100%",
        textAlign: "left",
        border: "none",
        background: active ? "var(--bg-glass)" : "transparent",
        color: active ? "var(--accent)" : "var(--text-primary)",
        fontWeight: active ? 600 : 400,
        padding: "11px 14px",
        borderRadius: "var(--radius-sm)",
        cursor: "pointer",
        fontFamily: "inherit",
        fontSize: 15,
        lineHeight: 1.5,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </button>
  );
}
