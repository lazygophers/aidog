import { useState, useEffect, useCallback, useMemo, memo } from "react";
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
  type StatsBucket,
  type StatsSeries,
  type GroupDetail,
  type Platform,
  type QuotaSnapshot,
  type ScatterHistogram,
} from "../services/api";
import { formatNumber, formatCost, formatCostUsd, successRate } from "../utils/formatters";
import { F } from "../domains/shared/tokens";
import { getProtocolSearchTermsMap } from "../domains/platforms/defaults";
import {
  successRateLevel,
  costLevel,
  levelColor,
  FilterDropdown,
  useReveal,
  useCounter,
  type ColorLevel,
} from "../components/shared";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { DonutChart, LineChart, StackedAreaChart, HourHeatmap, DimensionHeatmap, GaugeChart, ScatterChart, bucketMs, type GaugeTrendPoint } from "@/components/charts";
import type { ChartConfig } from "@/components/ui/chart";

// 桶解析已收编公共层（charts/ticks.ts）；re-export 维持 Stats.test.ts 既有导入路径。
export { bucketMs };

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

// ── 主图区四 tab（spec C1）：时间序列 / 占比 / 密度 / 配额 ──
type StatsTab = "trend" | "share" | "density" | "quota";
const TABS: { id: StatsTab; key: string }[] = [
  { id: "trend", key: "stats.tabTrend" },
  { id: "share", key: "stats.tabShare" },
  { id: "density", key: "stats.tabDensity" },
  { id: "quota", key: "stats.tabQuota" },
];

// time_bucket 串 → ms 见公共层 bucketMs（charts/ticks.ts，本文件顶部 re-export）。

export interface TrendChartData {
  config: ChartConfig;
  rows: Record<string, unknown>[];
  multi: boolean;
}

const reqSum = (bs: StatsBucket[]) => bs.reduce((s, b) => s + b.total_requests, 0);

// 时间序列 tab 数据：series_by 多序列 → 合并宽表（键 s0.. 安全 CSS 变量名，按总量降序 → 主琥珀线 = 最大维度值）；
// series 空 / 单序列（未传 series_by 的旧后端、单维度）→ buckets 总量单序列。
export function buildTrendChartData(
  buckets: StatsBucket[],
  series: StatsSeries[],
  singleLabel: string,
): TrendChartData {
  if (series.length > 1) {
    const ordered = [...series].sort((a, b) => reqSum(b.buckets) - reqSum(a.buckets));
    const rowMap = new Map<string, Record<string, unknown>>();
    ordered.forEach((s, i) => {
      for (const b of s.buckets) {
        let row = rowMap.get(b.time_bucket);
        if (!row) {
          row = { x: bucketMs(b.time_bucket) };
          rowMap.set(b.time_bucket, row);
        }
        row[`s${i}`] = b.total_requests;
      }
    });
    const rows = [...rowMap.values()].sort((a, b) => (a.x as number) - (b.x as number));
    const config: ChartConfig = Object.fromEntries(ordered.map((s, i) => [`s${i}`, { label: s.name }]));
    return { config, rows, multi: true };
  }
  return {
    config: { v: { label: singleLabel } },
    rows: buckets.map((b) => ({ x: bucketMs(b.time_bucket), v: b.total_requests })),
    multi: false,
  };
}

// 密度 tab：hourly/minute 桶 → (星期 getDay 0=周日, 小时) 请求量聚合（HourHeatmap 格式）。
// daily 桶无小时信息不入格（调用方喂 hourly 粒度数据，诚实不摊假）。
export function buildHeatCells(buckets: StatsBucket[]): { day: number; hour: number; value: number }[] {
  const m = new Map<number, number>();
  for (const b of buckets) {
    if (!b.time_bucket.includes(" ")) continue;
    const d = new Date(b.time_bucket.replace(" ", "T"));
    if (Number.isNaN(d.getTime())) continue;
    const k = d.getDay() * 24 + d.getHours();
    m.set(k, (m.get(k) ?? 0) + b.total_requests);
  }
  return [...m].map(([k, value]) => ({ day: Math.floor(k / 24), hour: k % 24, value }));
}

// 维度×日格子（占比 tab 维度热力，#39）：series 逐桶按本地日聚合（hourly/minute 桶摊入当日，
// daily 桶本身就是日）。行取总量降序 topN（D3 LIMIT 50 全画则行图过高，热力看头部维度）。
export function buildDimensionDayCells(
  series: StatsSeries[],
  topN = 8,
): { name: string; day: number; value: number }[] {
  const ordered = [...series].sort((a, b) => reqSum(b.buckets) - reqSum(a.buckets)).slice(0, topN);
  return ordered.flatMap((s) => {
    const perDay = new Map<number, number>();
    for (const b of s.buckets) {
      const ms = bucketMs(b.time_bucket);
      if (Number.isNaN(ms)) continue;
      const day = new Date(ms).setHours(0, 0, 0, 0);
      perDay.set(day, (perDay.get(day) ?? 0) + b.total_requests);
    }
    return [...perDay].map(([day, value]) => ({ name: s.name, day, value }));
  });
}

// ── 配额 tab：快照序列 → 每平台仪表盘数据（T10 / #40）──
// current = 该平台最新快照余额；peak = 窗口内峰值余额（快照无总量字段，仪表盘 fraction
// 语义 = 当前 / 峰值）；trend = fraction 序列（GaugeTrendPoint.at 为 Unix 秒）。
// 按当前余额降序（主平台排前）；peak ≤ 0 的组剔除（喂 GaugeChart 也是空态，不如不出卡）。
export interface QuotaGauge {
  platformId: number;
  current: number;
  peak: number;
  trend: GaugeTrendPoint[];
}

export function buildQuotaGauges(snaps: QuotaSnapshot[]): QuotaGauge[] {
  const byPlatform = new Map<number, QuotaSnapshot[]>();
  for (const s of snaps) {
    const arr = byPlatform.get(s.platform_id);
    if (arr) arr.push(s);
    else byPlatform.set(s.platform_id, [s]);
  }
  const gauges: QuotaGauge[] = [];
  for (const [platformId, arr] of byPlatform) {
    // 后端按 created_at 升序返回；仍显式排序一次，纯函数不依赖调用侧约定
    arr.sort((a, b) => a.created_at - b.created_at);
    const peak = Math.max(...arr.map((s) => s.est_balance_remaining));
    if (!(peak > 0)) continue;
    gauges.push({
      platformId,
      current: arr[arr.length - 1].est_balance_remaining,
      peak,
      trend: arr.map((s) => ({ at: Math.floor(s.created_at / 1000), fraction: s.est_balance_remaining / peak })),
    });
  }
  return gauges.sort((a, b) => b.current - a.current);
}

// ── 粒度可读标注（趋势图右上角；auto 降级时加「（自动）」后缀让用户知情） ──
function granLabel(g: StatsQuery["granularity"], auto: boolean, t: TFunction): string {
  const base =
    g === "minute" ? t("stats.granMinute", "1 分钟")
      : g === "5min" ? t("stats.gran5min", "5 分钟")
        : g === "hourly" ? t("stats.granHourly", "小时")
          : t("stats.granDaily", "天");
  return auto ? t("stats.granAuto", "{{g}}（自动）", { g: base }) : base;
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
  | "output_tokens" | "cache_tokens" | "cache_rate" | "avg_duration_ms" | "total_cost";
type SortDir = "asc" | "desc";

const PAGE_SIZE = 50;

export function Stats({ initialFilter }: { initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string } }) {
  const { t } = useTranslation();
  const [data, setData] = useState<StatsResult | null>(null);
  const [prevOverview, setPrevOverview] = useState<StatsOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [preset, setPreset] = useState<TimePreset>("today");
  const [granularity, setGranularity] = useState<"daily" | "hourly">("hourly");
  // 自动降级实际生效的粒度（auto 时 ≠ 用户所选 granularity），用于趋势图渲染 + UI 标注
  const [effectiveGran, setEffectiveGran] = useState<StatsQuery["granularity"]>("hourly");
  // 切 preset 联动粒度：today→hourly（24 点），7d/30d→daily；手动 select 仍可覆盖
  const changePreset = (p: TimePreset) => {
    setPreset(p);
    setGranularity(p === "today" ? "hourly" : "daily");
  };
  const [groupBy, setGroupBy] = useState<"platform" | "model" | "group">("platform");
  const [filterGroup, setFilterGroup] = useState(initialFilter?.groupKey ?? "");
  const [filterModel, setFilterModel] = useState("");
  const [filterPlatform, setFilterPlatform] = useState(initialFilter?.platformId ? String(initialFilter.platformId) : "");
  // 仅 Coding Plan 平台筛选（spec B3）：true → 后端 filter_coding_plan=true
  const [filterCodingPlan, setFilterCodingPlan] = useState(false);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  // 协议搜索词（registry name 全 locale + keywords）：平台筛选下拉跨语言搜索用
  const [protocolTerms, setProtocolTerms] = useState<Partial<Record<string, string[]>>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolSearchTermsMap().then(m => { if (!cancelled) setProtocolTerms(m); }).catch(console.error);
    return () => { cancelled = true; };
  }, []);

  // 维度表排序 / 分页
  const [sortKey, setSortKey] = useState<SortKey>("total_requests");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [page, setPage] = useState(0);

  // 四 tab（spec C1）：本地 state 切主图区，筛选条/Overview 卡/维度表不受影响
  const [activeTab, setActiveTab] = useState<StatsTab>("trend");
  // 时间序列 tab 视图切换（#39 批次二）：多序列时折线 / 堆叠面积两态
  const [trendStacked, setTrendStacked] = useState(false);
  // 密度 tab 按需 hourly 桶（主查询粒度可能是 daily，无小时信息可用）
  const [heatBuckets, setHeatBuckets] = useState<StatsBucket[] | null>(null);
  // 密度 tab 第二视图（T10）：时刻热力 ⇄ 请求散点（spec C1 四 tab 结构不动，视图切主图区内容）
  const [densityView, setDensityView] = useState<"heat" | "scatter">("heat");
  // 散点矩阵（D2 bin 化）与配额快照序列（D4）均按需拉取
  const [scatterHist, setScatterHist] = useState<ScatterHistogram | null>(null);
  const [quotaSnaps, setQuotaSnaps] = useState<QuotaSnapshot[] | null>(null);

  // 「无分组」sentinel 映射：下拉选「无分组」→ filter_group=''（隧道请求 group_key 空）。
  // "0" 在平台筛选 truthy，直接透传后端 CAST AS INTEGER = 0（无平台）。
  const NO_GROUP_SENTINEL = "__none__";
  // 筛选条公共查询体：主查询 / 环比查询 / 密度 hourly 查询共用
  const baseQuery = useCallback(
    (): Omit<StatsQuery, "start" | "end"> => ({
      granularity,
      group_by: groupBy,
      filter_group: filterGroup
        ? (filterGroup === NO_GROUP_SENTINEL ? "" : filterGroup)
        : undefined,
      filter_model: filterModel || undefined,
      filter_platform: filterPlatform || undefined,
      filter_coding_plan: filterCodingPlan || undefined,
    }),
    [granularity, groupBy, filterGroup, filterModel, filterPlatform, filterCodingPlan],
  );

  // silent=true：后台刷新（代理事件触发），不掀 loading。掀了就会把整页内容换成「加载中...」，
  // 图表 / 卡片全部卸载再挂载 —— 每次代理跑完一个请求，4 张图就从零重播一遍生长动画
  // （每帧一次 React 提交），实测一次刷新 55 次提交里 ~40 次出自这一次重挂载（票 11 病灶 A）。
  // silent 分支的写法对齐 RequestLog.tsx 的 load(silent)。
  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      const range = getTimeRange(preset);
      const base = baseQuery();
      const prevR = previousRange(range.start, range.end);
      // 当前周期 + 上一等长周期并行查询（上一周期仅用 overview 做环比）
      const [result, prev] = await Promise.all([
        // series_by=group_by：时间序列 tab 按维度多序列（chart-engine D1 / #32）
        statsApi.query({ ...base, series_by: groupBy, start: range.start, end: range.end }),
        statsApi.query({ ...base, start: prevR.start, end: prevR.end }).catch(() => null),
      ]);
      setPrevOverview(prev?.overview ?? null);

      // ── 自动降级粒度 ──
      // hourly/daily 走聚合表（agg）；minute/5min 走 proxy_log。聚合表稀疏（hourly 非空桶 < 4）时
      // 降级到 proxy_log 的 minute 查询，让短范围走势可读（不再降到 5min，与后端 agg 边界对齐）。
      // 仅在用户选「按小时」且短范围（≤24h，即 today preset）时生效；7d/30d 长范围绝不降到 minute（防桶爆炸）。
      const spanMs = range.end - range.start;
      const H24 = 24 * 60 * 60 * 1000;
      const nonEmpty = result.buckets.filter(b => b.total_requests > 0).length;
      let finalResult = result;
      let finalGran: StatsQuery["granularity"] = granularity;
      if (granularity === "hourly" && spanMs <= H24 && nonEmpty < 4) {
        const r = await statsApi.query({ ...base, granularity: "minute", start: range.start, end: range.end }).catch(() => null);
        if (r) { finalResult = r; finalGran = "minute"; }
      }
      setData(finalResult);
      setEffectiveGran(finalGran);
    } catch (e) {
      console.error(e);
    }
    if (!silent) setLoading(false);
  }, [baseQuery, preset, granularity, groupBy, filterGroup, filterModel, filterPlatform, filterCodingPlan]);

  useEffect(() => { load(); }, [load]);

  // 密度 tab 按需拉 hourly 桶：粒度选择是主查询概念，热力图需要小时信息，固定 hourly（走聚合表）
  useEffect(() => {
    if (activeTab !== "density") return;
    let cancelled = false;
    const range = getTimeRange(preset);
    statsApi
      .query({ ...baseQuery(), granularity: "hourly", start: range.start, end: range.end })
      .then((r) => { if (!cancelled) setHeatBuckets(r.buckets); })
      .catch(console.error);
    return () => { cancelled = true; };
  }, [activeTab, preset, baseQuery]);

  // 密度 tab 散点视图按需拉 bin 化矩阵（D2）：filter 语义同主查询（granularity/group_by 不适用）
  useEffect(() => {
    if (activeTab !== "density" || densityView !== "scatter") return;
    let cancelled = false;
    const range = getTimeRange(preset);
    const f = baseQuery();
    statsApi
      .scatterHistogram({
        start: range.start,
        end: range.end,
        filter_group: f.filter_group,
        filter_model: f.filter_model,
        filter_platform: f.filter_platform,
        filter_coding_plan: f.filter_coding_plan,
      })
      .then((h) => { if (!cancelled) setScatterHist(h); })
      .catch(console.error);
    return () => { cancelled = true; };
  }, [activeTab, densityView, preset, baseQuery]);

  // 配额 tab 按需拉快照序列（D4）：时间窗沿用页面 preset；平台筛选沿用筛选条
  // （「无平台」sentinel 0：quota_snapshot 只记真实平台，直接置空走诚实空态）
  useEffect(() => {
    if (activeTab !== "quota") return;
    if (filterPlatform === "0") { setQuotaSnaps([]); return; }
    let cancelled = false;
    const range = getTimeRange(preset);
    const pid = filterPlatform ? Number(filterPlatform) : null;
    statsApi
      .quotaSnapshots({ start: range.start, end: range.end, platform_id: pid })
      .then((s) => { if (!cancelled) setQuotaSnaps(s); })
      .catch(console.error);
    return () => { cancelled = true; };
  }, [activeTab, preset, filterPlatform]);

  // 请求完成后后端 emit "proxy-log-updated" → debounce 重载（依赖 load，刷新尊重当前时间范围/筛选）。
  // 走 silent 分支：后台刷新不掀 loading，页面内容原地更新而不是卸载重挂。
  useEffect(() => onProxyLogUpdated(() => { load(true); }), [load]);

  // Load filter options
  const loadFilterOptions = useCallback(() => {
    groupDetailApi.list().then(setGroups).catch(() => {});
    platformApi.list().then(setPlatforms).catch(() => {});
  }, []);

  useEffect(() => { loadFilterOptions(); }, [loadFilterOptions]);

  // 注意：筛选选项（分组 / 平台列表）**不订阅** proxy-log-updated。代理跑完一个请求不会
  // 改变分组或平台列表，那两条查询是纯浪费；而独立订阅还会带来第二条 500 ms 防抖，
  // 让一次代理事件触发两轮重查（票 11 病灶 B）。列表变更走用户编辑路径，挂载时拉一次即可。

  // 维度 / 筛选变化时重置分页
  useEffect(() => { setPage(0); }, [groupBy, filterGroup, filterModel, filterPlatform, filterCodingPlan, preset]);

  // 模型筛选项来自实际 proxy_log 记录（后端 available_models），非配置列表
  const allModels = useMemo(
    () => (data?.available_models ?? []).slice().sort(),
    [data],
  );
  const allPlatforms = useMemo(
    () => platforms.map(p => ({ value: String(p.id), label: p.name, searchTerms: protocolTerms[p.platform_type] })),
    [platforms, protocolTerms],
  );

  const overview = data?.overview;
  const buckets = data?.buckets ?? [];
  // 维度名回溯失败兜底：Rust 返空串（与 stats_today 口径一致），此处统一归「未知平台」。
  // 单点归一化，donut / 维度表 / 趋势图例 / 维度热力四消费点全覆盖。
  const unknownPlatform = t("popover.unknownPlatform", "未知平台");
  const dims = (data?.dimension_data ?? []).map(d => (d.name ? d : { ...d, name: unknownPlatform }));
  const series = useMemo(
    () => (data?.series ?? []).map(s => (s.name ? s : { ...s, name: unknownPlatform })),
    [data, unknownPlatform],
  );

  // 时间序列 tab：series_by 多序列（无 series 回落 buckets 单序列）
  const trend = useMemo(
    () => buildTrendChartData(buckets, series, t("stats.requests", "请求")),
    [buckets, series, t],
  );

  // 占比 tab 维度热力（#39）：维度 × 日活跃格子（series 空 → DimensionHeatmap 诚实空态）
  const dimHeat = useMemo(() => buildDimensionDayCells(series), [series]);

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
            <Button
              key={p}
              variant={preset === p ? "default" : "ghost"}
              style={{
                fontSize: 12,
                padding: "4px 10px",
                height: "auto",
                ...(preset === p
                  ? {
                      background: "var(--accent-subtle)",
                      color: "var(--accent)",
                      borderColor: "color-mix(in srgb, var(--accent-edge) 40%, var(--border))",
                    }
                  : {}),
              }}
              onClick={() => changePreset(p)}
            >
              {t(`stats.${p}`, p === "today" ? "今天" : p === "7d" ? "近 7 天" : "近 30 天")}
            </Button>
          ))}
        </div>

        {/* Granularity */}
        <Select value={granularity} onValueChange={v => setGranularity(v as "daily" | "hourly")}>
          <SelectTrigger style={{ fontSize: 12, width: 90, height: 30 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="daily">{t("stats.daily", "按天")}</SelectItem>
            <SelectItem value="hourly">{t("stats.hourly", "按小时")}</SelectItem>
          </SelectContent>
        </Select>

        {/* Group by */}
        <Select value={groupBy} onValueChange={v => setGroupBy(v as "platform" | "model" | "group")}>
          <SelectTrigger style={{ fontSize: 12, width: 110, height: 30 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="platform">{t("stats.byPlatform", "按平台")}</SelectItem>
            <SelectItem value="model">{t("stats.byModel", "按模型")}</SelectItem>
            <SelectItem value="group">{t("stats.byGroup", "按分组")}</SelectItem>
          </SelectContent>
        </Select>

        {/* Filter: group（带搜索） */}
        <FilterDropdown
          width={140}
          value={filterGroup}
          onChange={setFilterGroup}
          allLabel={t("stats.allGroups", "全部分组")}
          searchPlaceholder={t("stats.searchGroup", "搜索分组...")}
          options={[
            ...groups.map(g => ({ value: g.group.group_key, label: g.group.name })),
            // 隧道请求无 apikey → group_key=''（sentinel 映射见 load）
            { value: NO_GROUP_SENTINEL, label: t("stats.noGroup", "无分组") },
          ]}
          emptyLabel={t("stats.noMatch", "无匹配")}
        />

        {/* Filter: model（带搜索，列表可能很长） */}
        <FilterDropdown
          width={170}
          value={filterModel}
          onChange={setFilterModel}
          allLabel={t("stats.allModels", "全部模型")}
          searchPlaceholder={t("stats.searchModel", "搜索模型...")}
          options={allModels.map(m => ({ value: m, label: m }))}
          emptyLabel={t("stats.noMatch", "无匹配")}
        />

        {/* Filter: protocol（带搜索） */}
        <FilterDropdown
          width={140}
          value={filterPlatform}
          onChange={setFilterPlatform}
          allLabel={t("stats.allPlatforms", "全部平台")}
          searchPlaceholder={t("stats.searchPlatform", "搜索平台...")}
          // 追加「无平台」(platform_id=0，隧道请求 host 未命中)
          options={[...allPlatforms, { value: "0", label: t("stats.noPlatform", "无平台") }]}
          emptyLabel={t("stats.noMatch", "无匹配")}
        />
        {/* Filter: coding plan（订阅套餐平台，spec B3） */}
        <Select value={filterCodingPlan ? "coding" : "all"} onValueChange={v => setFilterCodingPlan(v === "coding")}>
          <SelectTrigger style={{ fontSize: 12, width: 130, height: 30 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t("stats.planAll", "全部计费")}</SelectItem>
            <SelectItem value="coding">{t("stats.codingPlanOnly", "仅 Coding Plan")}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      {loading ? (
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
              numericValue={overview.total_requests}
              staggerMs={0}
              delta={delta(overview.total_requests, prevOverview?.total_requests ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.successRate", "成功率")}
              value={curSuccessRate.toFixed(1)}
              unit="%"
              level={successRateLevel(curSuccessRate, overview.total_requests)}
              staggerMs={60}
              delta={delta(curSuccessRate, prevOverview?.success_rate ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.inputTokens", "输入 Token")}
              value={formatNumber(overview.total_input_tokens)}
              staggerMs={120}
              delta={delta(overview.total_input_tokens, prevOverview?.total_input_tokens ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.outputTokens", "输出 Token")}
              value={formatNumber(overview.total_output_tokens)}
              staggerMs={180}
              delta={delta(overview.total_output_tokens, prevOverview?.total_output_tokens ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.cacheTokens", "缓存 Token")}
              value={formatNumber(overview.total_cache_tokens)}
              staggerMs={240}
              delta={delta(overview.total_cache_tokens, prevOverview?.total_cache_tokens ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.cacheRate", "缓存率")}
              value={overview.cache_rate.toFixed(1)}
              unit="%"
              staggerMs={300}
              delta={delta(overview.cache_rate, prevOverview?.cache_rate ?? 0)}
              t={t}
            />
            <OverviewCard
              label={t("stats.avgLatency", "平均延迟")}
              value={overview.avg_duration_ms.toFixed(0)}
              unit="ms"
              staggerMs={360}
              delta={delta(overview.avg_duration_ms, prevOverview?.avg_duration_ms ?? 0)}
              deltaInverse
              t={t}
            />
            <OverviewCard
              label={t("stats.totalCost", "预估成本")}
              value={"$" + formatCost(overview.total_cost)}
              level={costLevel(overview.total_cost)}
              staggerMs={420}
              delta={delta(overview.total_cost, prevOverview?.total_cost ?? 0)}
              deltaInverse
              t={t}
            />
          </div>

          {/* 主图区四 tab（spec C1）：tab 本地 state 切换，筛选条作用于所有 tab */}
          <div role="tablist" aria-label={t("page.stats", "使用统计")} style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {TABS.map(({ id, key }) => (
              <Button
                key={id}
                role="tab"
                aria-selected={activeTab === id}
                variant={activeTab === id ? "default" : "ghost"}
                style={{
                  fontSize: 12,
                  padding: "4px 10px",
                  height: "auto",
                  ...(activeTab === id
                    ? {
                        background: "var(--accent-subtle)",
                        color: "var(--accent)",
                        borderColor: "color-mix(in srgb, var(--accent-edge) 40%, var(--border))",
                      }
                    : {}),
                }}
                onClick={() => setActiveTab(id)}
              >
                {t(key)}
              </Button>
            ))}
          </div>

          {/* 时间序列 tab：公共层 LineChart / StackedAreaChart（多序列可切堆叠视图，#39 批次二）；
              series_by 多序列（主琥珀线 = 最大维度值） */}
          {activeTab === "trend" && (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {trend.multi && (
                <div role="group" aria-label={t("stats.viewMode", "视图")} style={{ display: "flex", gap: 4, justifyContent: "flex-end" }}>
                  <Button
                    variant={!trendStacked ? "default" : "ghost"}
                    aria-pressed={!trendStacked}
                    style={{
                      fontSize: 12, padding: "4px 10px", height: "auto",
                      ...(!trendStacked
                        ? {
                            background: "var(--accent-subtle)",
                            color: "var(--accent)",
                            borderColor: "color-mix(in srgb, var(--accent-edge) 40%, var(--border))",
                          }
                        : {}),
                    }}
                    onClick={() => setTrendStacked(false)}
                  >
                    {t("stats.viewLine", "折线")}
                  </Button>
                  <Button
                    variant={trendStacked ? "default" : "ghost"}
                    aria-pressed={trendStacked}
                    style={{
                      fontSize: 12, padding: "4px 10px", height: "auto",
                      ...(trendStacked
                        ? {
                            background: "var(--accent-subtle)",
                            color: "var(--accent)",
                            borderColor: "color-mix(in srgb, var(--accent-edge) 40%, var(--border))",
                          }
                        : {}),
                    }}
                    onClick={() => setTrendStacked(true)}
                  >
                    {t("stats.viewStacked", "堆叠")}
                  </Button>
                </div>
              )}
              {trendStacked && trend.multi ? (
                <StackedAreaChart
                  title={t("stats.requestTrend", "请求趋势")}
                  subtitle={
                    <>
                      {t("stats.granularityLabel", "粒度")}：{granLabel(effectiveGran, effectiveGran !== granularity, t)}
                      {(effectiveGran === "minute" || effectiveGran === "5min") && (
                        <span title={t("stats.fineGranHint", "分钟级数据来自请求日志，仅短期可用（受日志保留天数限制）")}
                          style={{ marginLeft: 6, color: "var(--color-warning, var(--text-tertiary))", cursor: "help" }}>
                          {t("stats.fineGranBadge", "· 仅短期可用")}
                        </span>
                      )}
                    </>
                  }
                  config={trend.config}
                  data={trend.rows}
                  valueFormat={formatNumber}
                />
              ) : (
                <LineChart
                  title={t("stats.requestTrend", "请求趋势")}
                  subtitle={
                    <>
                      {t("stats.granularityLabel", "粒度")}：{granLabel(effectiveGran, effectiveGran !== granularity, t)}
                      {(effectiveGran === "minute" || effectiveGran === "5min") && (
                        <span title={t("stats.fineGranHint", "分钟级数据来自请求日志，仅短期可用（受日志保留天数限制）")}
                          style={{ marginLeft: 6, color: "var(--color-warning, var(--text-tertiary))", cursor: "help" }}>
                          {t("stats.fineGranBadge", "· 仅短期可用")}
                        </span>
                      )}
                    </>
                  }
                  config={trend.config}
                  data={trend.rows}
                  valueFormat={formatNumber}
                />
              )}
            </div>
          )}

          {/* 占比 tab：成本占比环形（#35 收编进图表引擎 DonutChart），维度切换沿用 groupBy；
              维度×日活跃热力（#39 批次二 DimensionHeatmap 消费落点——密度 tab 已被 HourHeatmap 占据） */}
          {activeTab === "share" && (
            <>
              <DonutChart
                title={
                  <>
                    {t("stats.costShare", "成本占比")} — {t(`stats.by${groupBy.charAt(0).toUpperCase() + groupBy.slice(1)}`, groupBy)}
                  </>
                }
                data={dims.filter(e => e.total_cost > 0).map(e => ({ name: e.name, value: e.total_cost }))}
                formatValue={formatCostUsd}
                centerLabel={t("stats.totalCost", "预估成本")}
              />
              <DimensionHeatmap
                title={t("stats.dimHeatTitle", "活跃热力（维度 × 日）")}
                data={dimHeat}
                formatValue={formatNumber}
              />
            </>
          )}

          {/* 密度 tab（T10 起双视图）：时刻热力 ⇄ 请求散点（延迟 × 成本，D2 服务端 bin 化）。
              视图切换按钮样式沿用 tab 按钮惯例；筛选条照常作用于两视图。 */}
          {activeTab === "density" && (
            <>
              <div style={{ display: "flex", gap: 4 }}>
                {(["heat", "scatter"] as const).map((v) => (
                  <Button
                    key={v}
                    aria-pressed={densityView === v}
                    variant={densityView === v ? "default" : "ghost"}
                    style={{
                      fontSize: 12,
                      padding: "4px 10px",
                      height: "auto",
                      ...(densityView === v
                        ? {
                            background: "var(--accent-subtle)",
                            color: "var(--accent)",
                            borderColor: "color-mix(in srgb, var(--accent-edge) 40%, var(--border))",
                          }
                        : {}),
                    }}
                    onClick={() => setDensityView(v)}
                  >
                    {v === "heat"
                      ? t("stats.densityViewHeat", "时刻热力")
                      : t("stats.densityViewScatter", "请求分布")}
                  </Button>
                ))}
              </div>
              {densityView === "heat" ? (
                <HourHeatmap
                  title={t("stats.heatTitle", "请求密度（星期 × 小时）")}
                  data={buildHeatCells(heatBuckets ?? [])}
                  formatValue={formatNumber}
                />
              ) : (
                <ScatterChart
                  title={t("stats.scatterTitle", "请求分布（延迟 × 成本）")}
                  histogram={scatterHist ?? { duration_bins: [], cost_bins: [], counts: [] }}
                />
              )}
            </>
          )}

          {/* 配额 tab（T10 接通 D4）：每平台 GaugeChart 快照 + 趋势 sparkline；
              无快照 → 诚实空态（快照仅在真实余额查询成功时产生） */}
          {activeTab === "quota" && (() => {
            const gauges = buildQuotaGauges(quotaSnaps ?? []);
            if (gauges.length === 0) {
              return (
                <GaugeChart
                  value={0}
                  max={0}
                  title={t("stats.quotaTitle", "配额快照")}
                  emptyHint={t("stats.quotaEmptyHint", "暂无配额快照；平台余额真实查询成功后会自动记录")}
                />
              );
            }
            return (
              <div>
                <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 8 }}>
                  {t("stats.quotaPeakNote", "占比 = 当前余额 / 窗口内峰值余额")}
                </div>
                <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
                  {gauges.map((g) => (
                    <GaugeChart
                      key={g.platformId}
                      value={g.current}
                      max={g.peak}
                      title={platforms.find(p => p.id === g.platformId)?.name ?? `#${g.platformId}`}
                      formatValue={(n) => formatCostUsd(n)}
                      trend={g.trend}
                      className="hover-lift"
                    />
                  ))}
                </div>
              </div>
            );
          })()}

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
              <Table style={{ fontSize: F.hint }}>
                <TableHeader>
                  <TableRow style={{ borderBottom: "1px solid var(--border)" }}>
                    <SortableTh label={t("stats.dimName", "名称")} col="name" align="left" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.requests", "请求")} col="total_requests" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.success", "成功")} col="success_count" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.inputTokens", "输入")} col="input_tokens" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.outputTokens", "输出")} col="output_tokens" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.cacheTokens", "缓存")} col="cache_tokens" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.cacheRate", "缓存率")} col="cache_rate" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.avgMs", "平均延迟")} col="avg_duration_ms" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                    <SortableTh label={t("stats.totalCost", "预估成本")} col="total_cost" sortKey={sortKey} sortDir={sortDir} onSort={toggleSort} />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {pagedDims.map((d, i) => {
                    const rate = successRate(d.success_count, d.total_requests);
                    return (
                      <TableRow key={safePage * PAGE_SIZE + i} style={{ borderBottom: "1px solid var(--border)", opacity: 0.9 }}>
                        <TableCell style={{ padding: "6px 8px", fontWeight: 500, maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{d.name}</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.total_requests)}</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px", color: levelColor(successRateLevel(rate, d.total_requests)) }}>{formatNumber(d.success_count)}</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.input_tokens)}</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.output_tokens)}</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.cache_tokens)}</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{d.cache_rate.toFixed(1)}%</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{d.avg_duration_ms.toFixed(0)} ms</TableCell>
                        <TableCell style={{ textAlign: "right", padding: "6px 8px", color: levelColor(costLevel(d.total_cost)) }}>${formatCost(d.total_cost)}</TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
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
  /** 数字原值：提供则启用 useCounter 滚动动效（仅顶部 hero 卡用，列表禁）。 */
  numericValue?: number;
  /** reveal stagger 延迟（ms），错峰入场。 */
  staggerMs?: number;
  unit?: string;
  level?: ColorLevel;
  /** 环比百分比，null 表示无对比基准（隐藏 delta，遵「无数据隐藏」约定）。 */
  delta: number | null;
  /** 反向指标（成本 / 延迟）：上升为「差」，箭头着 danger 色。 */
  deltaInverse?: boolean;
  t: TFunction;
}

// memo：8 张卡的 props 全是原始值 + 稳定的 t，页内排序 / 翻页 / 切 tab 这些与卡片无关的
// 重渲染就不再穿透到卡片子树（票 11 病灶 A）。
const OverviewCard = memo(function OverviewCard({ label, value, numericValue, staggerMs = 0, unit, level, delta, deltaInverse, t }: OverviewCardProps) {
  const { ref: revealRef, shown } = useReveal<HTMLDivElement>(staggerMs);
  // numericValue 提供时用 counter 滚动；否则回退到预格式化字符串
  const { ref: counterRef, display: counterDisplay } = useCounter(numericValue ?? 0, 0, 1200);
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
    <Card
      ref={revealRef}
      className={`glass-surface hover-lift reveal${shown ? " in" : ""}`}
      style={{ flex: "1 1 120px", padding: "16px 20px", display: "flex", flexDirection: "column", gap: 4 }}>
      <div style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{label}</div>
      <div className="counter" style={{ fontSize: F.title, fontWeight: 700, color: valueColor }}>
        <span ref={numericValue !== undefined ? counterRef : undefined}>
          {numericValue !== undefined ? counterDisplay : value}
        </span>{unit && <span style={{ fontSize: F.label, fontWeight: 400, marginLeft: 2 }}>{unit}</span>}
      </div>
      {deltaNode}
    </Card>
  );
});

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
    <TableHead
      onClick={() => onSort(col)}
      style={{
        textAlign: align,
        height: "auto",
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
    </TableHead>
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
      <Button variant="outline" style={{ fontSize: 12, padding: "3px 10px", height: "auto" }} disabled={page <= 0} onClick={onPrev}>
        {t("stats.prevPage", "上一页")}
      </Button>
      <span style={{ color: "var(--text-secondary)" }}>
        {/* 插值键必须是 current：翻译串写的是「第 {{current}} / {{total}} 页」
            （aidog_i18n/locales/zh-Hans.json:1894，en-US 同）。原先传的是 page，
            于是 {{current}} 原样印在界面上。Flutter 侧传的就是 current。 */}
        {t("stats.pageOf", "{{current}} / {{total}}", { current: page + 1, total: pageCount })}
      </span>
      <Button variant="outline" style={{ fontSize: 12, padding: "3px 10px", height: "auto" }} disabled={page >= pageCount - 1} onClick={onNext}>
        {t("stats.nextPage", "下一页")}
      </Button>
    </div>
  );
}
