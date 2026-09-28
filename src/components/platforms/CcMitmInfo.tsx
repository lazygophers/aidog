// ── Claude Code 订阅透传 MITM 信息（cc-sub-mitm 票 12）──
// 平台卡片消费：① 头部 OAuth refresh 失败警示 chip（#91703）② 展开明细里的
// 5h/7d 窗口利用率趋势 + 最新窗口值。数据打开卡片时拉一次（mitm_stats 开关才激活），
// 无轮询。行映射 buildUsageTrendRows 导出供单测。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { mitmStatsApi, type OauthUsageSample, type MitmRefreshStats, type CcProfile } from "../../services/api";
import { formatPercent } from "../../utils/formatters";
import { LineChart } from "../charts";
import type { ChartConfig } from "@/components/ui/chart";

export interface CcMitmInfoData {
  samples: OauthUsageSample[];
  refresh: MitmRefreshStats | null;
  plan: CcProfile | null;
}

/** 采样点 → LineChart 宽表行（x = sampled_at ms，两条 0-100 百分数线）。 */
export function buildUsageTrendRows(samples: OauthUsageSample[]): Record<string, unknown>[] {
  return samples.map(s => ({ x: s.sampled_at, five: s.five_hour_pct, seven: s.seven_day_pct }));
}

/** 激活时拉一次趋势 + refresh 计数（groupName 缺失 → 只拉 refresh，趋势留空）。 */
export function useCcMitmInfo(active: boolean, groupName?: string): CcMitmInfoData | null {
  const [data, setData] = useState<CcMitmInfoData | null>(null);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const tasks: Promise<void>[] = [
      mitmStatsApi.refreshStats(0)
        .then(refresh => { if (!cancelled) setData(d => ({ samples: d?.samples ?? [], refresh, plan: d?.plan ?? null })); })
        .catch(() => { if (!cancelled) setData(d => d ?? { samples: [], refresh: null, plan: null }); }),
    ];
    if (groupName) {
      tasks.push(
        mitmStatsApi.usageTrend(groupName)
          .then(samples => { if (!cancelled) setData(d => ({ samples, refresh: d?.refresh ?? null, plan: d?.plan ?? null })); })
          .catch(() => {}),
      );
      // 套餐档位（balance-full）：与趋势同一次打开拉一次，被动数据、无轮询。
      tasks.push(
        mitmStatsApi.planInfo(groupName)
          .then(plan => { if (!cancelled) setData(d => ({ samples: d?.samples ?? [], refresh: d?.refresh ?? null, plan })); })
          .catch(() => {}),
      );
    }
    return () => { cancelled = true; };
  }, [active, groupName]);
  return active ? data : null;
}

/** 头部警示 chip：token refresh 有失败才渲染（title 展开 #91703 应对文案）。 */
export function CcMitmRefreshWarning({ refresh }: { refresh: MitmRefreshStats }) {
  const { t } = useTranslation();
  return (
    <div
      style={{
        marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4, maxWidth: "100%",
        fontSize: 10, fontWeight: 600, color: "var(--color-warning)",
        background: "color-mix(in srgb, var(--color-warning) 14%, transparent)",
        border: "1px solid color-mix(in srgb, var(--color-warning) 35%, transparent)",
        borderRadius: 5, padding: "1px 6px",
      }}
      title={t("platform.mitmRefreshWarnHint",
        "platform.claude.com 令牌刷新过代理可能失败（Claude Code 已知 bug #91703，未修复）。{{fails}}/{{total}} 次失败。\n应对：临时在终端 export NO_PROXY=platform.claude.com 后重启 Claude Code（该域名流量不经代理、不统计），或等待上游修复。",
        { fails: refresh.failures, total: refresh.attempts })}
    >
      {t("platform.mitmRefreshWarn", "订阅刷新失败 {{fails}}/{{total}}",
        { fails: refresh.failures, total: refresh.attempts })}
    </div>
  );
}

const trendConfig: ChartConfig = {
  five: { label: "5h" },
  seven: { label: "7d" },
};

/** 展开明细：最新窗口利用率 + 趋势线（≥2 个采样点才画图）。 */
export function CcMitmTrendSection({ samples }: { samples: OauthUsageSample[] }) {
  const { t } = useTranslation();
  const latest = samples.length > 0 ? samples[samples.length - 1] : null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>
        {t("platform.mitmWindowsLabel", "订阅窗口利用率")}
      </span>
      {latest ? (
        <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap", fontSize: 12, color: "var(--text-secondary)" }}>
          <span>5h <strong style={{ color: "var(--text-primary)" }}>{formatPercent(latest.five_hour_pct)}</strong></span>
          <span>7d <strong style={{ color: "var(--text-primary)" }}>{formatPercent(latest.seven_day_pct)}</strong></span>
        </div>
      ) : (
        <span className="text-tertiary" style={{ fontSize: 11 }}>
          {t("platform.mitmWindowsEmpty", "暂无采样（Claude Code 自然请求 usage 端点后出现，不主动轮询）")}
        </span>
      )}
      {samples.length >= 2 && (
        <LineChart
          config={trendConfig}
          data={buildUsageTrendRows(samples)}
          height={110}
          mini
          valueFormat={n => formatPercent(n, 0)}
        />
      )}
    </div>
  );
}

/** 余额位块（balance-full）：套餐档位 + 5h/7d 剩余额度（剩余 = 100 − 最新采样利用率）。
 *  数据全被动（profile/usage 均蹭自然流量采样），无任一数据时不渲染。 */
export function CcPlanBalance({ plan, samples }: { plan: CcProfile | null; samples: OauthUsageSample[] }) {
  const { t } = useTranslation();
  const latest = samples.length > 0 ? samples[samples.length - 1] : null;
  if (!plan && !latest) return null;
  return (
    <span style={{ flexShrink: 0, fontSize: 10, color: "var(--text-tertiary)", whiteSpace: "nowrap", display: "inline-flex", gap: 6, alignItems: "baseline" }}>
      {plan && (
        <span>
          {t("platform.mitmPlanTier", "套餐 {{tier}}", { tier: plan.tier })}
        </span>
      )}
      {latest && (
        <span>
          {t("platform.mitmPlanRemain", "5h 剩 {{five}} · 7d 剩 {{seven}}", {
            five: formatPercent(100 - latest.five_hour_pct, 0),
            seven: formatPercent(100 - latest.seven_day_pct, 0),
          })}
        </span>
      )}
    </span>
  );
}
