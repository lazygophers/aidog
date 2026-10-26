// PlatformRow — G3 设计稿「平台管理页」行式布局（.scratch/重设计-终稿/5-平台管理页.html）。
// 10 列 grid（稿 .row）：handle 26 / logo 34 / ident 1.35fr / 配额 1fr / 今日 74 / 成功率 62 /
// 延迟 58 / 模型 58 / 最近测速 150 / 操作 108。色一律走主题 token（稿的 #08090A 画布映射
// var(--background) 系），几何（列宽/间距/字号）1:1 按稿。
import { useMemo } from "react";
import type { TFunction } from "i18next";
import type { Platform, PlatformUsageStats, PlatformQuota, LastTestResult } from "../../services/api";
import { parseRateLimitsBundle } from "../../services/api";
import { formatNumber, formatCost, formatPercent, formatDurationMs, formatRelativeTime, successRate as calcSuccessRate } from "../../utils/formatters";
import { isCurrentlyPeak } from "../../utils/timeWindow";
import { ProtocolLogo } from "../../domains/platforms/ProtocolLogo";
import { useProtocolMeta } from "../../domains/platforms/useProtocolMeta";
import { allModelValues } from "../../domains/platforms/health";
import {
  DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem,
} from "@/components/ui/dropdown-menu";

/** 行 grid 模板（稿真值，含未分组列头共用）。 */
export const PLATFORM_ROW_GRID = "26px 34px minmax(200px,1.35fr) minmax(150px,1fr) 74px 62px 58px 58px 150px 108px";

/** 未分组段列头（稿 .cols），与行 grid 同列宽。 */
export function PlatformRowCols({ t }: { t: TFunction }) {
  return (
    <div data-testid="platform-row-cols" style={{
      display: "grid", gridTemplateColumns: PLATFORM_ROW_GRID, gap: 10,
      padding: "0 14px 6px 8px", fontSize: 12, color: "var(--text-tertiary)", letterSpacing: "0.02em",
    }}>
      <span /><span />
      <span>{t("platform.col.name", "平台")}</span>
      <span>{t("platform.col.quota", "余额 / 配额")}</span>
      <span style={{ textAlign: "right" }}>{t("platform.col.today", "今日")}</span>
      <span style={{ textAlign: "right" }}>{t("platform.col.successRate", "成功率")}</span>
      <span style={{ textAlign: "right" }}>{t("platform.col.latency", "延迟")}</span>
      <span style={{ textAlign: "right" }}>{t("platform.col.models", "模型")}</span>
      <span>{t("platform.col.lastTest", "最近测速")}</span>
      <span style={{ textAlign: "right" }}>{t("platform.col.ops", "操作")}</span>
    </div>
  );
}

function hostOf(base_url: string): string {
  try { return new URL(base_url).host; } catch { return base_url; }
}

export interface PlatformRowProps {
  platform: Platform;
  usage?: PlatformUsageStats;
  quotaRaw?: PlatformQuota;
  quotaPreferReal?: boolean; // 保留接口（父级管线传入）；行式余额列只用 balance/est 回落，暂不消费校准标记
  lastTest?: LastTestResult;
  manualResult?: "ok" | "fail";
  testing?: boolean;
  expanded?: boolean;
  selecting?: boolean;
  selected?: boolean;
  levelPriority?: number;
  t: TFunction;
  lang: string;
  /** 列 1 插槽：多选 checkbox 或拖拽把手（组内排序把手 / 未分组 pointer 拖拽把手由父级渲染）。 */
  lead?: React.ReactNode;
  onSelect?: () => void;
  onToggleExpanded?: (next: boolean) => void;
  onToggle?: (p: Platform) => void;
  onEdit?: (p: Platform) => void;
  onShare?: (p: Platform) => void;
  onDuplicate?: (p: Platform) => void;
  onViewLogs?: (p: Platform) => void;
  onQuickTest?: (p: Platform) => void;
  onDelete?: (id: number) => void;
  onLevelPriorityChange?: (v: number) => void;
}

const STAT_GAP = 10;

function StatCell({ v, l, tone }: { v: string; l: string; tone?: "ok" | "warn" | "dim" }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 1, minWidth: 0 }}>
      <span className="num" style={{
        fontSize: 13, fontWeight: 580, textAlign: "right", fontVariantNumeric: "tabular-nums",
        color: tone === "ok" ? "var(--color-success)" : tone === "warn" ? "var(--color-warning)" : tone === "dim" ? "var(--text-tertiary)" : "var(--foreground)",
      }}>{v}</span>
      <span style={{ fontSize: 12, color: "var(--text-tertiary)", textAlign: "right" }}>{l}</span>
    </div>
  );
}

export function PlatformRow({
  platform: p, usage: u, quotaRaw, quotaPreferReal: _quotaPreferReal, lastTest, manualResult, testing,
  expanded, selecting, selected, levelPriority,
  t, lang, lead, onSelect, onToggleExpanded, onToggle,
  onEdit, onShare, onDuplicate, onViewLogs, onQuickTest, onDelete, onLevelPriorityChange,
}: PlatformRowProps) {
  const { isCpProtocol, defaultModels, peakWindows } = useProtocolMeta(p.platform_type, p.extra ?? "", lang);
  // 转发生效模型（PlatformCard 同口径）：显式槽位优先，没配回落 preset 默认。
  const configuredModels = useMemo(() => {
    const explicit = allModelValues(p.models);
    return explicit.length > 0 ? explicit : defaultModels;
  }, [p.models, defaultModels]);
  const rateLimitsBundle = useMemo(() => parseRateLimitsBundle(p.extra ?? ""), [p.extra]);
  // 限频徽标计数：平台级 RPM/TPM 配置记 1 窗 + 额度窗口数（稿 5「限频 N 窗口」）。
  const rlBase = rateLimitsBundle.rate_limits;
  const rlConfigured = !!(rlBase && ((rlBase.rpm ?? 0) > 0 || (rlBase.tpm ?? 0) > 0 || (rlBase.models && Object.keys(rlBase.models).length > 0)));
  const qwCount = rateLimitsBundle.quota_windows?.length ?? 0;
  const rlCount = (rlConfigured ? 1 : 0) + qwCount;
  const disableDuringPeak = useMemo(() => {
    try { return !!JSON.parse(p.extra || "{}").disable_during_peak; } catch { return false; }
  }, [p.extra]);
  const nowMs = Date.now();
  const inPeak = isCurrentlyPeak(peakWindows, nowMs);
  const hitWin = inPeak ? peakWindows.find(w => isCurrentlyPeak([w], nowMs)) : undefined;
  // 配额列：真实 quota balance 优先，缺省回落系统维护的 est_balance_remaining。
  const bal = quotaRaw?.balance ?? null;
  const remaining = bal ? bal.remaining : p.est_balance_remaining;
  const total = bal?.total ?? null;
  const pct = total != null && total > 0 ? Math.max(0, Math.min(100, (remaining / total) * 100)) : null;
  const level = p.balance_level;
  const barColor = level === "red" || (pct != null && pct < 15) ? "var(--color-danger)"
    : level === "yellow" || (pct != null && pct < 30) ? "var(--color-warning)" : "var(--color-success)";
  const sRate = u && u.total_requests > 0 ? calcSuccessRate(u.success_count, u.total_requests) : null;
  const host = hostOf(p.base_url);
  const hasCodingEndpoint = (p.endpoints ?? []).some(ep => ep.coding_plan);
  const status = p.status;
  const enabled = status === "enabled";
  const testOk = manualResult ? manualResult === "ok" : lastTest ? lastTest.success : null;
  const ago = lastTest ? formatRelativeTime(lastTest.created_at) : null;
  const badgeStyle = (fg: string, bg: string, bd: string): React.CSSProperties => ({
    fontSize: 12, fontWeight: 600, borderRadius: 5, padding: "1.5px 6px", whiteSpace: "nowrap", flexShrink: 0,
    color: fg, background: bg, border: `1px solid ${bd}`,
  });

  const iconBtn = (extra?: React.CSSProperties): React.CSSProperties => ({
    width: 27, height: 27, borderRadius: 7, display: "inline-flex", alignItems: "center", justifyContent: "center",
    color: "var(--text-tertiary)", cursor: "pointer", border: "1px solid transparent", background: "transparent",
    transition: "background .15s, color .15s, border-color .15s", ...extra,
  });

  return (
    <div data-testid="platform-row" data-platform-id={p.id} data-status={status} data-selected={selected ? "true" : undefined}
      style={{
        position: "relative",
        opacity: enabled ? undefined : 0.62,
        ...(selected ? { background: "var(--accent-subtle)", boxShadow: "inset 2px 0 0 var(--primary)" } : {}),
      }}>
      <div
        onClick={() => { if (!selecting) onToggleExpanded?.(!expanded); }}
        style={{
          display: "grid", gridTemplateColumns: PLATFORM_ROW_GRID, alignItems: "center", gap: STAT_GAP,
          padding: "11px 14px 11px 8px", borderBottom: "1px solid var(--border)",
          transition: "background .15s", cursor: "default",
        }}
        onMouseEnter={e => { if (!selected) (e.currentTarget as HTMLElement).style.background = "color-mix(in srgb, var(--foreground) 3%, transparent)"; }}
        onMouseLeave={e => { (e.currentTarget as HTMLElement).style.background = "transparent"; }}
      >
        {/* 列 1：把手 / 多选 checkbox */}
        <span style={{ justifySelf: "center", display: "inline-flex", alignItems: "center" }} onClick={e => e.stopPropagation()}>
          {selecting
            ? (
              <input type="checkbox" data-testid="row-select" checked={!!selected} onChange={() => onSelect?.()}
                style={{ width: 15, height: 15, accentColor: "var(--primary)", cursor: "pointer" }} />
            )
            : lead}
        </span>
        {/* 列 2：logo + 状态点 */}
        <span style={{ position: "relative", width: 30, height: 30, flexShrink: 0 }}>
          <ProtocolLogo protocol={p.platform_type} size={30} />
          <span data-testid="row-status-dot" title={status}
            style={{
              position: "absolute", top: -3, right: -3, width: 9, height: 9, borderRadius: "50%",
              border: "2px solid var(--background)",
              background: enabled ? "var(--color-success)" : status === "auto_disabled" ? "var(--color-danger)" : "var(--color-warning)",
            }} />
        </span>
        {/* 列 3：身份（名 + 徽标 / 协议 · host） */}
        <span style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: 3 }}>
          <span style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
            <span style={{ fontSize: 13.5, fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{p.name}</span>
            {(isCpProtocol || hasCodingEndpoint) && (
              <span style={badgeStyle("var(--color-success)", "color-mix(in srgb, var(--color-success) 12%, transparent)", "color-mix(in srgb, var(--color-success) 32%, transparent)")}>
                {t("platform.codingPlanBadge", "Coding Plan")}
              </span>
            )}
            {p.platform_type === "claude_code" && (
              <span style={badgeStyle("var(--primary)", "var(--accent-subtle)", "color-mix(in srgb, var(--primary) 40%, transparent)")}>
                {t("platform.subBadge", "订阅")}
              </span>
            )}
            {rlCount > 0 && (
              <span style={badgeStyle("var(--text-secondary)", "color-mix(in srgb, var(--foreground) 5%, transparent)", "var(--border)")}>
                {t("platform.rateLimitBadge", "限频 {{n}} 窗口", { n: rlCount })}
              </span>
            )}
            {disableDuringPeak && inPeak && (
              <span style={badgeStyle("var(--color-warning)", "color-mix(in srgb, var(--color-warning) 12%, transparent)", "color-mix(in srgb, var(--color-warning) 35%, transparent)")}>
                {t("platform.peak_disabled_badge", "高峰禁用中")}
              </span>
            )}
            {!disableDuringPeak && hitWin && (
              <span style={badgeStyle("var(--primary)", "color-mix(in srgb, var(--primary) 10%, transparent)", "color-mix(in srgb, var(--primary) 30%, transparent)")}>
                {t("platform.peakX", "Peak ×{{x}}", { x: hitWin.multiplier ?? 1 })}
              </span>
            )}
            {status === "auto_disabled" && (
              <span style={badgeStyle("var(--color-warning)", "color-mix(in srgb, var(--color-warning) 12%, transparent)", "color-mix(in srgb, var(--color-warning) 35%, transparent)")}>
                {t("platform.autoDisabledBadge", "自动禁用")}
              </span>
            )}
          </span>
          <span style={{ fontSize: 12, color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {p.platform_type} · {host}
          </span>
        </span>
        {/* 列 4：余额 / 配额 */}
        <span style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: 5 }}>
          <span style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--text-secondary)" }}>
            <span className="num" style={{ color: "var(--foreground)", fontWeight: 560, fontVariantNumeric: "tabular-nums" }}>
              ${remaining > 0 ? formatCost(remaining) : "0"}
            </span>
            <span className="num" style={{ fontVariantNumeric: "tabular-nums" }}>
              {total != null ? `/ $${formatCost(total)}` : t("platform.col.balance", "余额")}
            </span>
          </span>
          <span style={{ height: 4, borderRadius: 99, background: "color-mix(in srgb, var(--foreground) 7%, transparent)", overflow: "hidden", display: "block" }}>
            <span data-testid="row-quota-bar" style={{ display: "block", height: "100%", borderRadius: 99, width: `${pct ?? 0}%`, background: barColor }} />
          </span>
        </span>
        {/* 列 5-8：今日 / 成功率 / 延迟 / 模型 */}
        <StatCell v={u ? `$${formatCost(u.today_cost)}` : "—"} l={t("platform.col.today", "今日")} tone={u ? undefined : "dim"} />
        <StatCell v={sRate != null ? formatPercent(sRate, 1) : "—"} l={t("platform.col.successRate", "成功率")}
          tone={sRate == null ? "dim" : sRate >= 98 ? "ok" : sRate >= 90 ? "warn" : undefined} />
        <StatCell v={lastTest ? formatDurationMs(lastTest.duration_ms) : "—"} l={t("platform.col.latency", "延迟")} tone={lastTest ? undefined : "dim"} />
        <StatCell v={String(configuredModels.length)} l={t("platform.col.models", "模型")} />

        {/* 列 9：最近测速 */}
        <span data-testid="row-last-test" data-result={testOk == null ? "none" : testOk ? "ok" : "fail"}
          style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--text-secondary)", minWidth: 0 }}>
          {testing ? (
            <span style={{ color: "var(--text-tertiary)" }}>{t("status.testing", "测试中…")}</span>
          ) : testOk == null ? (
            <span style={{ color: "var(--text-tertiary)" }}>—</span>
          ) : (
            <>
              <span style={{
                width: 15, height: 15, borderRadius: 5, display: "inline-flex", alignItems: "center", justifyContent: "center",
                flexShrink: 0, fontSize: 12, fontWeight: 700,
                background: testOk ? "color-mix(in srgb, var(--color-success) 15%, transparent)" : "color-mix(in srgb, var(--color-danger) 15%, transparent)",
                color: testOk ? "var(--color-success)" : "var(--color-danger)",
              }}>{testOk ? "✓" : "✕"}</span>
              <span className="num" style={{ fontVariantNumeric: "tabular-nums" }}>
                {testOk ? formatDurationMs(lastTest!.duration_ms) : (lastTest?.status_code ?? t("platform.testFail", "失败"))}
              </span>
              {ago && <span style={{ color: "var(--text-tertiary)" }}>{ago}</span>}
            </>
          )}
        </span>
        {/* 列 10：操作（稿 .ops：⚡ 测速 · ✎ 编辑 · ⋯ 菜单 · 启用开关） */}
        <span style={{ display: "flex", alignItems: "center", gap: 7, justifyContent: "flex-end" }} onClick={e => e.stopPropagation()}>
          <button type="button" className="ripple" title={t("platform.quickTest", "测速")} style={iconBtn(inPeak ? { color: "var(--color-warning)" } : undefined)}
            onClick={() => onQuickTest?.(p)}>
            <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><path d="M8.2 1.2L3 8h3.4l-.8 4.8L11 6H7.4z" /></svg>
          </button>
          <button type="button" className="ripple" title={t("action.edit", "编辑")} style={iconBtn()} onClick={() => onEdit?.(p)}>
            <svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M9.8 1.8l2.4 2.4L5 11.4l-3 .6.6-3z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" /></svg>
          </button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button type="button" data-testid="row-menu" title={t("platform.more", "更多")} style={iconBtn()}>
                <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor"><circle cx="3" cy="7" r="1.3" /><circle cx="7" cy="7" r="1.3" /><circle cx="11" cy="7" r="1.3" /></svg>
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" style={{ minWidth: 150 }}>
              {onViewLogs && <DropdownMenuItem onSelect={() => onViewLogs(p)}>{t("platform.viewLogs", "查看日志")}</DropdownMenuItem>}
              {onShare && <DropdownMenuItem onSelect={() => onShare(p)}>{t("platform.share.button", "分享配置")}</DropdownMenuItem>}
              {onDuplicate && <DropdownMenuItem onSelect={() => onDuplicate(p)}>{t("platform.duplicate", "复制平台")}</DropdownMenuItem>}
              {onLevelPriorityChange && (
                <>
                  <div style={{ padding: "4px 9px 2px", fontSize: 11, color: "var(--text-tertiary)" }}>{t("platform.levelPriority", "候选优先级")}</div>
                  {[1, 2, 3, 4, 5].map(lv => (
                    <DropdownMenuItem key={lv} data-state={levelPriority === lv ? "active" : undefined} onSelect={() => onLevelPriorityChange(lv)}>
                      {t("platform.levelN", "L{{n}}", { n: lv })}{levelPriority === lv ? " ✓" : ""}
                    </DropdownMenuItem>
                  ))}
                </>
              )}
              {onDelete && <DropdownMenuItem style={{ color: "var(--color-danger)" }} onSelect={() => onDelete(p.id)}>{t("platform.deletePlatformMenu", "删除平台…")}</DropdownMenuItem>}
            </DropdownMenuContent>
          </DropdownMenu>
          <button type="button" role="switch" aria-checked={enabled} data-testid="row-toggle" data-state={enabled ? "on" : "off"}
            title={enabled ? t("platform.disable", "禁用") : t("platform.enable", "启用")}
            onClick={() => onToggle?.(p)}
            style={{
              width: 32, height: 19, borderRadius: 99, position: "relative", cursor: "pointer", flexShrink: 0,
              border: "none", padding: 0,
              background: enabled ? "var(--primary)" : "color-mix(in srgb, var(--foreground) 14%, transparent)",
              transition: "background .18s",
            }}>
            <span style={{
              position: "absolute", top: 2.5, left: 2.5, width: 14, height: 14, borderRadius: "50%",
              background: enabled ? "var(--primary-foreground)" : "var(--text-secondary)",
              transform: enabled ? "translateX(13px)" : undefined, transition: "transform .18s, background .18s", display: "block",
            }} />
          </button>
        </span>
      </div>
      {/* 展开明细（稿 .detail：模型 pills + 限频 / 预算 / 高峰 两栏） */}
      {expanded && (
        <div data-testid="row-detail" style={{
          padding: "14px 18px 16px 66px", borderBottom: "1px solid var(--border)",
          background: "color-mix(in srgb, var(--foreground) 1.5%, transparent)",
          display: "grid", gridTemplateColumns: "1.5fr 1fr", gap: 26,
        }}>
          <div>
            <h4 style={{ fontSize: 12, fontWeight: 600, color: "var(--text-tertiary)", letterSpacing: "0.02em", margin: "0 0 9px" }}>
              {t("platform.detail.models", "模型（{{n}}）", { n: configuredModels.length })}
            </h4>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
              {configuredModels.slice(0, 12).map((m, i) => (
                <span key={m} style={{
                  fontSize: 12, padding: "3px 9px", borderRadius: 6, color: "var(--text-secondary)",
                  background: "var(--secondary)", border: "1px solid var(--border)",
                  ...(i === 0 ? { color: "var(--primary)", borderColor: "color-mix(in srgb, var(--primary) 40%, transparent)", background: "var(--accent-subtle)" } : {}),
                }}>{m}</span>
              ))}
              {configuredModels.length > 12 && (
                <span style={{ fontSize: 12, color: "var(--text-tertiary)", alignSelf: "center" }}>+{configuredModels.length - 12}</span>
              )}
              {configuredModels.length === 0 && <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>—</span>}
            </div>
          </div>
          <div>
            <h4 style={{ fontSize: 12, fontWeight: 600, color: "var(--text-tertiary)", letterSpacing: "0.02em", margin: "0 0 9px" }}>
              {t("platform.detail.limits", "限频与预算")}
            </h4>
            {rlConfigured && (
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
                <thead><tr>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>{t("platform.detail.window", "窗口")}</th>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>RPM</th>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>TPM</th>
                </tr></thead>
                <tbody>
                  <tr>
                    <td style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--foreground)" }}>{t("platform.detail.allDay", "全天")}</td>
                    <td className="num" style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--text-secondary)" }}>{rlBase?.rpm ?? "—"}</td>
                    <td className="num" style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--text-secondary)" }}>{rlBase?.tpm != null ? formatNumber(rlBase.tpm) : "—"}</td>
                  </tr>
                </tbody>
              </table>
            )}
            {(rateLimitsBundle.quota_windows?.length ?? 0) > 0 && (
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12, marginTop: 10 }}>
                <thead><tr>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>{t("platform.detail.quotaWindow", "额度窗口")}</th>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>{t("platform.detail.budget", "预算")}</th>
                </tr></thead>
                <tbody>
                  {rateLimitsBundle.quota_windows!.map((w, i) => (
                    <tr key={i}>
                      <td style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--foreground)" }}>{formatWindowSecs(w.window)}</td>
                      <td className="num" style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--text-secondary)" }}>
                        {w.budget_tokens != null ? formatNumber(w.budget_tokens) + " tokens"
                          : w.budget_usd != null ? "$" + formatCost(w.budget_usd)
                          : w.budget_count != null ? formatNumber(w.budget_count) + " " + t("platform.detail.countUnit", "次")
                          : w.budget_points != null ? formatNumber(w.budget_points) + " pts" : "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
            {(p.manual_budgets ?? []).length > 0 && (
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12, marginTop: 10 }}>
                <thead><tr>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>{t("platform.detail.budget", "预算")}</th>
                  <th style={{ textAlign: "left", color: "var(--text-tertiary)", fontWeight: 500, padding: "0 8px 6px 0" }}>{t("platform.detail.used", "已用")}</th>
                </tr></thead>
                <tbody>
                  {p.manual_budgets.filter(b => b.enabled).map(b => (
                    <tr key={b.id}>
                      <td className="num" style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--foreground)" }}>
                        {b.unit === "usd" ? `$${formatCost(b.amount)}` : `${formatNumber(b.amount)} ${b.unit === "token" ? "tokens" : t("platform.detail.countUnit", "次")}`}
                      </td>
                      <td className="num" style={{ padding: "5px 8px 5px 0", borderTop: "1px solid var(--border)", color: "var(--text-secondary)" }}>
                        {b.unit === "usd" ? `$${formatCost(b.consumed)}` : formatNumber(b.consumed)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
            {rlCount === 0 && (p.manual_budgets ?? []).length === 0 && (
              <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>—</span>
            )}
            {hitWin && (
              <div style={{ marginTop: 12, fontSize: 12, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 7 }}>
                <span style={badgeStyle("var(--primary)", "color-mix(in srgb, var(--primary) 10%, transparent)", "color-mix(in srgb, var(--primary) 30%, transparent)")}>Peak</span>
                {peakWindows.map((w, i) => (
                  <span key={i}>{w.start_hour}–{w.end_hour}{i < peakWindows.length - 1 ? " / " : ""}</span>
                ))}
                · <span style={{ color: "var(--primary)", fontFamily: "var(--font-mono, monospace)", fontWeight: 600 }}>×{hitWin.multiplier ?? 1}</span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/** 额度窗口秒数 → 人读（3600→1 小时 / 86400→1 天）。 */
function formatWindowSecs(secs: number): string {
  if (secs % 86400 === 0) return `${secs / 86400} ${secs / 86400 > 1 ? "days" : "day"}`;
  if (secs % 3600 === 0) return `${secs / 3600} ${secs / 3600 > 1 ? "hours" : "hour"}`;
  if (secs % 60 === 0) return `${secs / 60} min`;
  return `${secs}s`;
}
