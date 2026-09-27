// ─── 首页 · 命令面板（Command Palette · #36 / spec C3）─────────────────
// 单块命令面板：琥珀渐变眉条 → 搜索栏式状态行（运行态点 + 端口 + ⌘C 复制地址）
// → 四 KPI 紧凑行（每格行内 sparkline，花费=琥珀其余灰阶）→ 维度趋势
// （按模型 / 按分组堆叠面积 + 右轴请求线，2026-09-27 起由面板外玻璃卡移入，
// 原 24h 总量双线趋势删除，深分析归 Stats）→ 平台 Top4（迷你环形 + 行内占比条
// + 等宽数字）→ 总余额行 → 快捷键 footer（⌘N/⌘S/⌘L/⌘C chip 可点击 + keydown 绑定同动作）。
// 数据源（各区独立 catch）/ reveal 入场 / RTL / i18n 不变。

import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  proxyApi,
  trayConfigApi,
  popoverConfigApi,
  platformApi,
  statsApi,
  onProxyLogUpdated,
  type TodayStats,
  type TodayPlatformStat,
  type Platform,
  type StatsBucket,
  type DimensionEntry,
  type StatsSeries,
} from "../services/api";
import type { NavContext } from "../components/Sidebar";
import { formatNumber, formatCostUsd, formatPercent } from "../utils/formatters";
import { writeText } from "../services/platform";
import { useReveal } from "../components/shared";
import { seriesColor } from "@/components/charts";
import { HomeTrendChart, buildSparkMap } from "./HomeTrendChart";
import { F } from "../domains/shared/tokens";

const DEFAULT_PORT = 7890;
const TOP_PLATFORMS = 4;
export const DIM_TOP_N = 8;

// 维度行（模型 / 分组，home-model-stats spec §2）：tokens 降序前 DIM_TOP_N，
// 其余合并成一条「其它」行（分母不可加的比率在该行显 --）。
const dimTokens = (d: DimensionEntry) => d.input_tokens + d.output_tokens + d.cache_tokens;

export function buildDimRows(
  data: DimensionEntry[],
  /** 分组维度传入「未分组平台」标签：空 group_key 是真实语义，标出且不可点下钻。
   *  模型维度不传：空 model 行已被 Rust 过滤，这里再滤一道防旧内核。 */
  ungroupedLabel?: string,
): { rows: { name: string; d: DimensionEntry; other: boolean; unclickable: boolean }[]; total: number } {
  const named = ungroupedLabel
    ? data.map(d => (d.name ? d : { ...d, name: ungroupedLabel }))
    : data.filter(d => d.name !== "");
  const sorted = [...named].sort((a, b) => dimTokens(b) - dimTokens(a));
  const top = sorted.slice(0, DIM_TOP_N);
  const rest = sorted.slice(DIM_TOP_N);
  const rows = top.map(d => ({
    name: d.name,
    d,
    other: false,
    // 「未分组平台」行显示自己的名字，只是不可点（groupKey='' 到统计页等于不带筛选）。
    unclickable: d.name === ungroupedLabel,
  }));
  if (rest.length > 0) {
    rows.push({
      name: "",
      other: true,
      unclickable: true,
      d: rest.reduce((acc, d) => ({
        name: "",
        total_requests: acc.total_requests + d.total_requests,
        success_count: acc.success_count + d.success_count,
        input_tokens: acc.input_tokens + d.input_tokens,
        output_tokens: acc.output_tokens + d.output_tokens,
        cache_tokens: acc.cache_tokens + d.cache_tokens,
        cache_rate: 0,
        avg_duration_ms: 0,
        total_cost: acc.total_cost + d.total_cost,
      })),
    });
  }
  return { rows, total: sorted.reduce((s, d) => s + dimTokens(d), 0) };
}

// 命令面板视觉 token（direction-approved.md 锁定）：分层中性面 s1/s2、行线、SF Mono 数字栈。
// 导出供 HomeTrendChart（嵌面板内的维度趋势）取同一份显式浅色文字。
export const PANEL = {
  fg: "#f5f5f0",
  muted: "#8a8580",
  s1: "#0e0e0e",
  s2: "#151514",
  line: "rgba(255,255,255,.07)",
  mono: '"SF Mono", ui-monospace, Menlo, monospace',
} as const;
// 琥珀渐变眉条（Raycast 品牌渐变位换琥珀系）。
const BROW_GRADIENT = "linear-gradient(90deg, #64521d, #e8c547 55%, #f2dc8a)";

/**
 * 数值序列 → 归一化 [x,y] 点集（KPI sparkline 用）：
 * y 按 min-max 缩放进 [pad, h-pad]（平坦序列落中线，不除零）；x 均分（单点居中）。
 */
export function normPoints(
  values: number[],
  w: number,
  h: number,
  pad = 2,
): Array<[number, number]> {
  if (values.length === 0) return [];
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min;
  return values.map((v, i) => [
    values.length > 1 ? (i / (values.length - 1)) * w : w / 2,
    span > 0 ? pad + (1 - (v - min) / span) * (h - 2 * pad) : h / 2,
  ]);
}

const ptsToPolyline = (pts: Array<[number, number]>) =>
  pts.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");

/** KPI 格行内 sparkline：viewBox 无填充折线（公共层无现成组件，最小 SVG 内联）。
 *  width 缺省占满父格（KPI 用）；DimPanel 行尾传定宽。 */
function Sparkline({
  values,
  color,
  width,
  marginTop = 8,
}: {
  values: number[];
  color: string;
  width?: number;
  marginTop?: number;
}) {
  if (values.length < 2) return null;
  return (
    <svg
      viewBox="0 0 100 22"
      preserveAspectRatio="none"
      style={{ display: "block", marginTop, width: width ?? "100%", height: 22, flexShrink: 0 }}
    >
      <polyline
        points={ptsToPolyline(normPoints(values, 100, 22))}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinejoin="round"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

/** KPI 紧凑格：等宽 label + 等宽 tabular-nums 大数字 + 行内 sparkline。 */
function KpiCell({ label, value, spark, amber }: { label: string; value: string; spark: number[]; amber?: boolean }) {
  return (
    <div style={{ padding: "14px 16px", minWidth: 0 }}>
      <div style={{ fontFamily: PANEL.mono, fontSize: 10, letterSpacing: ".1em", color: PANEL.muted, whiteSpace: "nowrap" }}>
        {label}
      </div>
      <div
        style={{
          fontFamily: PANEL.mono,
          fontSize: 24,
          fontWeight: 700,
          fontVariantNumeric: "tabular-nums",
          color: PANEL.fg,
          marginTop: 3,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {value}
      </div>
      <Sparkline values={spark} color={amber ? seriesColor(0) : seriesColor(1)} />
    </div>
  );
}

/** 平台行迷你环形：琥珀弧 = share（该平台花费 / Top4 合计），余弧弱灰。 */
function MiniRing({ share }: { share: number }) {
  const r = 9;
  const c = 2 * Math.PI * r;
  const len = Math.max(0, Math.min(1, share)) * c;
  return (
    <svg viewBox="0 0 26 26" width={26} height={26} style={{ flexShrink: 0 }}>
      <circle cx={13} cy={13} r={r} fill="none" stroke="rgba(255,255,255,.14)" strokeWidth={5} />
      <circle
        cx={13}
        cy={13}
        r={r}
        fill="none"
        stroke={seriesColor(0)}
        strokeWidth={5}
        strokeDasharray={`${len.toFixed(2)} ${c.toFixed(2)}`}
        transform="rotate(-90 13 13)"
      />
    </svg>
  );
}

export function Home({ onNavigate }: { onNavigate: (id: string, context?: NavContext) => void }) {
  const { t } = useTranslation();
  const [running, setRunning] = useState<boolean | null>(null);
  const [port, setPort] = useState<number>(DEFAULT_PORT);
  const [today, setToday] = useState<TodayStats | null>(null);
  const [platformsToday, setPlatformsToday] = useState<TodayPlatformStat[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [trendBuckets, setTrendBuckets] = useState<StatsBucket[]>([]);
  const [dimModels, setDimModels] = useState<DimensionEntry[]>([]);
  const [dimGroups, setDimGroups] = useState<DimensionEntry[]>([]);
  // 维度小时序列（home-dim-trend）：同一次 queryBatch 连带 series_by 取回，
  // 喂维度趋势图 + DimPanel 行迷你曲线（与行数据共用同一份查询）。
  const [seriesModels, setSeriesModels] = useState<StatsSeries[]>([]);
  const [seriesGroups, setSeriesGroups] = useState<StatsSeries[]>([]);
  // 平台维度小时序列（home-dim-trend 按平台 tab，2026-09-27）：仅喂趋势图，
  // DimPanel 行不消费（平台维度不加行面板）。序列名 = 平台显示名（后端回填）。
  const [seriesPlatforms, setSeriesPlatforms] = useState<StatsSeries[]>([]);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  // 并行拉取，各区独立 catch 兜底（单 API 失败该区空态，不整页崩）。
  const load = useCallback(async () => {
    // 最近 24 小时 hourly 趋势：now-24h → now 滚动窗口（24 桶），喂 KPI sparkline
    //（24h 总量趋势图已删；维度趋势 / 维度面板走下面的 queryBatch，不消费本查询）。
    const now = new Date();
    const windowStart = now.getTime() - 24 * 3600 * 1000;
    await Promise.all([
      proxyApi.status().then(setRunning).catch(() => setRunning(null)),
      proxyApi.getSettings().then(s => setPort(s.port)).catch(() => {}),
      trayConfigApi.todayStats().then(setToday).catch(() => setToday(null)),
      popoverConfigApi.platformToday().then(setPlatformsToday).catch(() => setPlatformsToday([])),
      platformApi.list().then(setPlatforms).catch(() => setPlatforms([])),
      statsApi.query({ start: windowStart, end: now.getTime(), granularity: "hourly" })
        .then(r => setTrendBuckets(r.buckets)).catch(() => setTrendBuckets([])),
      // 模型 / 分组 / 平台维度统计（同 24h 窗）：一次 batch 三条 group_by + series_by，
      // dimension_data 喂行、series 喂维度趋势图与行迷你曲线（同一份数据；
      // 平台维度仅趋势图消费，dimension_data 不落地 state）。
      statsApi.queryBatch([
        { start: windowStart, end: now.getTime(), granularity: "hourly", group_by: "model", series_by: "model" },
        { start: windowStart, end: now.getTime(), granularity: "hourly", group_by: "group", series_by: "group" },
        { start: windowStart, end: now.getTime(), granularity: "hourly", group_by: "platform", series_by: "platform" },
      ])
        .then(([m, g, p]) => {
          setDimModels(m?.dimension_data ?? []);
          setDimGroups(g?.dimension_data ?? []);
          setSeriesModels(m?.series ?? []);
          setSeriesGroups(g?.series ?? []);
          setSeriesPlatforms(p?.series ?? []);
        })
        .catch(() => {
          setDimModels([]); setDimGroups([]);
          setSeriesModels([]); setSeriesGroups([]); setSeriesPlatforms([]);
        }),
    ]);
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);
  // 请求完成后后端 emit "proxy-log-updated" → 重载今日 / 平台用量 / 趋势。
  useEffect(() => onProxyLogUpdated(() => { load(); }), [load]);

  const proxyBaseUrl = `http://127.0.0.1:${port}/proxy`;
  const copyUrl = useCallback(() => {
    writeText(proxyBaseUrl)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => {});
  }, [proxyBaseUrl]);

  // 今日是否有数据：requests/cost/tokens 任一 > 0。
  const hasTodayData = !!today && (today.total_requests > 0 || today.cost > 0 || today.tokens > 0);

  // 总余额 = 关联平台 est_balance_remaining 求和（平台级属性，无 per-group 概念）。
  const totalBalance = platforms.reduce((acc, p) => acc + (p.est_balance_remaining || 0), 0);

  // 平台今日用量 top N（已用 cost 降序）。
  const topPlatforms = [...platformsToday]
    .filter(p => p.cost > 0 || p.tokens > 0 || p.requests > 0)
    .sort((a, b) => b.cost - a.cost)
    .slice(0, TOP_PLATFORMS);
  const maxPlatformCost = topPlatforms.reduce((m, p) => Math.max(m, p.cost), 0);
  const topCostSum = topPlatforms.reduce((s, p) => s + p.cost, 0);

  // 24h 趋势 / KPI sparkline 序列（hourly 桶；24h 总量双线趋势图已删，KPI sparkline 仍消费）。
  const reqSeries = trendBuckets.map(b => b.total_requests);
  const costSeries = trendBuckets.map(b => b.total_cost);
  const tokensSeries = trendBuckets.map(b => b.input_tokens + b.output_tokens + b.cache_tokens);
  const cacheSeries = trendBuckets.map(b => b.cache_tokens);

  // ── 模型 / 分组维度行（home-model-stats spec §2）──
  const modelRows = useMemo(() => buildDimRows(dimModels), [dimModels]);
  const ungroupedLabel = t("platform.ungrouped", "未分组平台");
  const groupRows = useMemo(
    () => buildDimRows(dimGroups, ungroupedLabel),
    [dimGroups, ungroupedLabel],
  );
  // 行迷你曲线数据（home-dim-trend §2）：维度 → 24h 逐桶 token 序列。
  const modelSparks = useMemo(() => buildSparkMap(seriesModels), [seriesModels]);
  const groupSparks = useMemo(
    () => buildSparkMap(seriesGroups, ungroupedLabel),
    [seriesGroups, ungroupedLabel],
  );

  const statusColor = running == null
    ? PANEL.muted
    : running ? "var(--color-success)" : PANEL.muted;
  const statusText = running == null
    ? t("home.statusUnknown", "未知")
    : running ? t("home.statusRunning", "运行中") : t("home.statusStopped", "已停止");

  // 萤火虫动效：面板内 5 区块 reveal 入场错峰（0/70/140/210/280ms）。
  const revealSearch = useReveal<HTMLDivElement>(0);
  const revealKpi = useReveal<HTMLDivElement>(70);
  const revealTrend = useReveal<HTMLDivElement>(140);
  const revealPlats = useReveal<HTMLDivElement>(210);
  const revealFoot = useReveal<HTMLDivElement>(280);

  const kpis = hasTodayData && today
    ? [
      { label: t("home.cost", "费用"), value: formatCostUsd(today.cost), spark: costSeries, amber: true },
      { label: t("home.tokens", "Token"), value: formatNumber(today.tokens), spark: tokensSeries },
      { label: t("home.requests", "请求"), value: formatNumber(today.total_requests), spark: reqSeries },
      { label: t("home.cacheRate", "缓存率"), value: formatPercent(today.cache_rate), spark: cacheSeries },
    ]
    : [];

  const footChips = useMemo(
    () => [
      { label: t("home.addPlatform", "添加平台"), kbd: "⌘N", key: "n", run: () => onNavigate("platforms") },
      { label: t("home.viewStats", "查看统计"), kbd: "⌘S", key: "s", run: () => onNavigate("stats") },
      { label: t("home.viewLogs", "查看日志"), kbd: "⌘L", key: "l", run: () => onNavigate("logs") },
      { label: t("home.copyBaseUrl", "复制代理地址"), kbd: "⌘C", key: "c", run: copyUrl },
    ],
    [t, onNavigate, copyUrl],
  );

  // 快捷键绑定（footer chip 同动作）：Mac ⌘ 为主、Ctrl 兼容；焦点在输入类元素时不触发，
  // 防打断输入；命中即 preventDefault 抢占 —— ⌘C 是复制代理地址（chip onClick 同语义），
  // 不抢占会变成系统复制。卸载 / chips 变化时摘监听。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey || !(e.metaKey || e.ctrlKey)) return;
      const k = e.key.toLowerCase();
      if (!/^[a-z]$/.test(k)) return;
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable)) return;
      const chip = footChips.find(c => c.key === k);
      if (!chip) return;
      e.preventDefault();
      chip.run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [footChips]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 1200, margin: "0 auto", width: "100%" }}>
      {/* Header */}
      <div>
        <div className="section-title">{t("page.home", "首页")}</div>
        <div className="section-desc">{t("home.desc", "代理状态、今日用量与分组平台速览")}</div>
      </div>

      {/* 琥珀渐变眉条（Raycast 品牌渐变位换琥珀系） */}
      <div style={{ height: 3, borderRadius: 2, background: BROW_GRADIENT }} />

      {/* 命令面板单块 */}
      <div
        style={{
          background: PANEL.s1,
          border: `1px solid ${PANEL.line}`,
          borderRadius: 14,
          overflow: "hidden",
          boxShadow: "0 8px 32px rgba(0,0,0,.6)",
        }}
      >
        {/* 1. 搜索栏式状态行：运行态点 + 端口 + ⌘C 复制地址 */}
        <div
          ref={revealSearch.ref}
          className={`reveal${revealSearch.shown ? " in" : ""}`}
          style={{ display: "flex", alignItems: "center", gap: 10, padding: "14px 16px", borderBottom: `1px solid ${PANEL.line}` }}
        >
          <span
            style={{
              width: 10,
              height: 10,
              borderRadius: "50%",
              flexShrink: 0,
              background: statusColor,
              boxShadow: running ? "0 0 0 4px color-mix(in srgb, var(--color-success) 14%, transparent)" : "none",
            }}
          />
          <span style={{ fontSize: F.body, fontWeight: 600, color: PANEL.fg, minWidth: 0, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {t("home.proxyStatus", "代理服务")} {statusText} · {t("home.port", "端口")} {port}
          </span>
          <button
            type="button"
            className="cmd-kb"
            style={{ marginInlineStart: "auto" }}
            title={t("home.copyBaseUrlTitle", "复制代理 base_url")}
            onClick={copyUrl}
          >
            <span className="k">⌘C</span>
            {copied ? "✓" : t("home.copyBaseUrl", "复制代理地址")}
          </button>
        </div>

        {/* 2. 四 KPI 紧凑行：今日花费（琥珀 sparkline）/ Token / 请求 / 缓存率（灰阶） */}
        <div
          ref={revealKpi.ref}
          className={`reveal${revealKpi.shown ? " in" : ""}`}
          style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", borderBottom: `1px solid ${PANEL.line}` }}
        >
          {kpis.length > 0 ? (
            kpis.map((k, i) => (
              <div key={k.label} style={{ borderInlineEnd: i < kpis.length - 1 ? `1px solid ${PANEL.line}` : undefined }}>
                <KpiCell {...k} />
              </div>
            ))
          ) : (
            <div style={{ padding: "14px 16px", fontSize: F.hint, color: PANEL.muted }}>
              {loading ? "" : t("home.noToday", "今日暂无请求")}
            </div>
          )}
        </div>

        {/* 3. 维度趋势（home-dim-trend）：2026-09-27 由面板外独立玻璃卡移入面板
            （原 24h 总量双线趋势位）。bare 拆外壳融入面板，文字走 PANEL 显式色。 */}
        <div
          ref={revealTrend.ref}
          className={`reveal${revealTrend.shown ? " in" : ""}`}
          style={{ padding: "14px 16px", borderBottom: `1px solid ${PANEL.line}` }}
        >
          <HomeTrendChart
            modelSeries={seriesModels}
            groupSeries={seriesGroups}
            platformSeries={seriesPlatforms}
            ungroupedLabel={ungroupedLabel}
            loading={loading}
          />
        </div>

        {/* 4. 平台 Top4：迷你环形（花费占比）+ 行内占比条 + 等宽数字 */}
        <div
          ref={revealPlats.ref}
          className={`reveal${revealPlats.shown ? " in" : ""}`}
          style={{ padding: "14px 16px", borderBottom: `1px solid ${PANEL.line}` }}
        >
            <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 12, marginBottom: 4, flexWrap: "wrap" }}>
              <b style={{ fontSize: F.small + 1, color: PANEL.fg }}>{t("home.topPlatforms", "今日平台用量")}</b>
              <span style={{ fontFamily: PANEL.mono, fontSize: 10, color: PANEL.muted }}>
                TOP {TOP_PLATFORMS} · {t("home.trendCost", "花费")}
              </span>
            </div>
            {topPlatforms.length > 0 ? (
              <div style={{ display: "flex", flexDirection: "column" }}>
                {topPlatforms.map((p, i) => (
                  <div
                    key={p.platform_id}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 12,
                      padding: "9px 0",
                      borderBottom: i < topPlatforms.length - 1 ? "1px solid rgba(255,255,255,.05)" : undefined,
                    }}
                  >
                    <MiniRing share={topCostSum > 0 ? p.cost / topCostSum : 0} />
                    <span style={{ fontSize: F.small + 1, fontWeight: 600, color: PANEL.fg, flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.platform_name}
                    </span>
                    <span style={{ flex: 1, height: 3, background: "rgba(232,197,71,.12)", borderRadius: 2, overflow: "hidden" }}>
                      <span
                        style={{
                          display: "block",
                          width: `${maxPlatformCost > 0 ? (p.cost / maxPlatformCost) * 100 : 0}%`,
                          height: "100%",
                          background: seriesColor(0),
                          borderRadius: 2,
                          transition: "width 0.3s ease",
                        }}
                      />
                    </span>
                    <span style={{ fontFamily: PANEL.mono, fontSize: 11, color: PANEL.muted, whiteSpace: "nowrap" }}>
                      {formatNumber(p.requests)} · {formatNumber(p.tokens)}
                    </span>
                    <span style={{ fontFamily: PANEL.mono, fontSize: 12, color: seriesColor(0), whiteSpace: "nowrap" }}>
                      {formatCostUsd(p.cost)}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <div style={{ fontSize: F.hint, color: PANEL.muted, padding: "4px 0" }}>
                {loading ? "" : t("home.noToday", "今日暂无请求")}
              </div>
            )}
        </div>

        {/* 4.5 模型 / 分组维度（home-model-stats spec §2）：并排两面板，TopN 横条 + 五指标 */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(420px, 1fr))",
            gap: 1,
            background: PANEL.line,
            borderBottom: `1px solid ${PANEL.line}`,
          }}
        >
          <DimPanel
            titleKey="home.byModel"
            titleDefault="按模型 · 24 小时"
            rows={modelRows.rows}
            totalTokens={modelRows.total}
            sparks={modelSparks}
            loading={loading}
            onRow={name => onNavigate("stats", { model: name })}
          />
          <DimPanel
            titleKey="home.byGroup"
            titleDefault="按分组 · 24 小时"
            rows={groupRows.rows}
            totalTokens={groupRows.total}
            sparks={groupSparks}
            loading={loading}
            onRow={name => onNavigate("stats", { groupKey: name })}
          />
        </div>

        {/* 5. 总余额行 + 快捷键 footer */}
        <div ref={revealFoot.ref} className={`reveal${revealFoot.shown ? " in" : ""}`}>
          {totalBalance > 0 && (
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "baseline",
                gap: 12,
                padding: "12px 16px",
                borderBottom: `1px solid ${PANEL.line}`,
                fontSize: F.small,
                color: PANEL.muted,
              }}
            >
              <span>{t("home.totalBalance", "总余额")}</span>
              <b style={{ fontSize: 17, color: PANEL.fg, fontFamily: PANEL.mono, fontVariantNumeric: "tabular-nums" }}>
                {formatCostUsd(totalBalance)}
              </b>
            </div>
          )}
          <div style={{ display: "flex", gap: 8, padding: "12px 14px", flexWrap: "wrap" }}>
            {footChips.map(c => (
              <button key={c.kbd} type="button" className="cmd-kb" onClick={c.run}>
                {c.label} <span className="k">{c.kbd}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── 模型 / 分组维度面板（home-model-stats spec §2）─────────────────
// 行结构：名称（弹性省略）｜tokens 占比条｜tokens｜cost｜请求数｜成功率｜缓存率｜24h 迷你曲线。
// 「其它」行灰显不可点；缓存率分母不可加，合并行显 --。
function DimPanel({
  titleKey,
  titleDefault,
  rows,
  totalTokens,
  sparks,
  loading,
  onRow,
}: {
  titleKey: string;
  titleDefault: string;
  rows: { name: string; d: DimensionEntry; other: boolean; unclickable: boolean }[];
  totalTokens: number;
  /** 行名 → 24h 逐桶 token 序列（home-dim-trend §2 行尾迷你曲线；缺名不画）。 */
  sparks: Map<string, number[]>;
  loading: boolean;
  onRow: (name: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "14px 16px", background: PANEL.s1 }}>
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 12, marginBottom: 4, flexWrap: "wrap" }}>
        <b style={{ fontSize: F.small + 1, color: PANEL.fg }}>{t(titleKey, titleDefault)}</b>
        <span style={{ fontFamily: PANEL.mono, fontSize: 10, color: PANEL.muted }}>
          24H · {t("home.dimMetric", "tokens / 花费 / 请求")}
        </span>
      </div>
      {rows.length > 0 ? (
        <div style={{ display: "flex", flexDirection: "column" }}>
          {rows.map((r, i) => (
            <div
              key={r.other ? "__other__" : r.name}
              role={r.unclickable ? undefined : "button"}
              onClick={r.unclickable ? undefined : () => onRow(r.name)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 12,
                padding: "9px 0",
                borderBottom: i < rows.length - 1 ? "1px solid rgba(255,255,255,.05)" : undefined,
                cursor: r.unclickable ? undefined : "pointer",
                opacity: r.unclickable ? 0.55 : 1,
              }}
            >
              <span
                title={r.other ? t("home.dimOther", "其它") : r.name}
                style={{
                  fontSize: F.small + 1,
                  fontWeight: 600,
                  color: r.unclickable ? PANEL.muted : PANEL.fg,
                  flex: 1,
                  minWidth: 0,
                  maxWidth: 180,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  textAlign: "start",
                }}
              >
                {r.other ? t("home.dimOther", "其它") : r.name}
              </span>
              {/* 条长 = 该行 tokens 占全量比例（不是相对 top1，读者期望占比和为 100%） */}
              <span style={{ flex: 1, height: 3, background: "rgba(232,197,71,.12)", borderRadius: 2, overflow: "hidden" }}>
                <span
                  style={{
                    display: "block",
                    width: `${totalTokens > 0 ? (dimTokens(r.d) / totalTokens) * 100 : 0}%`,
                    height: "100%",
                    background: r.unclickable ? PANEL.muted : seriesColor(0),
                    borderRadius: 2,
                    transition: "width 0.3s ease",
                  }}
                />
              </span>
              <span style={{ fontFamily: PANEL.mono, fontSize: 11, color: PANEL.muted, whiteSpace: "nowrap", minWidth: 64, textAlign: "end" }}>
                {formatNumber(dimTokens(r.d))}
              </span>
              <span style={{ fontFamily: PANEL.mono, fontSize: 12, color: seriesColor(0), whiteSpace: "nowrap", minWidth: 56, textAlign: "end" }}>
                {formatCostUsd(r.d.total_cost)}
              </span>
              <span style={{ fontFamily: PANEL.mono, fontSize: 11, color: PANEL.muted, whiteSpace: "nowrap", minWidth: 44, textAlign: "end" }}>
                {formatNumber(r.d.total_requests)}
              </span>
              <span style={{ fontFamily: PANEL.mono, fontSize: 11, color: PANEL.muted, whiteSpace: "nowrap", minWidth: 42, textAlign: "end" }}>
                {r.d.total_requests > 0 ? formatPercent((r.d.success_count / r.d.total_requests) * 100, 0) : "--"}
              </span>
              <span style={{ fontFamily: PANEL.mono, fontSize: 11, color: PANEL.muted, whiteSpace: "nowrap", minWidth: 42, textAlign: "end" }}>
                {r.other ? "--" : formatPercent(r.d.cache_rate, 0)}
              </span>
              {/* 行尾 24h token 迷你曲线（home-dim-trend §2）：该维度逐小时走势，灰阶不与占比条抢焦点 */}
              <Sparkline values={sparks.get(r.name) ?? []} color={seriesColor(1)} width={72} marginTop={0} />
            </div>
          ))}
        </div>
      ) : (
        <div style={{ fontSize: F.hint, color: PANEL.muted, padding: "4px 0" }}>
          {loading ? "" : t("home.noToday", "今日暂无请求")}
        </div>
      )}
    </div>
  );
}
