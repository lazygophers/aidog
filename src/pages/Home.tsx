// ─── 首页 · 总览仪表盘 ──────────────────────────────────
// 一眼概览 + 入口：代理状态/端口 + 今日用量 + 分组/平台速览·总余额 + 快捷操作。
// 从现有设计系统长出（Liquid Glass + CSS 变量 + 共享组件 / formatters / usageColor），
// 真实数据 only，无数据留诚实空态；深度分析留 Stats，本页只做概览与跳转入口。

import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  proxyApi,
  trayConfigApi,
  popoverConfigApi,
  groupDetailApi,
  platformApi,
  statsApi,
  onProxyLogUpdated,
  type TodayStats,
  type TodayPlatformStat,
  type GroupDetail,
  type Platform,
  type StatsBucket,
} from "../services/api";
import { formatNumber, formatCostUsd, formatPercent } from "../utils/formatters";
import { StatChip, BalanceBar, costLevel, levelColor } from "../components/shared";
import {
  IconCost,
  IconBolt,
  IconPackage,
  IconCard,
  IconPlatforms,
  IconStats,
  IconLogs,
} from "../components/icons";

const F = { title: 20, label: 15, body: 14, hint: 13, small: 12 } as const;
const DEFAULT_PORT = 7890;
const TOP_PLATFORMS = 5;

/** Copy text to clipboard with brief visual feedback（对齐 Groups.tsx CopyButton 模式）。 */
function CopyButton({ text, title, label, size = 14 }: { text: string; title?: string; label?: string; size?: number }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  return (
    <button
      className={label ? "btn btn-ghost" : "btn btn-ghost btn-icon"}
      onClick={handleCopy}
      title={title || text}
      style={{ position: "relative", flexShrink: 0, gap: label ? 5 : 0, fontSize: label ? 12 : undefined, padding: label ? "4px 10px" : undefined }}
    >
      {copied ? (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M20 6L9 17l-5-5" />
        </svg>
      ) : (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
      {label && <span style={{ fontWeight: 500 }}>{label}</span>}
    </button>
  );
}

export function Home({ onNavigate }: { onNavigate: (id: string) => void }) {
  const { t } = useTranslation();
  const [running, setRunning] = useState<boolean | null>(null);
  const [port, setPort] = useState<number>(DEFAULT_PORT);
  const [today, setToday] = useState<TodayStats | null>(null);
  const [platformsToday, setPlatformsToday] = useState<TodayPlatformStat[]>([]);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [trendBuckets, setTrendBuckets] = useState<StatsBucket[]>([]);
  const [loading, setLoading] = useState(true);

  // 并行拉取，各区独立 catch 兜底（单 API 失败该区空态，不整页崩）。
  const load = useCallback(async () => {
    // 最近 24 小时 hourly 趋势：now-24h → now 滚动窗口（24 桶）。
    const now = new Date();
    const windowStart = now.getTime() - 24 * 3600 * 1000;
    await Promise.all([
      proxyApi.status().then(setRunning).catch(() => setRunning(null)),
      proxyApi.getSettings().then(s => setPort(s.port)).catch(() => {}),
      trayConfigApi.todayStats().then(setToday).catch(() => setToday(null)),
      popoverConfigApi.platformToday().then(setPlatformsToday).catch(() => setPlatformsToday([])),
      groupDetailApi.list().then(setGroups).catch(() => setGroups([])),
      platformApi.list().then(setPlatforms).catch(() => setPlatforms([])),
      statsApi.query({ start: windowStart, end: now.getTime(), granularity: "hourly" })
        .then(r => setTrendBuckets(r.buckets)).catch(() => setTrendBuckets([])),
    ]);
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);
  // 请求完成后后端 emit "proxy-log-updated" → debounce 重载今日 / 平台用量。
  useEffect(() => onProxyLogUpdated(() => { load(); }), [load]);

  const proxyBaseUrl = `http://127.0.0.1:${port}/proxy`;

  // 今日是否有数据：requests/cost/tokens 任一 > 0。
  const hasTodayData = !!today && (today.total_requests > 0 || today.cost > 0 || today.tokens > 0);

  // 总余额 = 关联平台 est_balance_remaining 求和（平台级属性，无 per-group 概念）。
  const totalBalance = platforms.reduce((acc, p) => acc + (p.est_balance_remaining || 0), 0);
  const enabledCount = platforms.filter(p => p.status === "enabled").length;

  // 平台今日用量 top N（已用 cost 降序）。
  const topPlatforms = [...platformsToday]
    .filter(p => p.cost > 0 || p.tokens > 0 || p.requests > 0)
    .sort((a, b) => b.cost - a.cost)
    .slice(0, TOP_PLATFORMS);
  const maxPlatformCost = topPlatforms.reduce((m, p) => Math.max(m, p.cost), 0);

  // 最近 24 小时请求趋势：各小时桶的 total_requests。峰值 / 总请求用于标注 + 柱高归一化。
  const trendPeak = trendBuckets.reduce((m, b) => Math.max(m, b.total_requests), 0);
  const trendTotal = trendBuckets.reduce((s, b) => s + b.total_requests, 0);
  const hasTrend = trendTotal > 0;

  const statusColor = running == null
    ? "var(--text-tertiary)"
    : running ? "var(--color-success)" : "var(--text-tertiary)";
  const statusText = running == null
    ? t("home.statusUnknown", "未知")
    : running ? t("home.statusRunning", "运行中") : t("home.statusStopped", "已停止");

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {/* Header */}
      <div>
        <div className="section-title">{t("page.home", "首页")}</div>
        <div className="section-desc">{t("home.desc", "代理状态、今日用量与分组平台速览")}</div>
      </div>

      {/* 1. 状态条：代理运行状态 + 端口 + 复制 base_url */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", alignItems: "center", gap: 16, flexWrap: "wrap" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ width: 9, height: 9, borderRadius: "50%", background: statusColor, flexShrink: 0 }} />
          <span style={{ fontSize: F.body, fontWeight: 600 }}>{t("home.proxyStatus", "代理服务")}</span>
          <span style={{ fontSize: F.body, fontWeight: 700, color: statusColor }}>{statusText}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("home.port", "端口")}</span>
          <span style={{ fontSize: F.hint, fontWeight: 600 }}>{port}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginLeft: "auto", minWidth: 0 }}>
          <code style={{ fontSize: F.small, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{proxyBaseUrl}</code>
          <CopyButton text={proxyBaseUrl} label={t("home.copyBaseUrl", "复制代理地址")} title={t("home.copyBaseUrlTitle", "复制代理 base_url")} />
        </div>
      </div>

      {/* 2. 今日概览：StatChip × 4（无数据 → 诚实空态） */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("home.todayTitle", "今日概览")}</div>
        {hasTodayData ? (
          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            <StatChip
              icon={<IconCost size={13} />}
              value={formatCostUsd(today!.cost)}
              label={t("home.cost", "费用")}
              color={levelColor(costLevel(today!.cost))}
            />
            <StatChip icon={<IconBolt size={13} />} value={formatNumber(today!.tokens)} label={t("home.tokens", "Token")} />
            <StatChip icon={<IconLogs size={13} />} value={formatNumber(today!.total_requests)} label={t("home.requests", "请求")} />
            <StatChip icon={<IconPackage size={13} />} value={formatPercent(today!.cache_rate)} label={t("home.cacheRate", "缓存率")} />
          </div>
        ) : (
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", padding: "4px 0" }}>
            {t("home.noToday", "今日暂无请求")}
          </div>
        )}
      </div>

      {/* 3. 请求趋势 · 今日（hourly 曲线图：单 accent 折线 + 面积填充） */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("home.trendTitle", "请求趋势 · 今日")}</div>
          {hasTrend && (
            <div style={{ display: "flex", gap: 14, fontSize: F.small, color: "var(--text-tertiary)" }}>
              <span>{t("home.trendPeak", "峰值")} <span style={{ fontWeight: 700, color: "var(--text-secondary)" }}>{formatNumber(trendPeak)}</span></span>
              <span>{t("home.trendTotal", "总请求")} <span style={{ fontWeight: 700, color: "var(--text-secondary)" }}>{formatNumber(trendTotal)}</span></span>
            </div>
          )}
        </div>
        {hasTrend ? (
          (() => {
            // SVG 曲线图：viewBox 固定坐标系，preserveAspectRatio=none 横向拉满容器，纵向固定高
            const W = 1000;            // viewBox 宽（任意单位，等比映射到容器宽）
            const H = 80;              // viewBox 高
            const PAD_T = 6;           // 顶部留白（圆点不被裁）
            const n = trendBuckets.length;
            const plotH = H - PAD_T;
            const xAt = (i: number) => n > 1 ? (i / (n - 1)) * W : W / 2;
            const yAt = (v: number) => PAD_T + (trendPeak > 0 ? (1 - v / trendPeak) : 1) * plotH;
            const pts = trendBuckets.map((b, i) => ({ x: xAt(i), y: yAt(b.total_requests), b }));
            // Catmull-Rom → 三次贝塞尔 平滑曲线（张力 0.5 经典），控制点 y clamp 到 [PAD_T, H] 防过冲
            const clampY = (v: number) => Math.min(H, Math.max(PAD_T, v));
            const smoothPath = (points: { x: number; y: number }[]): string => {
              if (points.length === 0) return "";
              if (points.length < 3) {
                // 单点/双点 → 退化为直线（不崩）
                return points.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
              }
              let d = `M ${points[0].x.toFixed(1)},${points[0].y.toFixed(1)}`;
              for (let i = 0; i < points.length - 1; i++) {
                const p0 = points[i === 0 ? 0 : i - 1];
                const p1 = points[i];
                const p2 = points[i + 1];
                const p3 = points[i + 2 < points.length ? i + 2 : points.length - 1];
                // 张力 0.5：控制点 = 端点 ± (相邻点切线)/6
                const c1x = p1.x + (p2.x - p0.x) / 6;
                const c1y = clampY(p1.y + (p2.y - p0.y) / 6);
                const c2x = p2.x - (p3.x - p1.x) / 6;
                const c2y = clampY(p2.y - (p3.y - p1.y) / 6);
                d += ` C ${c1x.toFixed(1)},${c1y.toFixed(1)} ${c2x.toFixed(1)},${c2y.toFixed(1)} ${p2.x.toFixed(1)},${p2.y.toFixed(1)}`;
              }
              return d;
            };
            const linePath = smoothPath(pts);
            const areaPath = `${linePath} L ${pts[n - 1].x.toFixed(1)},${H} L ${pts[0].x.toFixed(1)},${H} Z`;
            const peakIdx = pts.reduce((mi, p, i) => p.b.total_requests > pts[mi].b.total_requests ? i : mi, 0);
            return (
              <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" style={{ width: "100%", height: 80, display: "block", overflow: "visible" }}>
                  <defs>
                    <linearGradient id="homeTrendArea" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.28" />
                      <stop offset="100%" stopColor="var(--accent)" stopOpacity="0.02" />
                    </linearGradient>
                  </defs>
                  <path d={areaPath} fill="url(#homeTrendArea)" />
                  <path
                    d={linePath}
                    fill="none"
                    stroke="var(--accent)"
                    strokeWidth={2}
                    strokeLinejoin="round"
                    strokeLinecap="round"
                    vectorEffect="non-scaling-stroke"
                  />
                  {/* hover 命中区 + tooltip（每桶一竖条，透明） */}
                  {pts.map((p, i) => (
                    <rect
                      key={i}
                      x={(p.x - W / (n * 2)).toFixed(1)}
                      y={0}
                      width={(W / n).toFixed(1)}
                      height={H}
                      fill="transparent"
                    >
                      <title>{`${p.b.time_bucket.slice(-5)} · ${formatNumber(p.b.total_requests)}`}</title>
                    </rect>
                  ))}
                  {/* 峰值点高亮（克制，单点） */}
                  {trendPeak > 0 && (
                    <circle cx={pts[peakIdx].x.toFixed(1)} cy={pts[peakIdx].y.toFixed(1)} r={3.5} fill="var(--accent)" vectorEffect="non-scaling-stroke" />
                  )}
                </svg>
                {/* x 轴整点小时标注：每 4 桶，与曲线 x 对齐 */}
                <div style={{ position: "relative", height: 12 }}>
                  {trendBuckets.map((b, i) =>
                    i % 4 === 0 ? (
                      <span
                        key={i}
                        style={{
                          position: "absolute",
                          left: `${(xAt(i) / W) * 100}%`,
                          transform: "translateX(-50%)",
                          fontSize: 8,
                          color: "var(--text-tertiary)",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {b.time_bucket.slice(-5).slice(0, 2)}
                      </span>
                    ) : null
                  )}
                </div>
              </div>
            );
          })()
        ) : (
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", padding: "4px 0" }}>
            {t("home.trendEmpty", "今日暂无请求")}
          </div>
        )}
      </div>

      {/* 4. 分组 / 平台速览 + 总余额 + 平台今日用量 top N */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 14 }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("home.overviewTitle", "分组 / 平台速览")}</div>
        <div style={{ display: "flex", gap: 24, flexWrap: "wrap", alignItems: "flex-start" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            <span style={{ fontSize: F.title, fontWeight: 700 }}>{groups.length}</span>
            <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("home.groups", "分组")}</span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            <span style={{ fontSize: F.title, fontWeight: 700 }}>{platforms.length}</span>
            <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>
              {t("home.platforms", "平台")}
              {platforms.length > 0 && (
                <span style={{ marginLeft: 4 }}>{t("home.enabledCount", "{{count}} 启用", { count: enabledCount })}</span>
              )}
            </span>
          </div>
          {totalBalance > 0 && (
            <div style={{ minWidth: 160, display: "flex", flexDirection: "column", gap: 4 }}>
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", display: "inline-flex", alignItems: "center", gap: 4 }}>
                <IconCard size={12} /> {t("home.totalBalance", "总余额")}
              </span>
              <BalanceBar remaining={totalBalance} />
            </div>
          )}
        </div>

        {/* 平台今日用量 top N */}
        {topPlatforms.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 8, borderTop: "1px solid var(--border)", paddingTop: 12 }}>
            <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("home.topPlatforms", "今日平台用量")}</span>
            {topPlatforms.map(p => (
              <div key={p.platform_id} style={{ display: "flex", flexDirection: "column", gap: 3 }}>
                <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
                  <span style={{ fontSize: F.hint, fontWeight: 500, flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.platform_name}</span>
                  <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{formatNumber(p.requests)} · {formatNumber(p.tokens)}</span>
                  <span style={{ fontSize: F.hint, fontWeight: 700, color: levelColor(costLevel(p.cost)) }}>{formatCostUsd(p.cost)}</span>
                </div>
                <div style={{ height: 4, borderRadius: "var(--radius-sm)", background: "var(--bg-glass)", overflow: "hidden" }}>
                  <div style={{ width: `${maxPlatformCost > 0 ? (p.cost / maxPlatformCost) * 100 : 0}%`, height: "100%", background: "var(--accent)", borderRadius: "var(--radius-sm)", transition: "width 0.3s ease" }} />
                </div>
              </div>
            ))}
          </div>
        )}
        {!loading && groups.length === 0 && platforms.length === 0 && (
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("home.noPlatforms", "暂无分组或平台")}</div>
        )}
      </div>

      {/* 5. 快捷操作 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("home.quickActions", "快捷操作")}</div>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          <button className="btn" style={{ gap: 6, fontSize: 13 }} onClick={() => onNavigate("platforms")}>
            <IconPlatforms size={15} /> {t("home.addPlatform", "添加平台")}
          </button>
          <button className="btn" style={{ gap: 6, fontSize: 13 }} onClick={() => onNavigate("stats")}>
            <IconStats size={15} /> {t("home.viewStats", "查看统计")}
          </button>
          <button className="btn" style={{ gap: 6, fontSize: 13 }} onClick={() => onNavigate("logs")}>
            <IconLogs size={15} /> {t("home.viewLogs", "查看日志")}
          </button>
          <CopyButton text={proxyBaseUrl} label={t("home.copyBaseUrl", "复制代理地址")} title={t("home.copyBaseUrlTitle", "复制代理 base_url")} />
        </div>
      </div>
    </div>
  );
}
