import React, { memo } from "react";
import { useTranslation } from "react-i18next";
import type { Platform, Protocol, PlatformUsageStats, LastTestResult } from "../../services/api";
import { getPlatformLogo, getFaviconUrl } from "../../assets/platforms";
import { CompactCard, StatChip, BalanceBar, successRateLevel, costLevel, usageLevelToColor } from "../shared";
import { formatNumber, formatCost, formatPercent } from "../../utils/formatters";
import { IconBolt, IconCost, IconCheck, IconCoin, IconClock } from "../icons";
import {
  PROTOCOL_LABELS, PROTOCOL_COLORS, HEALTH_COLORS,
  getDefaultModels, computeManualBudgetDisplay,
  type QuotaDisplay,
  allModelValues, tierLabel, formatResetCountdown, healthStatus,
} from "../../pages/Platforms";

// ── Props types ──

export interface PlatformCardActions {
  onPointerDown: (e: React.PointerEvent, index: number) => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onPointerUp: () => void;
  onToggleExpanded: (id: number, next: boolean) => void;
  onRefreshQuota: (p: Platform) => void;
  onToggleEnabled: (p: Platform) => void;
  onEdit: (p: Platform) => void;
  onDelete: (id: number) => void;
  onViewLogs: (p: Platform) => void;
  onQuickTest: (p: Platform) => void;
  onCustomTest: (p: Platform) => void;
  onFaviconFailed: (id: number) => void;
}

export interface PlatformCardProps {
  platform: Platform;
  index: number;
  isDragging: boolean;
  dragActive: boolean;
  quota: QuotaDisplay;
  refreshing: boolean;
  /** 延迟档 quota 外部 HTTP 待回：余额/配额区显骨架而非 est 旧值，避免闪烁回填（默认 false）。 */
  quotaPending?: boolean;
  /** 渐进档 usage 批量待回：用量区显骨架而非空白（默认 false）。 */
  usagePending?: boolean;
  usage: PlatformUsageStats | undefined;
  expanded: boolean;
  manualResult: "ok" | "fail" | undefined;
  testing: boolean;
  faviconFailed: boolean;
  actions: PlatformCardActions;
  platformMembership?: string[];
  /** 是否显示拖拽把手（默认 true；分组展开区等只读场景传 false） */
  draggable?: boolean;
  /** 最近一次测试结果（来自 proxy_log source_protocol='test' 最新一条）；undefined/无记录不渲染徽章 */
  lastTest?: LastTestResult;
}

// ── PlatformCard 组件 ──

export const PlatformCard = memo(function PlatformCard({
  platform: p,
  index: i,
  isDragging,
  dragActive,
  quota,
  refreshing,
  quotaPending = false,
  usagePending = false,
  usage: u,
  expanded,
  manualResult: manual,
  testing,
  faviconFailed: faviconHasFailed,
  actions,
  platformMembership,
  draggable = true,
  lastTest,
}: PlatformCardProps) {
  const { t } = useTranslation();
  const color = PROTOCOL_COLORS[p.platform_type] || "var(--accent)";
  const hasCodingEndpoint = (p.endpoints ?? []).some(ep => ep.coding_plan);
  const configuredModels = (() => {
    const explicit = allModelValues(p.models);
    if (explicit.length > 0) return explicit;
    if ((p.available_models?.length ?? 0) > 0) return explicit;
    return allModelValues(getDefaultModels(p.platform_type, hasCodingEndpoint));
  })();
  const quotaCapable = p.platform_type !== "mock" && p.platform_type !== "claude_code";
  const showQuota = quotaCapable && quota.hasData;
  // ④ 延迟档：可查 quota 的平台数据未回（quotaPending）→ 余额区显骨架而非空白/est 旧值闪烁
  const showQuotaSkeleton = quotaCapable && !quota.hasData && quotaPending;
  const mb = computeManualBudgetDisplay(p.manual_budgets);
  const total = u ? u.total_input_tokens + u.total_output_tokens : 0;
  const sr = u && u.total_requests > 0 ? (u.success_count / u.total_requests * 100) : 0;
  const hasDetail = !!u || usagePending || (p.endpoints && p.endpoints.length > 0) || configuredModels.length > 0 || quota.tiers.length > 0;
  const health = manual
    ? (manual === "ok" ? "healthy" : "error")
    : u ? healthStatus(u.recent_total, u.recent_failures) : "unknown";
  const logoSvg = getPlatformLogo(p.platform_type);
  const favicon = !logoSvg && !faviconHasFailed ? getFaviconUrl(p) : null;
  const getBaseUrl = (proto: Protocol, eps: Platform["endpoints"]): string => {
    const primary = eps?.find(ep => ep.protocol === proto);
    if (primary) return primary.base_url;
    return eps?.[0]?.base_url || "";
  };

  return (
    <div
      data-platform-id={p.id}
      style={{
        animationDelay: `${i * 50}ms`,
        opacity: dragActive ? (isDragging ? 0 : 0.4) : p.enabled ? 1 : 0.5,
        ...(isDragging ? { height: 0, overflow: "hidden", padding: 0, margin: 0, borderWidth: 0, minHeight: 0 } : {}),
        transition: "opacity 150ms ease",
      }}
    >
      <CompactCard
        expanded={hasDetail ? expanded : undefined}
        onToggle={hasDetail ? (next) => actions.onToggleExpanded(p.id, next) : undefined}
        toggleLabel={t("platform.toggleDetail", "展开/收起明细")}
        header={(
          <div style={{ display: "flex", flexDirection: "column", gap: 10, minWidth: 0 }}>
            {/* ── 行 1：身份 + 快操作 ── */}
            <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0 }}>
              {/* 拖拽把手（分组展开区等只读场景不渲染） */}
              {draggable && (
              <div
                className={`drag-handle-inline${isDragging ? " is-active" : ""}`}
                style={{ cursor: "grab", color: "var(--text-tertiary)", flexShrink: 0, display: "flex", touchAction: "none" }}
                onPointerDown={e => actions.onPointerDown(e, i)}
                onPointerMove={actions.onPointerMove}
                onPointerUp={actions.onPointerUp}
                title={t("platform.dragReorder", "拖拽排序")}
              >
                <svg width="12" height="18" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
              </div>
              )}
              {/* Logo + 健康点 */}
              <div style={{ position: "relative", flexShrink: 0 }}>
                <div style={{
                  width: 36, height: 36, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: (logoSvg || favicon) ? "transparent" : `${color}15`,
                  border: `1px solid ${color}30`,
                  color: color, fontSize: 12, fontWeight: 700, overflow: "hidden",
                }}>
                  {logoSvg
                    ? <img src={logoSvg} alt={p.platform_type} style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
                    : favicon
                      ? <img src={favicon} alt={p.platform_type}
                          style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }}
                          onError={() => actions.onFaviconFailed(p.id)}
                        />
                      : p.platform_type.slice(0, 2).toUpperCase()
                  }
                </div>
                {health !== "unknown" && (
                  <div style={{
                    position: "absolute", top: -3, right: -3,
                    width: 10, height: 10, borderRadius: "50%",
                    background: HEALTH_COLORS[health],
                    border: "2px solid var(--bg-primary)",
                    boxShadow: `0 0 4px ${HEALTH_COLORS[health]}60`,
                  }} />
                )}
              </div>
              {/* 名称 + 协议·base_url */}
              <div style={{ minWidth: 0, flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: 14, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.name}</div>
                <div className="text-secondary" style={{ fontSize: 11, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {p.platform_type.toUpperCase()} · {getBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url}
                </div>
                {p.status === "auto_disabled" && (
                  <div
                    style={{
                      marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
                      fontSize: 10, fontWeight: 600, color: "var(--color-warning)",
                      background: "color-mix(in srgb, var(--color-warning) 14%, transparent)",
                      border: "1px solid color-mix(in srgb, var(--color-warning) 35%, transparent)",
                      borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
                    }}
                    title={t("platform.autoDisabledHint", "401/403 自动禁用，下次试探时间 {{time}}")
                      .replace("{{time}}", p.auto_disabled_until > 0 ? new Date(p.auto_disabled_until).toLocaleString() : "-")}
                  >
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M12 9v4" /><path d="M12 17h.01" />
                      <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                    </svg>
                    {t("platform.autoDisabled", "自动禁用")}
                  </div>
                )}
                {/* 所属分组 badge */}
                {platformMembership && platformMembership.length > 0 && (
                  <div style={{ marginTop: 3, display: "flex", gap: 4, flexWrap: "wrap" }}>
                    {platformMembership.map(gName => (
                      <span key={gName} className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px" }}>
                        {gName}
                      </span>
                    ))}
                  </div>
                )}
                {/* 最近一次测试结果徽章（常驻；无记录不渲染） */}
                {lastTest && <LastTestBadge result={lastTest} />}
              </div>
              {/* 快操作 */}
              <div style={{ display: "flex", gap: 4, flexShrink: 0, alignItems: "center" }}>
                {showQuota && (
                  <button
                    className="btn btn-ghost btn-icon"
                    style={{ padding: 4, lineHeight: 0, minWidth: "auto" }}
                    disabled={refreshing}
                    title={t("platform.quotaRefresh", "刷新额度")}
                    onClick={(e) => { e.stopPropagation(); actions.onRefreshQuota(p); }}
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                      strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"
                      style={refreshing ? { animation: "spin 0.9s linear infinite" } : undefined}>
                      <path d="M21 12a9 9 0 1 1-2.64-6.36" />
                      <polyline points="21 3 21 9 15 9" />
                    </svg>
                  </button>
                )}
                <div
                  className={`toggle ${p.status === "enabled" ? "active" : ""}`}
                  style={{ cursor: "pointer" }}
                  onClick={(e) => { e.stopPropagation(); actions.onToggleEnabled(p); }}
                  title={p.status === "enabled"
                    ? t("platform.disable", "禁用")
                    : p.status === "auto_disabled"
                      ? t("platform.reenable", "重新启用")
                      : t("platform.enable", "启用")}
                />
                <div style={{ display: "inline-flex", fontSize: 11 }}>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 11, gap: 4, padding: "3px 8px", borderRadius: "6px 0 0 6px", borderRight: "1px solid var(--border)" }}
                    disabled={testing}
                    onClick={(e) => { e.stopPropagation(); actions.onQuickTest(p); }}
                    title={t("platform.quickTest", "快速测试默认模型")}
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" stroke="none">
                      <path d="M13 2L4 14h7l-2 8 9-12h-7l2-8z"/>
                    </svg>
                  </button>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 11, padding: "3px 6px", borderRadius: "0 6px 6px 0" }}
                    onClick={(e) => { e.stopPropagation(); actions.onCustomTest(p); }}
                    title={t("platform.customTest", "自定义测试")}
                  >
                    <svg width="10" height="10" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M3 5l4 4 4-4" />
                    </svg>
                  </button>
                </div>
                <button className="btn btn-ghost btn-icon" title={t("platform.viewLogs", "查看日志")} onClick={(e) => { e.stopPropagation(); actions.onViewLogs(p); }}>
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 2h10v10H2z" />
                    <path d="M4 5h6M4 7h4M4 9h5" />
                  </svg>
                </button>
                <button className="btn btn-ghost btn-icon" onClick={(e) => { e.stopPropagation(); actions.onEdit(p); }}>
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M10 2l2 2-7 7H3v-2l7-7z" />
                  </svg>
                </button>
                <button className="btn btn-ghost btn-icon btn-danger" onClick={(e) => { e.stopPropagation(); actions.onDelete(p.id); }}>
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                  </svg>
                </button>
              </div>
            </div>
            {/* ── 行 2：余额 / 预算 / coding tiers ── */}
            {showQuota && (quota.balanceRemaining != null || (mb && mb.hasData) || (quota.balanceRemaining == null && quota.tiers.length > 0)) && (
              <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", paddingLeft: 24 }}>
                {/* 余额 */}
                {quota.balanceRemaining != null && (() => {
                  const balColor = usageLevelToColor(p.balance_level);
                  return (
                    <div style={{ flexShrink: 0, width: 120, display: "flex", flexDirection: "column", gap: 2 }}>
                      <BalanceBar remaining={quota.balanceRemaining} total={quota.balanceTotal} currency={quota.currency === "USD" ? "$" : quota.currency} level={balColor === "neutral" ? undefined : balColor} />
                    </div>
                  );
                })()}
                {/* 手动预算 */}
                {mb && mb.hasData && (
                  <div style={{ flexShrink: 0, width: 120, display: "flex", flexDirection: "column", gap: 2 }}>
                    {mb.unit === "usd" ? (
                      <BalanceBar remaining={mb.remaining} total={mb.amount} currency="$" />
                    ) : (
                      <div style={{ display: "flex", flexDirection: "column", gap: 3, minWidth: 0 }}>
                        <span style={{ fontWeight: 700, fontSize: 12, color: mb.depleted ? "var(--color-danger)" : mb.ratio < 0.2 ? "var(--color-warning)" : "var(--text-primary)" }}>
                          {formatNumber(Math.max(0, mb.remaining))}
                          <span style={{ fontSize: 9, color: "var(--text-tertiary)", marginLeft: 3 }}>/ {formatNumber(mb.amount)} tok</span>
                        </span>
                        <div style={{ height: 4, borderRadius: "var(--radius-sm)", background: "var(--bg-glass)", overflow: "hidden" }}>
                          <div style={{ width: `${mb.ratio * 100}%`, height: "100%", background: mb.depleted ? "var(--color-danger)" : mb.ratio < 0.2 ? "var(--color-warning)" : "var(--color-success)", borderRadius: "var(--radius-sm)", transition: "width 0.3s ease" }} />
                        </div>
                      </div>
                    )}
                    <span style={{ fontSize: 9, fontWeight: 700, color: mb.depleted ? "var(--color-danger)" : "var(--text-tertiary)" }}>
                      {mb.depleted
                        ? t("platform.manualBudgetDepleted", "额度耗尽")
                        : t("platform.manualBudgetLabel", "手动预算")}
                      {mb.unit === "token" && ` · ${t("platform.manualBudgetTokenApprox", "≈未知$")}`}
                    </span>
                  </div>
                )}
                {/* Coding plan tiers */}
                {quota.balanceRemaining == null && quota.tiers.length > 0 && (
                  <div style={{ flexShrink: 0, display: "flex", gap: 4, flexWrap: "wrap", maxWidth: 260 }}>
                    {quota.tiers.map(tier => {
                      const isMcp = tier.name === "mcp_monthly";
                      const value = isMcp && tier.limit != null
                        ? `${tier.remaining ?? 0}/${tier.limit}`
                        : `${tier.remainPct.toFixed(0)}%`;
                      const remainSuffix = t("platform.quotaRemainSuffix", "剩");
                      const tierColor = tier.level === "danger" ? "var(--color-danger)" : tier.level === "warning" ? "var(--color-warning)" : tier.level === "success" ? "var(--color-success)" : "var(--text-secondary)";
                      const countdown = formatResetCountdown(tier.resetsAt);
                      return (
                        <span key={tier.name} style={{
                          display: "inline-flex", alignItems: "center", gap: 3,
                          padding: "2px 6px", borderRadius: "var(--radius-sm)",
                          fontSize: 10, fontWeight: 600,
                          background: tier.level === "neutral" ? "var(--bg-glass)" : tier.level === "danger" ? "var(--color-danger)15" : tier.level === "warning" ? "var(--color-warning)15" : "var(--color-success)15",
                          color: tierColor,
                        }}>
                          <span style={{ fontSize: 11, fontWeight: 700 }}>{value}<span style={{ fontSize: 8, fontWeight: 600, opacity: 0.65, marginLeft: 1 }}>{remainSuffix}</span></span>
                          <span style={{ fontSize: 9, opacity: 0.7 }}>{tierLabel(tier.name)}</span>
                          {countdown && <span style={{ fontSize: 8, opacity: 0.6 }}>·{countdown}</span>}
                        </span>
                      );
                    })}
                  </div>
                )}
              </div>
            )}
            {/* ④ 余额区骨架：quota 外部 HTTP 待回时占位，禁空白/est 旧值闪烁 */}
            {showQuotaSkeleton && (
              <div style={{ display: "flex", alignItems: "center", gap: 10, paddingLeft: 24 }}>
                <span className="skeleton" style={{ width: 120, height: 22 }} aria-label={t("platform.quotaLoading", "额度加载中")} />
              </div>
            )}
          </div>
        )}
      >
        {hasDetail && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {/* 已使用统计 */}
            {u && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.usageLabel", "已使用")}</span>
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  <StatChip icon={<IconBolt size={13} />} value={formatNumber(total)} label="tokens" />
                  <StatChip icon={<IconCost size={13} />} value={`$${formatCost(u.total_cost)}`} label="cost" level={costLevel(u.total_cost)} />
                  <StatChip icon={<IconCheck size={13} />} value={formatPercent(sr)} label="ok" level={successRateLevel(sr, u.total_requests)} />
                </div>
              </div>
            )}
            {/* ④ 用量区骨架：渐进档批量 usage 待回时占位（无既有 usage），禁空白 */}
            {!u && usagePending && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.usageLabel", "已使用")}</span>
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  <span className="skeleton" style={{ width: 80, height: 24 }} aria-label={t("platform.usageLoading", "用量加载中")} />
                  <span className="skeleton" style={{ width: 70, height: 24 }} />
                  <span className="skeleton" style={{ width: 60, height: 24 }} />
                </div>
              </div>
            )}
            {/* 配额各档明细 */}
            {showQuota && quota.tiers.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.quotaLabel", "额度")}</span>
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  {quota.tiers.map(tier => {
                    const countdown = formatResetCountdown(tier.resetsAt);
                    const value = (tier.name === "mcp_monthly" && tier.limit != null
                      ? `${tier.remaining ?? 0}/${tier.limit}`
                      : `${tier.remainPct.toFixed(0)}%`) + t("platform.quotaRemainSuffix", "剩");
                    return (
                      <div key={tier.name} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                        <StatChip icon={<IconCoin size={13} />}
                          value={value}
                          label={tierLabel(tier.name)}
                          level={tier.level === "danger" ? "danger" : tier.level === "warning" ? "warning" : tier.level === "success" ? "success" : "neutral"} />
                        {countdown && (
                          <span className="text-tertiary" style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 10, fontWeight: 600, paddingLeft: 2 }}>
                            <IconClock size={11} />
                            {t("platform.resetIn", "重置 {{countdown}}", { countdown })}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
            {/* Endpoints badges */}
            {p.endpoints && p.endpoints.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.endpoints", "Protocol Endpoints")}</span>
                <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                  {p.endpoints.map((ep, ei) => (
                    <span key={ei} className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px", opacity: 0.85 }}>
                      {PROTOCOL_LABELS[ep.protocol] || ep.protocol}
                      {ep.coding_plan && <span style={{ color: "var(--color-success)", marginLeft: 2, fontWeight: 700 }}>Code</span>}
                    </span>
                  ))}
                </div>
              </div>
            )}
            {/* 模型 badges */}
            {configuredModels.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.models")}</span>
                <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                  {configuredModels.map((m, mi) => (
                    <span key={mi} className="badge badge-muted" style={{ fontSize: 11, padding: "2px 6px" }}>{m}</span>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </CompactCard>
    </div>
  );
});

// ── 最近一次测试徽章 ──

/** 毫秒 epoch → 相对时间文案（刚刚 / N 分钟前 / N 小时前 / N 天前）。 */
function relativeTime(createdMs: number, now: number = Date.now()): string {
  const diff = Math.max(0, now - createdMs);
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h`;
  const day = Math.floor(hr / 24);
  return `${day}d`;
}

function LastTestBadge({ result }: { result: LastTestResult }) {
  const { t } = useTranslation();
  const ok = result.success;
  const color = ok ? "var(--color-success)" : "var(--color-danger)";
  const rel = relativeTime(result.created_at);
  const errorText = !ok && result.error ? result.error.slice(0, 30) : "";
  return (
    <div style={{
      marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
      fontSize: 10, fontWeight: 600, color,
      background: `color-mix(in srgb, ${color} 12%, transparent)`,
      border: `1px solid color-mix(in srgb, ${color} 30%, transparent)`,
      borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap", maxWidth: "100%",
    }}
      title={ok
        ? t("platform.lastTestOkHint", "最近测试通过 · {{time}}", { time: new Date(result.created_at).toLocaleString() })
        : t("platform.lastTestFailHint", "最近测试失败 · {{time}}{{error}}", {
            time: new Date(result.created_at).toLocaleString(),
            error: result.error ? `\n${result.error}` : "",
          })}
    >
      <span style={{ fontWeight: 700 }}>{ok ? "✓" : "✗"}</span>
      {result.duration_ms > 0 && <span>{result.duration_ms}ms</span>}
      {rel && <span style={{ opacity: 0.85 }}>· {rel}</span>}
      {!ok && errorText && (
        <span style={{ opacity: 0.85, overflow: "hidden", textOverflow: "ellipsis", maxWidth: 120 }}>{errorText}</span>
      )}
    </div>
  );
}
