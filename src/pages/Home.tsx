// ─── 首页 · 命令面板（Command Palette · #36 / spec C3）─────────────────
// 单块命令面板：琥珀渐变眉条 → 搜索栏式状态行（运行态点 + 端口 + ⌘C 复制地址）
// → 四 KPI 紧凑行（每格行内 sparkline，花费=琥珀其余灰阶）→ 24h 紧凑双线趋势
// （琥珀主线 + 灰阶虚线辅线）→ 平台 Top4（迷你环形 + 行内占比条 + 等宽数字）
// → 总余额行 → 快捷键 footer（⌘N/⌘S/⌘L/⌘C 视觉示意，键位绑定不做）。
// 旧三曲线主图删除，深分析归 Stats。数据源（各区独立 catch）/ reveal 入场 / RTL / i18n 不变。

import { useState, useEffect, useCallback } from "react";
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
} from "../services/api";
import { formatNumber, formatCostUsd, formatPercent } from "../utils/formatters";
import { writeText } from "../services/platform";
import { useReveal } from "../components/shared";
import { seriesColor } from "@/components/charts";
import { F } from "../domains/shared/tokens";

const DEFAULT_PORT = 7890;
const TOP_PLATFORMS = 4;

// 命令面板视觉 token（direction-approved.md 锁定）：分层中性面 s1/s2、行线、SF Mono 数字栈。
const PANEL = {
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
 * 数值序列 → 归一化 [x,y] 点集（sparkline / 趋势线共用）：
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
const ptsToPath = (pts: Array<[number, number]>) =>
  pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");

/** KPI 格行内 sparkline：100×22 viewBox 无填充折线（公共层无现成组件，最小 SVG 内联）。 */
function Sparkline({ values, color }: { values: number[]; color: string }) {
  if (values.length < 2) return null;
  return (
    <svg
      viewBox="0 0 100 22"
      preserveAspectRatio="none"
      style={{ display: "block", marginTop: 8, width: "100%", height: 22 }}
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

export function Home({ onNavigate }: { onNavigate: (id: string) => void }) {
  const { t } = useTranslation();
  const [running, setRunning] = useState<boolean | null>(null);
  const [port, setPort] = useState<number>(DEFAULT_PORT);
  const [today, setToday] = useState<TodayStats | null>(null);
  const [platformsToday, setPlatformsToday] = useState<TodayPlatformStat[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [trendBuckets, setTrendBuckets] = useState<StatsBucket[]>([]);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  // 并行拉取，各区独立 catch 兜底（单 API 失败该区空态，不整页崩）。
  const load = useCallback(async () => {
    // 最近 24 小时 hourly 趋势：now-24h → now 滚动窗口（24 桶），喂 KPI sparkline + 双线趋势区。
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

  // 24h 趋势 / KPI sparkline 序列（hourly 桶）。
  const reqSeries = trendBuckets.map(b => b.total_requests);
  const costSeries = trendBuckets.map(b => b.total_cost);
  const tokensSeries = trendBuckets.map(b => b.input_tokens + b.output_tokens + b.cache_tokens);
  const cacheSeries = trendBuckets.map(b => b.cache_tokens);
  const trendPeak = trendBuckets.reduce((m, b) => Math.max(m, b.total_requests), 0);
  const hasTrend = reqSeries.some(v => v > 0);

  const statusColor = running == null
    ? PANEL.muted
    : running ? "var(--color-success)" : PANEL.muted;
  const statusText = running == null
    ? t("home.statusUnknown", "未知")
    : running ? t("home.statusRunning", "运行中") : t("home.statusStopped", "已停止");

  // 24h 趋势 SVG 几何：viewBox 640×88，主线（请求）琥珀 + 面积填充，辅线（花费）灰阶虚线独立归一化。
  const TW = 640;
  const TH = 88;
  const mainPts = normPoints(reqSeries, TW, TH, 8);
  const mainPath = ptsToPath(mainPts);
  const areaPath = mainPts.length > 1 ? `${mainPath} L ${TW},${TH} L 0,${TH} Z` : "";
  const auxPath = ptsToPath(normPoints(costSeries, TW, TH, 8));

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

  const footChips = [
    { label: t("home.addPlatform", "添加平台"), kbd: "⌘N", run: () => onNavigate("platforms") },
    { label: t("home.viewStats", "查看统计"), kbd: "⌘S", run: () => onNavigate("stats") },
    { label: t("home.viewLogs", "查看日志"), kbd: "⌘L", run: () => onNavigate("logs") },
    { label: t("home.copyBaseUrl", "复制代理地址"), kbd: "⌘C", run: copyUrl },
  ];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 720, margin: "0 auto", width: "100%" }}>
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

        {/* 3. 24h 紧凑双线趋势：琥珀主线（请求，面积填充）+ 灰阶虚线辅线（花费，独立标尺） */}
        <div
          ref={revealTrend.ref}
          className={`reveal${revealTrend.shown ? " in" : ""}`}
          style={{ padding: "14px 16px", borderBottom: `1px solid ${PANEL.line}` }}
        >
          <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 12, marginBottom: 10, flexWrap: "wrap" }}>
            <b style={{ fontSize: F.small + 1, color: PANEL.fg }}>{t("home.trend24h", "24 小时趋势")}</b>
            <span style={{ fontFamily: PANEL.mono, fontSize: 10, color: PANEL.muted }}>
              HOURLY · {t("home.trendRequests", "请求数")} / {t("home.trendCost", "花费")}
              {hasTrend && ` · ${t("home.trendPeak", "峰值")} ${formatNumber(trendPeak)}`}
            </span>
          </div>
          {hasTrend ? (
            <>
              <svg viewBox={`0 0 ${TW} ${TH}`} preserveAspectRatio="none" style={{ display: "block", width: "100%", height: TH }}>
                <defs>
                  <linearGradient id="homeCmdTrendArea" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0" stopColor={seriesColor(0)} stopOpacity=".2" />
                    <stop offset="1" stopColor={seriesColor(0)} stopOpacity="0" />
                  </linearGradient>
                </defs>
                <g stroke="rgba(255,255,255,.05)">
                  <line x1={0} y1={22} x2={TW} y2={22} />
                  <line x1={0} y1={44} x2={TW} y2={44} />
                  <line x1={0} y1={66} x2={TW} y2={66} />
                </g>
                <path d={areaPath} fill="url(#homeCmdTrendArea)" />
                <path d={mainPath} fill="none" stroke={seriesColor(0)} strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
                <path d={auxPath} fill="none" stroke={seriesColor(1)} strokeWidth={1.2} strokeDasharray="3 3" vectorEffect="non-scaling-stroke" />
              </svg>
              {/* x 轴整点小时标注：每 6 桶（hourly 桶 time_bucket = "YYYY-MM-DD HH:00:00"） */}
              <div style={{ position: "relative", height: 12 }}>
                {trendBuckets.map((b, i) =>
                  i % 6 === 0 ? (
                    <span
                      key={i}
                      style={{
                        position: "absolute",
                        left: `${((i / (trendBuckets.length - 1)) * 100).toFixed(1)}%`,
                        transform: "translateX(-50%)",
                        fontFamily: PANEL.mono,
                        fontSize: 8,
                        color: PANEL.muted,
                        whiteSpace: "nowrap",
                      }}
                    >
                      {b.time_bucket.slice(11, 13)}
                    </span>
                  ) : null,
                )}
              </div>
            </>
          ) : (
            <div style={{ fontSize: F.hint, color: PANEL.muted, padding: "8px 0" }}>
              {loading ? "" : t("home.noToday", "今日暂无请求")}
            </div>
          )}
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
