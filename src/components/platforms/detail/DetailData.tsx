// T3 数据板块（详情页）：B/B' 曲线、C 使用统计（四档窗口）、D 健康明细、E 失败分析、
// G 模型分布；F 水位板块在 WaterLevel（T5，额度窗口逐条水位；限频占位命令侧不可达未含）。
// 数据映射纯函数在 chartMath；曲线/柱图为纯渲染视图组件。
import { useEffect, useMemo, useState } from "react";
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

export function DetailData({ platform: p }: { platform: Platform }) {
  const { t } = useTranslation();
  const [win, setWin] = useState<Win>("7d");
  const [stats, setStats] = useState<PlatformUsageStats | undefined>();
  const [series, setSeries] = useState<PlatformUsageSeriesPoint[]>([]);
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
    platformApi.usageSeries(p.id, "7d").then(s => { if (alive && Array.isArray(s)) setSeries(s); }).catch(() => {});
    platformApi.healthDetail(p.id).then(h => { if (alive && h) setHealth(h); }).catch(() => {});
    proxyLogApi.listFiltered({ platform_id: p.id, status: -1, time_start: Date.now() - 7 * 86_400_000 }, 200, 0)
      .then(pg => { if (alive && pg && Array.isArray(pg.items)) setFails(pg.items); })
      .catch(() => {});
    return () => { alive = false; };
  }, [p.id]);

  // 订阅平台：一次官方 quota 查询（快照校准点 / resets_at 窗口锚点；失败静默，回落 est 档位）
  useEffect(() => {
    if (!isSubscription) return;
    let alive = true;
    quotaApi.query(primaryBaseUrl(p), p.api_key, p.id)
      .then(q => { if (alive && q && q.success) setQuota(q); })
      .catch(() => {});
    return () => { alive = false; };
  }, [p.id, isSubscription]);

  const curve: SubscriptionCurve | null = useMemo(() => {
    if (!isSubscription) return null;
    const quotaTiers: CurveTierLike[] = (quota?.coding_plan?.tiers ?? []).map(tr => ({
      name: tr.name, utilization: tr.utilization, limit: tr.limit, resets_at: tr.resets_at,
    }));
    const estTiers: CurveTierLike[] = (estPlan?.tiers ?? []).map(tr => ({
      name: tr.name, est_utilization: tr.est_utilization, limit: tr.limit ?? null, window_start: tr.window_start,
    }));
    return buildSubscriptionCurve({
      series, quotaTiers,
      quotaQueriedAt: quota?.queried_at ?? null,
      estTiers, nowMs: Date.now(),
    });
  }, [isSubscription, series, quota, estPlan]);

  const bars: DailyBar[] = useMemo(() => buildDailyBars(series), [series]);
  const failStats = useMemo(() => classifyFailures(fails), [fails]);
  const modelRows = useMemo(() => topModels(models, 8), [models]);

  return (
    <div>
      {/* B / B'：曲线 或 按量柱线图 */}
      <Blk
        testid="detail-t3-usage"
        title={isSubscription ? t("platform.detail.curve", "订阅消耗曲线") : t("platform.detail.paygo", "近 7 天用量")}
        hint={isSubscription
          ? t("platform.detail.curveWindowHint", "本窗口 7 天")
          : formatMoney(bars.reduce((s, b) => s + b.cost, 0))}
      >
        {!isSubscription && p.est_balance_remaining > 0 && (
          <div data-testid="detail-t3-balance" style={{ fontSize: 20, fontWeight: 600, marginBottom: 8 }}>
            {formatMoney(p.est_balance_remaining)}
            <span style={{ fontSize: 14, fontWeight: 400, color: "var(--muted-foreground)", marginLeft: 8 }}>
              {t("platform.detail.balance", "余额")}
            </span>
          </div>
        )}
        {isSubscription
          ? (curve
              ? <SubscriptionCurveView curve={curve} nowMs={Date.now()} />
              : <Empty t={t} />)
          : (bars.length > 0 ? <PaygoBarsView bars={bars} /> : <Empty t={t} />)}
      </Blk>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))", alignItems: "start" }}>
        {/* C：使用统计（四档窗口） */}
        <Blk testid="detail-t3-stats" title={t("platform.detail.stats", "使用统计")}>
          <div style={{ display: "flex", gap: 4, marginBottom: 10 }}>
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

        {/* D：健康状态 */}
        <HealthBlock t={t} health={health} />

        {/* E：失败分析 */}
        <Blk testid="detail-t3-fail" title={t("platform.detail.failures", "失败分析")}
          hint={t("platform.detail.win.7d", "近 7 天")}>
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
                  <th style={thStyle}>{t("platform.detail.colTime", "时间")}</th>
                  <th style={thStyle}>{t("platform.detail.colModel", "模型")}</th>
                  <th style={thStyle}>{t("platform.detail.colReason", "原因")}</th>
                  <th style={{ ...thStyle, textAlign: "right" }}>{t("platform.detail.colStatus", "状态码")}</th>
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

        {/* G：模型用量分布 */}
        <Blk testid="detail-t3-models" title={t("platform.detail.modelDist", "模型用量分布")} hint={t(`platform.detail.win.${win}`, WIN_FALLBACK[win])}>          {modelRows.length === 0
            ? <Empty t={t} />
            : modelRows.map(r => (
              <div key={r.model} data-testid="detail-t3-model-row"
                style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14, padding: "3px 0" }}>
                <span style={{ width: 130, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  fontFamily: "var(--font-mono, monospace)" }} title={r.model}>{r.model}</span>
                <div style={{ flex: 1, height: 8, borderRadius: 4, background: "var(--muted)", overflow: "hidden" }}>
                  <div style={{ width: `${r.pct}%`, height: "100%", background: "var(--data-primary)" }} />
                </div>
                <span style={{ width: 56, textAlign: "right", color: "var(--muted-foreground)", fontFamily: "var(--font-mono, monospace)" }}>
                  {formatNumber(r.requests)} · {r.pct}%
                </span>
              </div>
            ))}
        </Blk>

        {/* F：限频与额度窗口水位（T5） */}
        <WaterLevel platformId={p.id} />
      </div>
    </div>
  );
}

const thStyle = { fontWeight: 400, paddingBottom: 4 } as const;
const tdStyle = { padding: "4px 0", color: "var(--text-secondary)" } as const;

const WIN_FALLBACK: Record<Win, string> = {
  today: "今日", "7d": "近 7 天", "30d": "近 30 天", all: "累计",
};

function StatRows({ t, stats }: { t: (k: string, d: string) => string; stats?: PlatformUsageStats }) {
  const rows: Array<[string, string]> = stats
    ? [
        [t("platform.detail.requests", "请求数"), formatNumber(stats.total_requests)],
        [t("platform.detail.successRate", "成功率"), formatPercent(
          stats.total_requests > 0 ? stats.success_count / stats.total_requests : 0)],
        [t("platform.detail.inputTokens", "输入 token"), formatNumber(stats.total_input_tokens)],
        [t("platform.detail.outputTokens", "输出 token"), formatNumber(stats.total_output_tokens)],
        [t("platform.detail.cacheTokens", "缓存读 token"), formatNumber(stats.total_cache_tokens)],
        [t("platform.detail.estCost", "估算费用"), formatMoney(stats.total_cost)],
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
        {secs(s) ?? t("platform.detail.noCooldown", "无")}
      </span>
    </div>
  );
  return (
    <Blk testid="detail-t3-health" title={t("platform.detail.health", "健康状态")} hint={t("platform.detail.live", "实时")}>
      {!health ? <Empty t={t} /> : (
        <div>
          <div data-testid="detail-t3-breaker" style={{ fontSize: 14, padding: "3px 0", display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: "var(--text-secondary)" }}>{t("platform.detail.breakerState", "熔断状态")}</span>
            <span style={{ color: health.breaker_open ? "var(--color-danger)" : "var(--color-success)" }}>
              {health.breaker_open
                ? t("platform.detail.breakerOpen", "已触发 · 连续失败 {{n}}").replace("{{n}}", `${health.breaker_fails}`)
                : t("platform.detail.breakerOk", "未触发 · 连续失败 {{n}}").replace("{{n}}", `${health.breaker_fails}`)}
            </span>
          </div>
          {cd(t("platform.detail.cdQuota", "配额冷却"), health.quota_cooldown_secs)}
          {cd(t("platform.detail.cdVerdict", "额度判定冷却"), health.quota_verdict_secs)}
          {cd(t("platform.detail.cdAuth", "鉴权冷却"), health.auth_cooldown_secs)}
          {cd(t("platform.detail.cdBalance", "余额冷却"), health.balance_cooldown_secs)}
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 14, padding: "3px 0" }}>
            <span style={{ color: "var(--text-secondary)" }}>{t("platform.detail.peakDisabled", "高峰禁用")}</span>
            <span style={{ color: health.peak_disabled ? "var(--color-warning)" : "var(--muted-foreground)" }}>
              {health.peak_disabled ? t("platform.detail.on", "启用中") : t("platform.detail.off", "未启用")}
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
const FAIL_FALLBACK: Record<FailClass, string> = { service: "服务类", model: "模型类", rate: "限频", auth: "鉴权" };
const FAIL_COLOR: Record<FailClass, string> = {
  service: "var(--color-danger)", model: "var(--color-warning)",
  rate: "var(--data-primary)", auth: "var(--muted-foreground)",
};

function Empty({ t }: { t: (k: string, d: string) => string }) {
  return <div style={{ fontSize: 14, color: "var(--muted-foreground)", padding: "8px 0" }}>
    {t("platform.detail.noData", "暂无数据")}
  </div>;
}
