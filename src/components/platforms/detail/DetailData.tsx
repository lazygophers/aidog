// T3 数据板块（详情页）：B/B' 曲线、C 使用统计（四档窗口）、D 健康明细、E 失败分析、
// G 模型分布；F 水位板块在 WaterLevel（T5，额度窗口逐条水位；限频占位命令侧不可达未含）。
// 数据映射纯函数在 chartMath；曲线/柱图为纯渲染视图组件。
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { Platform, PlatformUsageStats, PlatformUsageSeriesPoint, PlatformHealthDetail,
  PlatformModelDistributionRow, PlatformQuota, ProxyLogSummary } from "../../../services/api";
import { platformApi, quotaApi, proxyLogApi } from "../../../services/api";
import { parseEstCodingPlan } from "../../../domains/platforms";
import { Blk } from "./Blk";
import { WaterLevel } from "./WaterLevel";
import { SubscriptionCurveView } from "./SubscriptionCurve";
import { PaygoBarsView } from "./PaygoBars";
import {
  buildSubscriptionCurve, buildDailyBars, classifyFailures, topModels,
  type CurveTierLike, type SubscriptionCurve, type DailyBar, type FailClass,
} from "./chartMath";
import { formatNumber, formatMoney, formatPercent, formatDurationMs } from "../../../utils/formatters";

type Win = "today" | "7d" | "30d" | "all";
const WINS: Win[] = ["today", "7d", "30d", "all"];

const primaryBaseUrl = (p: Platform): string => {
  const eps = p.endpoints ?? [];
  if (eps.length === 0) return p.base_url;
  const primary = eps.find(ep => ep.protocol === p.platform_type);
  return primary?.base_url || eps[0]?.base_url || p.base_url;
};

export function DetailData({ platform: p, col3Extra }: { platform: Platform; col3Extra?: ReactNode }) {
  const { t } = useTranslation();
  const [win, setWin] = useState<Win>("7d");
  const [stats, setStats] = useState<PlatformUsageStats | undefined>();
  const [series, setSeries] = useState<PlatformUsageSeriesPoint[]>([]);
  const [seriesWindowStart, setSeriesWindowStart] = useState<number | null>(null);
  const [seriesQueriedAt, setSeriesQueriedAt] = useState<number | null>(null);
  const [health, setHealth] = useState<PlatformHealthDetail | undefined>();
  const [models, setModels] = useState<PlatformModelDistributionRow[]>([]);
  const [fails, setFails] = useState<ProxyLogSummary[]>([]);
  const [quota, setQuota] = useState<PlatformQuota | null>(null);

  const estPlan = useMemo(() => parseEstCodingPlan(p.est_coding_plan), [p.est_coding_plan]);
  const isSubscription = p.platform_type === "claude_code" || (estPlan !== null && estPlan.tiers.length > 0);

  // 窗口相关：统计 + 模型分布
  useEffect(() => {
    let alive = true;
    platformApi.usageStats(p.id, win).then(s => { if (alive && s) setStats(s); }).catch(() => {});
    platformApi.modelDistribution(p.id, win).then(m => { if (alive && Array.isArray(m)) setModels(m); }).catch(() => {});
    return () => { alive = false; };
  }, [p.id, win]);

  // 一次性：7 天序列（曲线/柱图）、健康明细、失败清单（近 7 天）
  useEffect(() => {
    let alive = true;
    setSeries([]);
    setSeriesWindowStart(null);
    setSeriesQueriedAt(null);
    // 7d API lower bound is hour-floored; +1h is the first safely complete bucket boundary.
    const seriesWindowStart = Date.now() - 7 * 86_400_000 + 3_600_000;
    platformApi.usageSeries(p.id, "7d").then(s => {
      if (alive && Array.isArray(s)) {
        setSeries(s);
        setSeriesWindowStart(seriesWindowStart);
        setSeriesQueriedAt(Date.now());
      }
    }).catch(() => {});
    platformApi.healthDetail(p.id).then(h => { if (alive && h) setHealth(h); }).catch(() => {});
    proxyLogApi.listFiltered({ platform_id: p.id, status: -1, time_start: Date.now() - 7 * 86_400_000 }, 200, 0)
      .then(pg => { if (alive && pg && Array.isArray(pg.items)) setFails(pg.items); })
      .catch(() => {});
    return () => { alive = false; };
  }, [p.id]);

  // 订阅平台：一次官方 quota 查询（快照校准点 / resets_at 窗口锚点；失败静默，回落 est 档位）
  useEffect(() => {
    let alive = true;
    setQuota(null);
    if (!isSubscription) return () => { alive = false; };
    quotaApi.query(primaryBaseUrl(p), p.api_key, p.id)
      .then(q => { if (alive && q && q.success) setQuota(q); })
      .catch(() => {});
    return () => { alive = false; };
  }, [p.id, isSubscription]);

  const curve: SubscriptionCurve | null = useMemo(() => {
    if (!isSubscription) return null;
    const quotaTiers: CurveTierLike[] = (quota?.coding_plan?.tiers ?? []).map(tr => ({
      name: tr.name, unit: tr.unit ?? null, utilization: tr.utilization, limit: tr.limit, resets_at: tr.resets_at,
    }));
    const estTiers: CurveTierLike[] = (estPlan?.tiers ?? []).map(tr => ({
      name: tr.name, unit: tr.unit ?? null, est_utilization: tr.est_utilization,
      limit: tr.has_base ? tr.limit ?? null : null, window_start: tr.window_start,
    }));
    return buildSubscriptionCurve({
      series, seriesWindowStart, seriesQueriedAt, quotaTiers,
      quotaQueriedAt: quota?.queried_at ?? null,
      estTiers, nowMs: Date.now(),
    });
  }, [isSubscription, series, seriesWindowStart, seriesQueriedAt, quota, estPlan]);

  const bars: DailyBar[] = useMemo(() => buildDailyBars(series), [series]);
  const failStats = useMemo(() => classifyFailures(fails), [fails]);
  const modelRows = useMemo(() => topModels(models, 8), [models]);

  return (
    <div data-testid="detail-t3-mount" className="contents">
      <div className="min-w-0 border-r border-[var(--border)] max-xl:border-r-0">
        {/* C：使用统计（四档窗口） */}
        <Blk testid="detail-t3-stats" title={t("platform.detail.stats", "Usage Stats")}>
          <div style={{ display: "flex", gap: 4, marginBottom: 10, flexWrap: "wrap" }}>
            {WINS.map(w => (
              <button
                key={w}
                data-testid={`detail-t3-win-${w}`}
                onClick={() => setWin(w)}
                style={{
                  fontSize: 14, padding: "3px 10px", cursor: "pointer", borderRadius: 4,
                  border: `1px solid ${win === w ? "var(--accent)" : "var(--border)"}`,
                  background: win === w ? "var(--accent-subtle)" : "transparent",
                  color: win === w ? "var(--accent)" : "var(--muted-foreground)",
                }}
              >
                {t(`platform.detail.win.${w}`, WIN_FALLBACK[w])}
              </button>
            ))}
          </div>
          <StatRows t={t} stats={stats} />
        </Blk>
        <Blk testid="detail-t3-fail" title={t("platform.detail.failures", "Failure Analysis")}
          hint={t("platform.detail.win.7d", "Last 7 days")}>
          <div style={{ display: "flex", flexDirection: "column", gap: 6, marginBottom: 10 }}>
            {FAIL_CLASSES.map(cls => {
              const v = failStats.counts[cls];
              const total = Math.max(Object.values(failStats.counts).reduce((a, b) => a + b, 0), 1);
              return (
                <div key={cls} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14 }}>
                  <span style={{ width: 44, color: "var(--text-secondary)" }}>{t(FAIL_LABEL[cls], FAIL_FALLBACK[cls])}</span>
                  <div style={{ flex: 1, height: 8, borderRadius: 4, background: "var(--muted)", overflow: "hidden" }}>
                    <div style={{ width: `${(v / total) * 100}%`, height: "100%", background: FAIL_COLOR[cls] }} />
                  </div>
                  <span style={{ width: 28, textAlign: "right", fontFamily: "var(--font-mono, monospace)" }}>{v}</span>
                </div>
              );
            })}
          </div>
          {failStats.recent.length > 0 && (
            <table style={{ width: "100%", fontSize: 14, borderCollapse: "collapse" }}>
              <thead>
                <tr style={{ color: "var(--muted-foreground)", textAlign: "left" }}>
                  <th style={thStyle}>{t("platform.detail.colTime", "Time")}</th>
                  <th style={thStyle}>{t("platform.detail.colModel", "Model")}</th>
                  <th style={thStyle}>{t("platform.detail.colReason", "Reason")}</th>
                  <th style={{ ...thStyle, textAlign: "right" }}>{t("platform.detail.colStatus", "Status")}</th>
                </tr>
              </thead>
              <tbody>
                {failStats.recent.map((r, i) => (
                  <tr key={`${r.created_at}-${i}`} style={{ borderTop: "1px solid var(--border)" }}>
                    <td style={tdStyle}>{new Date(r.created_at).toLocaleString()}</td>
                    <td style={{ ...tdStyle, fontFamily: "var(--font-mono, monospace)" }}>{r.model || "-"}</td>
                    <td style={tdStyle}>{t(FAIL_LABEL[r.cls], FAIL_FALLBACK[r.cls])}</td>
                    <td style={{ ...tdStyle, textAlign: "right", fontFamily: "var(--font-mono, monospace)" }}>{r.status_code}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Blk>
      </div>
      <div className="min-w-0 border-r border-[var(--border)] max-xl:border-r-0">
        {/* B / B'：曲线 或 按量柱线图 */}
      <Blk
        testid="detail-t3-usage"
        title={isSubscription ? t("platform.detail.curve", "Subscription Usage Curve") : t("platform.detail.paygo", "Last 7 Days Usage")}
        hint={isSubscription
          ? t("platform.detail.curveWindowHint", "Current window: 7 days")
          : formatMoney(bars.reduce((s, b) => s + b.cost, 0))}
      >
        {!isSubscription && p.est_balance_remaining > 0 && (
          <div data-testid="detail-t3-balance" style={{ fontSize: 20, fontWeight: 600, marginBottom: 8 }}>
            {formatMoney(p.est_balance_remaining)}
            <span style={{ fontSize: 14, fontWeight: 400, color: "var(--muted-foreground)", marginLeft: 8 }}>
              {t("platform.detail.balance", "Balance")}
            </span>
          </div>
        )}
        {isSubscription
          ? (curve
              ? <SubscriptionCurveView curve={curve} nowMs={Date.now()} />
              : <Empty t={t} />)
          : (bars.length > 0 ? <PaygoBarsView bars={bars} /> : <Empty t={t} />)}
      </Blk>

        <WaterLevel platformId={p.id} extra={p.extra} />
      </div>
      <div className="min-w-0 sm:col-span-2 xl:col-span-1">
        {/* D：健康状态 */}
        <HealthBlock t={t} health={health} />


        {/* G：模型用量分布 */}
        <Blk testid="detail-t3-models" title={t("platform.detail.modelDist", "Model Usage Distribution")} hint={t(`platform.detail.win.${win}`, WIN_FALLBACK[win])}>          {modelRows.length === 0
            ? <Empty t={t} />
            : modelRows.map(r => (
              <div key={r.model} data-testid="detail-t3-model-row"
                className="grid grid-cols-[minmax(80px,130px)_minmax(24px,1fr)_84px] items-center gap-2 py-[3px] text-[14px] max-[640px]:grid-cols-[minmax(0,1fr)_auto]"
              >
                <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  fontFamily: "var(--font-mono, monospace)" }} title={r.model}>{r.model}</span>
                <div className="max-[640px]:col-span-2 max-[640px]:row-start-2" style={{ height: 8, borderRadius: 4, background: "var(--muted)", overflow: "hidden" }}>
                  <div style={{ width: `${r.pct}%`, height: "100%", background: "var(--data-primary)" }} />
                </div>
                <span className="max-[640px]:col-start-2 max-[640px]:row-start-1" style={{ textAlign: "right", whiteSpace: "nowrap", color: "var(--muted-foreground)", fontFamily: "var(--font-mono, monospace)" }}>
                  {formatNumber(r.requests)} · {r.pct}%
                </span>
              </div>
            ))}
        </Blk>

        {/* F：限频与额度窗口水位（T5） */}
        {col3Extra}
      </div>
    </div>
  );
}

const thStyle = { fontWeight: 400, paddingBottom: 4 } as const;
const tdStyle = { padding: "4px 0", color: "var(--text-secondary)" } as const;

const WIN_FALLBACK: Record<Win, string> = {
  today: "Today", "7d": "Last 7 days", "30d": "Last 30 days", all: "All time",
};

function StatRows({ t, stats }: { t: (k: string, d: string) => string; stats?: PlatformUsageStats }) {
  const rows: Array<[string, string]> = stats
    ? [
        [t("platform.detail.requests", "Requests"), formatNumber(stats.total_requests)],
        [t("platform.detail.successRate", "Success rate"), formatPercent(
          stats.total_requests > 0 ? (stats.success_count / stats.total_requests) * 100 : 0)],
        [t("platform.detail.inputTokens", "Input tokens"), formatNumber(stats.total_input_tokens)],
        [t("platform.detail.outputTokens", "Output tokens"), formatNumber(stats.total_output_tokens)],
        [t("platform.detail.cacheTokens", "Cache read tokens"), formatNumber(stats.total_cache_tokens)],
        [t("platform.detail.estCost", "Estimated cost"), formatMoney(stats.total_cost)],
      ]
    : [];
  return (
    <div>
      {rows.length === 0
        ? <Empty t={t} />
        : rows.map(([k, v]) => (
          <div key={k} data-testid="detail-t3-stat-row"
            style={{ display: "flex", justifyContent: "space-between", fontSize: 14, padding: "4px 0" }}>
            <span style={{ color: "var(--text-secondary)" }}>{k}</span>
            <span style={{ fontFamily: "var(--font-mono, monospace)" }}>{v}</span>
          </div>
        ))}
    </div>
  );
}

function HealthBlock({ t, health }: { t: (k: string, d: string) => string; health?: PlatformHealthDetail }) {
  const secs = (s: number) => (s > 0 ? formatDurationMs(s * 1000) : null);
  const cd = (label: string, s: number) => (
    <div data-testid="detail-t3-cd-row" style={{ display: "flex", justifyContent: "space-between", fontSize: 14, padding: "3px 0" }}>
      <span style={{ color: "var(--text-secondary)" }}>{label}</span>
      <span style={{ color: s > 0 ? "var(--color-warning)" : "var(--muted-foreground)" }}>
        {secs(s) ?? t("platform.detail.noCooldown", "None")}
      </span>
    </div>
  );
  return (
    <Blk testid="detail-t3-health" title={t("platform.detail.health", "Health")} hint={t("platform.detail.live", "Live")}>
      {!health ? <Empty t={t} /> : (
        <div>
          <div data-testid="detail-t3-breaker" style={{ fontSize: 14, padding: "3px 0", display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: "var(--text-secondary)" }}>{t("platform.detail.breakerState", "Circuit breaker")}</span>
            <span style={{ color: health.breaker_open ? "var(--color-danger)" : "var(--color-success)" }}>
              {health.breaker_open
                ? t("platform.detail.breakerOpen", "Open · {{n}} consecutive failures").replace("{{n}}", `${health.breaker_fails}`)
                : t("platform.detail.breakerOk", "Closed · {{n}} consecutive failures").replace("{{n}}", `${health.breaker_fails}`)}
            </span>
          </div>
          {cd(t("platform.detail.cdQuota", "Quota cooldown"), health.quota_cooldown_secs)}
          {cd(t("platform.detail.cdVerdict", "Quota verdict cooldown"), health.quota_verdict_secs)}
          {cd(t("platform.detail.cdAuth", "Auth cooldown"), health.auth_cooldown_secs)}
          {cd(t("platform.detail.cdBalance", "Balance cooldown"), health.balance_cooldown_secs)}
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 14, padding: "3px 0" }}>
            <span style={{ color: "var(--text-secondary)" }}>{t("platform.detail.peakDisabled", "Peak disable")}</span>
            <span style={{ color: health.peak_disabled ? "var(--color-warning)" : "var(--muted-foreground)" }}>
              {health.peak_disabled ? t("platform.detail.on", "Active") : t("platform.detail.off", "Inactive")}
            </span>
          </div>
        </div>
      )}
    </Blk>
  );
}

const FAIL_CLASSES: FailClass[] = ["service", "model", "rate", "auth"];
const FAIL_LABEL: Record<FailClass, string> = {
  service: "platform.detail.failService",
  model: "platform.detail.failModel",
  rate: "platform.detail.failRate",
  auth: "platform.detail.failAuth",
};
const FAIL_FALLBACK: Record<FailClass, string> = { service: "Service", model: "Model", rate: "Rate limit", auth: "Auth" };
const FAIL_COLOR: Record<FailClass, string> = {
  service: "var(--color-danger)", model: "var(--color-warning)",
  rate: "var(--data-primary)", auth: "var(--muted-foreground)",
};

function Empty({ t }: { t: (k: string, d: string) => string }) {
  return <div style={{ fontSize: 14, color: "var(--muted-foreground)", padding: "8px 0" }}>
    {t("platform.detail.noData", "No data")}
  </div>;
}
