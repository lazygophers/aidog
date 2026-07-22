import React, { memo, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Platform, Protocol, PlatformUsageStats, LastTestResult, PlatformQuota } from "../../services/api";
import { getPlatformLogo, getFaviconUrl } from "../../assets/platforms";
import { CompactCard, StatChip, BalanceBar, TestResultBody, successRateLevel, costLevel, usageLevelToColor } from "../shared";
import { clamp, formatNumber, formatPercent, formatCostUsd, formatDateTime } from "../../utils/formatters";
import { IconBolt, IconCost, IconCheck, IconClock } from "../icons";
import {
  PROTOCOL_LABELS, HEALTH_COLORS,
  getDefaultModels, getProtocolHomepage, isCodingPlanProtocol, computeManualBudgetDisplay, computeQuotaDisplay,
  allModelValues, tierLabel, formatResetCountdown, formatResetClock, healthStatus,
} from "../../domains/platforms";
import { getProtocolLabel, getProtocolLabelMap, getProtocolColorMap, getDefaultPeakHours } from "../../domains/platforms/defaults";
import { useProtocolLogo } from "../../domains/platforms/useProtocolLogo";
import type { HealthStatus } from "../../domains/platforms";
import { isCurrentlyPeak } from "../../utils/peakHours";
import { parseDisableDuringPeak, parsePlatformPeakHours } from "../../services/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

// ── Props types ──

export interface PlatformCardActions {
  onPointerDown: (e: React.PointerEvent, index: number) => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onPointerUp: () => void;
  onToggleExpanded: (id: number, next: boolean) => void;
  onRefreshQuota: (p: Platform) => void;
  onToggleEnabled: (p: Platform) => void;
  onEdit: (p: Platform) => void;
  onShare: (p: Platform) => void;
  onDuplicate: (p: Platform) => void;
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
  /**
   * quota 原始输入：实时查回的 PlatformQuota（未查则 undefined）。
   * QuotaDisplay 在卡片内 useMemo 计算，避免父组件每渲染现算新对象击穿 memo 浅比较。
   */
  quotaRaw: PlatformQuota | undefined;
  /** 是否优先用真实校准 quota（quotaRealIds 命中）。 */
  quotaPreferReal: boolean;
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
  /**
   * per-group 优先级（1~10，10=最高优先）。仅分组展开区上下文传入；
   * 必须与 onLevelPriorityChange 成对传入才渲染编辑控件（主列表不传 → 不串味）。
   */
  levelPriority?: number;
  /** 改 level_priority 回调（v 已 clamp 调用方负责持久化 + 乐观更新）。 */
  onLevelPriorityChange?: (v: number) => void;
}

// ── PlatformCard 组件 ──

export const PlatformCard = memo(function PlatformCard({
  platform: p,
  index: i,
  isDragging,
  dragActive,
  quotaRaw,
  quotaPreferReal,
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
  levelPriority,
  onLevelPriorityChange,
}: PlatformCardProps) {
  const { t, i18n } = useTranslation();
  // QuotaDisplay 在卡片内计算并缓存：父列表渲染时不再现算新对象，浅比较稳定 → 局部交互不全卡重渲。
  const quota = useMemo(
    () => computeQuotaDisplay(p, quotaRaw, quotaPreferReal),
    [p, quotaRaw, quotaPreferReal],
  );
  const [color, setColor] = useState<string>("var(--accent)");
  useEffect(() => {
    let cancelled = false;
    getProtocolColorMap().then(m => {
      if (!cancelled && m[p.platform_type]) setColor(m[p.platform_type]!);
    });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.platform_type]);
  const hasCodingEndpoint = (p.endpoints ?? []).some(ep => ep.coding_plan);
  // 协议层 coding plan 套餐标记（数据驱动，读 preset.is_coding_plan 真值源；非硬编码协议键名）。
  const [isCpProtocol, setIsCpProtocol] = useState(false);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const flag = await isCodingPlanProtocol(p.platform_type);
      if (!cancelled) setIsCpProtocol(flag);
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.platform_type]);
  // 默认模型异步从 defaults.json 取（4 函数 async 化后），首次渲染为 []，加载完触发更新。
  // PRD 07-11：高峰期切 models.peak 分支。isPeak 判定 = 用户 extra.peak_hours ?? preset peak_hours。
  const [defaultModels, setDefaultModels] = useState<ReturnType<typeof allModelValues>>([]);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const userPh = parsePlatformPeakHours(p.extra ?? "");
      const phWindows = userPh.length > 0 ? userPh : await getDefaultPeakHours(p.platform_type);
      const isPeak = isCurrentlyPeak(phWindows, Date.now());
      const m = await getDefaultModels(p.platform_type, hasCodingEndpoint, isPeak);
      if (!cancelled) setDefaultModels(allModelValues(m));
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.platform_type, hasCodingEndpoint, p.extra]);
  const configuredModels = (() => {
    const explicit = allModelValues(p.models);
    if (explicit.length > 0) return explicit;
    if ((p.available_models?.length ?? 0) > 0) return explicit;
    return defaultModels;
  })();
  const quotaCapable = p.platform_type !== "mock" && p.platform_type !== "claude_code";
  const showQuota = quotaCapable && quota.hasData;
  // ④ 延迟档：可查 quota 的平台数据未回（quotaPending）→ 余额区显骨架而非空白/est 旧值闪烁
  const showQuotaSkeleton = quotaCapable && !quota.hasData && quotaPending;
  const mb = computeManualBudgetDisplay(p.manual_budgets);
  const total = u ? u.total_input_tokens + u.total_output_tokens : 0;
  const sr = u && u.total_requests > 0 ? (u.success_count / u.total_requests * 100) : 0;
  const hasDetail = !!u || usagePending || (p.endpoints && p.endpoints.length > 0) || configuredModels.length > 0 || quota.tiers.length > 0;
  // 健康点派生（R4，纯前端，不加后端字段）：优先按 status + last_error 综合最近健康——
  //   红 = key 失效（auto_disabled 且 last_error 为 401/403）；
  //   黄 = 有 last_error 但可恢复（402/429/5xx/连接失败等）；
  //   绿 = enabled 且无 last_error。其余（手动 disabled 无 error、mock）回退 manual/成功率派生。
  const keyInvalid = p.status === "auto_disabled"
    && (p.last_error?.startsWith("HTTP 401") || p.last_error?.startsWith("HTTP 403"));
  const health: HealthStatus = keyInvalid
    ? "error"
    : p.last_error
      ? "warning"
      : p.status === "enabled"
        ? "healthy"
        : manual
          ? (manual === "ok" ? "healthy" : "error")
          : u ? healthStatus(u.recent_total, u.recent_failures) : "unknown";
  const logoSvg = getPlatformLogo(p.platform_type);
  const favicon = !logoSvg && !faviconHasFailed ? getFaviconUrl(p) : null;
  // 缓存 logo（~/.aidog/logos/<protocol>.png，logo_sync 同步）— Layer 0，优先级最高
  // ponytail: hook 内部 mount 查路径 + miss 触发后台同步 + onError 清空 fallback 下层
  const { logoSrc: cachedLogo } = useProtocolLogo(p.platform_type);
  const [cachedLogoFailed, setCachedLogoFailed] = useState(false);
  const cachedLogoUrl = cachedLogo && !cachedLogoFailed ? cachedLogo : null;
  // 平台详情外链（protocol homepage；未配置则不渲染）
  const [homepage, setHomepage] = useState<string>("");
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const hp = await getProtocolHomepage(p.platform_type);
      if (!cancelled) setHomepage(hp);
    })();
    return () => { cancelled = true; };
  }, [p.platform_type]);
  // 协议本地化 label（fallback: PROTOCOL_LABELS → key）
  const [protocolLabel, setProtocolLabel] = useState("");
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const label = await getProtocolLabel(p.platform_type, i18n.language);
      if (!cancelled) setProtocolLabel(label);
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i18n.language, p.platform_type]);
  // 批量协议 labelMap（endpoint badge 覆盖所有 ep.protocol，单 protocol state 不够）
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolLabelMap(i18n.language).then(m => { if (!cancelled) setLabelMap(m); });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i18n.language]);
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
                  background: (cachedLogoUrl || logoSvg || favicon) ? "transparent" : `${color}15`,
                  border: `1px solid ${color}30`,
                  color: color, fontSize: 12, fontWeight: 700, overflow: "hidden",
                }}>
                  {cachedLogoUrl
                    ? <img src={cachedLogoUrl} alt={p.platform_type} style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }}
                        onError={() => setCachedLogoFailed(true)} />
                    : logoSvg
                      ? <img src={logoSvg} alt={p.platform_type} style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
                      : favicon
                        ? <img src={favicon} alt={p.platform_type}
                            style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }}
                            onError={() => actions.onFaviconFailed(p.id)}
                          />
                        : p.platform_type.slice(0, 2).toUpperCase()
                  }
                </div>
                {/* 健康点常驻（R4）：status + last_error 派生（红=key 失效 / 黄=可恢复 / 绿=正常），
                    无 last_error 时回退成功率/manual；title 复用 lastError 提示文案。 */}
                <div
                  style={{
                    position: "absolute", top: -3, right: -3,
                    width: 10, height: 10, borderRadius: "50%",
                    background: HEALTH_COLORS[health],
                    border: "2px solid var(--bg-primary)",
                    boxShadow: `0 0 4px ${HEALTH_COLORS[health]}60`,
                  }}
                  title={p.last_error
                    ? t("platform.lastErrorHint", "最近一次失败 · {{time}}\n{{error}}")
                        .replace("{{time}}", formatDateTime(p.last_error_at) ?? "")
                        .replace("{{error}}", p.last_error)
                    : undefined}
                />
              </div>
              {/* 名称 + 协议·base_url */}
              <div style={{ minWidth: 0, flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: 14, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.name}</div>
                {/* Coding Plan 套餐协议徽标（数据驱动：读 preset.is_coding_plan，非硬编码协议键名） */}
                {isCpProtocol && (
                  <div
                    style={{
                      marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
                      fontSize: 10, fontWeight: 600, color: "var(--color-success)",
                      background: "color-mix(in srgb, var(--color-success) 12%, transparent)",
                      border: "1px solid color-mix(in srgb, var(--color-success) 30%, transparent)",
                      borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
                    }}
                    title={t("platform.codingPlanHint", "Coding Plan 套餐协议：走独立子域 / 配额计费")}
                  >
                    {t("platform.codingPlanBadge", "Coding Plan")}
                  </div>
                )}
                <div className="text-secondary" style={{ fontSize: 11, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {protocolLabel || p.platform_type} · {getBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url}
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
                      .replace("{{time}}", p.auto_disabled_until > 0 ? (formatDateTime(p.auto_disabled_until) ?? "-") : "-")}
                  >
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M12 9v4" /><path d="M12 17h.01" />
                      <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                    </svg>
                    {t("platform.autoDisabled", "自动禁用")}
                  </div>
                )}
                {/* 高峰禁用中徽标（独立维度，与 status 正交）：开关 on && now 在 peak window 命中 → 实时显 */}
                {parseDisableDuringPeak(p.extra ?? "") && isCurrentlyPeak(parsePlatformPeakHours(p.extra ?? ""), Date.now()) && (
                  <div
                    style={{
                      marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
                      fontSize: 10, fontWeight: 600, color: "var(--color-warning)",
                      background: "color-mix(in srgb, var(--color-warning) 14%, transparent)",
                      border: "1px solid color-mix(in srgb, var(--color-warning) 35%, transparent)",
                      borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
                    }}
                    title={t("platform.disable_during_peak_desc", "启用后该平台在高峰时段从路由候选排除（不改 status，临时闸门）。")}
                  >
                    {t("platform.peak_disabled_badge", "高峰禁用中")}
                  </div>
                )}
                {/* 高峰生效态徽标（R6 UI 可见性）：平台有 peak_hours && 当前命中 → 显示。
                    model scope 限定时徽标显「高峰·N模型」+ tooltip 列模型；非限定显「高峰」。
                    disable_during_peak 已有「高峰禁用中」徽标时跳过（避免重复）。 */}
                {!parseDisableDuringPeak(p.extra ?? "") && (() => {
                  const phWindows = parsePlatformPeakHours(p.extra ?? "");
                  if (phWindows.length === 0) return null;
                  const nowMs = Date.now();
                  if (!isCurrentlyPeak(phWindows, nowMs)) return null;
                  // 复用 isCurrentlyPeak 单窗口调用找当前命中窗口（取其 model scope）。
                  const hitWin = phWindows.find(w => isCurrentlyPeak([w], nowMs));
                  const models = hitWin?.models;
                  const hasScope = models && models.length > 0;
                  const tooltip = hasScope
                    ? t("platform.peak_badge_models_tooltip", "受影响模型：{{models}}")
                        .replace("{{models}}", models!.join(", "))
                    : t("platform.peak_hours", "高峰时段倍率");
                  return (
                    <div
                      style={{
                        marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
                        fontSize: 10, fontWeight: 600, color: "var(--accent)",
                        background: "color-mix(in srgb, var(--accent) 12%, transparent)",
                        border: "1px solid color-mix(in srgb, var(--accent) 30%, transparent)",
                        borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
                      }}
                      title={tooltip}
                    >
                      {hasScope
                        ? t("platform.peak_badge_limited", "高峰·{{count}}模型")
                            .replace("{{count}}", String(models!.length))
                        : t("platform.peak_badge", "高峰")}
                    </div>
                  );
                })()}
                {/* 过期标记（独立维度，与 status 正交）：已过期显红 badge，未过期临近时显小字 */}
                {p.expires_at > 0 && (() => {
                  const nowMs = Date.now();
                  if (nowMs >= p.expires_at) {
                    return (
                      <div
                        style={{
                          marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4,
                          fontSize: 10, fontWeight: 600, color: "var(--color-danger)",
                          background: "color-mix(in srgb, var(--color-danger) 14%, transparent)",
                          border: "1px solid color-mix(in srgb, var(--color-danger) 35%, transparent)",
                          borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
                        }}
                        title={t("platform.expiredHint", "已过期：{{time}}，路由自动排除")
                          .replace("{{time}}", formatDateTime(p.expires_at) ?? "")}
                      >
                        {t("platform.expired", "已过期")}
                      </div>
                    );
                  }
                  const soon = p.expires_at - nowMs < 86_400_000; // 24h 内高亮
                  return (
                    <div
                      style={{
                        marginTop: 3, fontSize: 10, whiteSpace: "nowrap",
                        color: soon ? "var(--color-warning)" : "var(--text-tertiary)",
                        fontWeight: soon ? 600 : 500,
                      }}
                      title={t("platform.expiresAtHint", "可选。到期后该平台自动从路由候选排除（等效禁用），改值或清空即恢复。")}
                    >
                      {t("platform.expiresAtBadge", "到期 {{time}}")
                        .replace("{{time}}", formatDateTime(p.expires_at) ?? "")}
                    </div>
                  );
                })()}
                {/* 所属分组 badge */}
                {platformMembership && platformMembership.length > 0 && (
                  <div style={{ marginTop: 3, display: "flex", gap: 4, flexWrap: "wrap" }}>
                    {platformMembership.map(gName => (
                      <Badge key={gName} variant="secondary" style={{ fontSize: 10, padding: "1px 6px" }}>
                        {gName}
                      </Badge>
                    ))}
                  </div>
                )}
                {/* 最近一次测试结果徽章（常驻；无记录不渲染） */}
                {lastTest && <LastTestBadge result={lastTest} />}
                {/* 最近一次代理错误（系统维护，非请求记录实时取；最近一次成功即清空不渲染） */}
                {p.last_error && (
                  <div
                    style={{
                      marginTop: 3, display: "inline-flex", alignItems: "center", gap: 4, maxWidth: "100%",
                      fontSize: 10, fontWeight: 600, color: "var(--color-danger)",
                      background: "color-mix(in srgb, var(--color-danger) 14%, transparent)",
                      border: "1px solid color-mix(in srgb, var(--color-danger) 35%, transparent)",
                      borderRadius: 5, padding: "1px 6px",
                    }}
                    title={t("platform.lastErrorHint", "最近一次失败 · {{time}}\n{{error}}")
                      .replace("{{time}}", (p.last_error_at ?? 0) > 0 ? (formatDateTime(p.last_error_at) ?? "-") : "-")
                      .replace("{{error}}", p.last_error)}
                  >
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
                      <circle cx="12" cy="12" r="10" /><path d="M12 8v4" /><path d="M12 16h.01" />
                    </svg>
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {t("platform.lastError", "最近错误")}: {p.last_error}
                    </span>
                  </div>
                )}
              </div>
              {/* 快操作 */}
              <PlatformActionButtons
                canRefresh={quotaCapable}
                refreshing={refreshing}
                testing={testing}
                platform={p}
                actions={actions}
              />
            </div>
            {/* ── 行 1.5：per-group 优先级编辑（仅分组展开区上下文，主列表不渲染） ── */}
            {onLevelPriorityChange && (
              <LevelPriorityControl
                value={levelPriority ?? 5}
                onChange={onLevelPriorityChange}
              />
            )}
            {/* ── 行 2：余额 / 预算 / coding tiers ── */}
            {showQuota && (quota.balanceRemaining != null || (mb && mb.hasData) || (quota.balanceRemaining == null && quota.tiers.length > 0)) && (
              <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", paddingLeft: 24 }}>
                {/* 余额 */}
                {quota.balanceRemaining != null && (() => {
                  const balColor = usageLevelToColor(p.balance_level);
                  const isAcu = quota.currency === "ACU";
                  return (
                    <div style={{ flexShrink: 0, width: 120, display: "flex", flexDirection: "column", gap: 2 }}>
                      <BalanceBar
                        remaining={quota.balanceRemaining}
                        total={quota.balanceTotal}
                        currency={isAcu ? "" : (quota.currency === "USD" ? "$" : quota.currency)}
                        level={balColor === "neutral" ? undefined : balColor}
                        label={isAcu ? t("platform.acuUsage", "ACU 用量") : undefined}
                      />
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
                {/* Coding plan：已用 tokens + 预估金额（折叠态亦可见，配额档无此数据） */}
                {hasCodingEndpoint && u && (
                  <div style={{ flexShrink: 0, display: "inline-flex", alignItems: "center", gap: 8 }} title={t("platform.codingUsedHint", "本平台累计已用 tokens 与预估金额（基于实际请求估算）")}>
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, fontWeight: 700, color: "var(--text-secondary)" }}>
                      <IconBolt size={12} />
                      {formatNumber(total)}
                      <span style={{ fontSize: 9, fontWeight: 600, opacity: 0.6 }}>tok</span>
                    </span>
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, fontWeight: 700, color: "var(--text-secondary)" }}>
                      <IconCost size={12} />
                      {formatCostUsd(u.total_cost)}
                    </span>
                  </div>
                )}
                {/* Coding plan tiers */}
                {quota.balanceRemaining == null && quota.tiers.length > 0 && (
                  <div style={{ flexShrink: 0, display: "flex", gap: 4, flexWrap: "wrap", maxWidth: 300 }}>
                    {quota.tiers.map(tier => {
                      const isMcp = tier.name === "mcp_monthly";
                      const value = isMcp && tier.limit != null
                        ? `${tier.remaining ?? 0}/${tier.limit}`
                        : `${tier.remainPct.toFixed(0)}%`;
                      const remainSuffix = t("platform.quotaRemainSuffix", "剩");
                      const tierColor = tier.level === "danger" ? "var(--color-danger)" : tier.level === "warning" ? "var(--color-warning)" : tier.level === "success" ? "var(--color-success)" : "var(--text-secondary)";
                      const countdown = formatResetCountdown(tier.resetsAt);
                      const resetClock = formatResetClock(tier.resetsAt);
                      // ponytail: 进度条块（紧凑态）— 进度条 + 主数 + 档名/倒计时 + clock 四层，沿用 tier.level 色口径
                      return (
                        <div key={tier.name} style={{
                          display: "flex", flexDirection: "column", gap: 2,
                          padding: "3px 6px", borderRadius: "var(--radius-sm)",
                          minWidth: 64, maxWidth: 120,
                          background: "var(--bg-glass)",
                          border: "1px solid var(--border)",
                        }}>
                          <div style={{ height: 4, borderRadius: "var(--radius-sm)", background: "var(--bg-primary)", overflow: "hidden" }}>
                            <div style={{
                              width: `${Math.max(0, Math.min(100, tier.remainPct))}%`, height: "100%",
                              background: tierColor, borderRadius: "var(--radius-sm)",
                              transition: "width 0.3s ease",
                            }} />
                          </div>
                          <span style={{ fontSize: 11, fontWeight: 700, color: "var(--text-primary)", lineHeight: 1.1 }}>
                            {value}<span style={{ fontSize: 8, fontWeight: 600, opacity: 0.65, marginLeft: 1 }}>{remainSuffix}</span>
                          </span>
                          <span style={{ fontSize: 9, color: "var(--text-tertiary)", whiteSpace: "nowrap", lineHeight: 1.1 }}>
                            {tierLabel(tier.name)}{countdown && ` ·${countdown}`}
                          </span>
                          {resetClock && (
                            <span style={{ fontSize: 8, color: "var(--text-tertiary)", whiteSpace: "nowrap", lineHeight: 1.1 }}>
                              {resetClock}
                            </span>
                          )}
                        </div>
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
            {/* 官网外链（platform-presets.json homepage 字段，未配置则不渲染） */}
            {homepage && (
              <div>
                <a
                  href={homepage}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{
                    fontSize: 11, color: "var(--accent)",
                    textDecoration: "none", display: "inline-flex", alignItems: "center", gap: 4,
                  }}
                  onClick={(e) => e.stopPropagation()}
                  title={homepage}
                >
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
                    <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
                  </svg>
                  {t("platform.homepage", "Homepage")}
                </a>
              </div>
            )}
            {/* 已使用统计（总计 + 今日） */}
            <UsageSection
              usage={u}
              usagePending={usagePending}
              totalTokens={total}
              totalCost={u?.total_cost ?? 0}
              successRate={sr}
              totalRequests={u?.total_requests ?? 0}
            />
            {/* 配额各档明细 */}
            {showQuota && quota.tiers.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.quotaLabel", "额度")}</span>
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  {quota.tiers.map(tier => {
                    const isMcp = tier.name === "mcp_monthly";
                    const value = isMcp && tier.limit != null
                      ? `${tier.remaining ?? 0}/${tier.limit}`
                      : `${tier.remainPct.toFixed(0)}%`;
                    const remainSuffix = t("platform.quotaRemainSuffix", "剩");
                    const tierColor = tier.level === "danger" ? "var(--color-danger)" : tier.level === "warning" ? "var(--color-warning)" : tier.level === "success" ? "var(--color-success)" : "var(--text-secondary)";
                    const countdown = formatResetCountdown(tier.resetsAt);
                    const resetClock = formatResetClock(tier.resetsAt);
                    // ponytail: 进度条块（展开态）— 同款更大版，进度条宽度 = remainPct% 色 = tier.level 语义色
                    return (
                      <div key={tier.name} style={{
                        display: "flex", flexDirection: "column", gap: 4,
                        padding: "6px 10px", borderRadius: "var(--radius-sm)",
                        minWidth: 96, maxWidth: 150,
                        background: "var(--bg-glass)",
                        border: "1px solid var(--border)",
                      }}>
                        <div style={{ height: 5, borderRadius: "var(--radius-sm)", background: "var(--bg-primary)", overflow: "hidden" }}>
                          <div style={{
                            width: `${Math.max(0, Math.min(100, tier.remainPct))}%`, height: "100%",
                            background: tierColor, borderRadius: "var(--radius-sm)",
                            transition: "width 0.3s ease",
                          }} />
                        </div>
                        <span style={{ fontSize: 13, fontWeight: 700, color: "var(--text-primary)", lineHeight: 1.1 }}>
                          {value}<span style={{ fontSize: 9, fontWeight: 600, opacity: 0.65, marginLeft: 2 }}>{remainSuffix}</span>
                        </span>
                        <span style={{ fontSize: 11, color: "var(--text-tertiary)", display: "inline-flex", alignItems: "center", gap: 4, whiteSpace: "nowrap" }}>
                          <span>{tierLabel(tier.name)}</span>
                          {countdown && (
                            <span style={{ display: "inline-flex", alignItems: "center", gap: 2 }}>
                              <IconClock size={11} />
                              {countdown}{resetClock && ` · ${resetClock}`}
                            </span>
                          )}
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
            {/* Endpoints badges */}
            <EndpointsSection
              endpoints={p.endpoints}
              labelMap={labelMap}
              configuredModels={configuredModels}
            />
          </div>
        )}
      </CompactCard>
    </div>
  );
});

// ── Endpoints & 模型展示区 ──

function EndpointsSection({
  endpoints,
  labelMap,
  configuredModels,
}: {
  endpoints: Platform["endpoints"];
  labelMap: Record<string, string>;
  configuredModels: string[];
}) {
  const { t } = useTranslation();
  return (
    <>
      {endpoints && endpoints.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.endpoints", "Protocol Endpoints")}</span>
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {endpoints.map((ep, ei) => (
              <Badge key={ei} variant="secondary" style={{ fontSize: 10, padding: "1px 6px", opacity: 0.85 }}>
                {labelMap?.[ep.protocol] || PROTOCOL_LABELS[ep.protocol] || ep.protocol}
                {ep.coding_plan && <span style={{ color: "var(--color-success)", marginLeft: 2, fontWeight: 700 }}>Code</span>}
              </Badge>
            ))}
          </div>
        </div>
      )}
      {configuredModels.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.models")}</span>
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {configuredModels.map((m, mi) => (
              <Badge key={mi} variant="secondary" style={{ fontSize: 11, padding: "2px 6px" }}>{m}</Badge>
            ))}
          </div>
        </div>
      )}
    </>
  );
}

// ── 用量统计区 ──

function UsageSection({
  usage,
  usagePending,
  totalTokens,
  totalCost,
  successRate,
  totalRequests,
}: {
  usage: PlatformUsageStats | undefined;
  usagePending: boolean;
  totalTokens: number;
  totalCost: number;
  successRate: number;
  totalRequests: number;
}) {
  const { t } = useTranslation();
  return (
    <>
      {usage && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.usageLabel", "已使用")}</span>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              <StatChip icon={<IconBolt size={13} />} value={formatNumber(totalTokens)} label="tokens" />
              <StatChip icon={<IconCost size={13} />} value={formatCostUsd(totalCost)} label="cost" level={costLevel(totalCost)} />
              <StatChip icon={<IconCheck size={13} />} value={formatPercent(successRate)} label="ok" level={successRateLevel(successRate, totalRequests)} />
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.todayUsageLabel", "今日")}</span>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              <StatChip icon={<IconBolt size={13} />} value={formatNumber(usage.today_tokens)} label="tokens" />
              <StatChip icon={<IconCost size={13} />} value={formatCostUsd(usage.today_cost)} label="cost" level={costLevel(usage.today_cost)} />
            </div>
          </div>
        </div>
      )}
      {!usage && usagePending && (
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span className="text-tertiary" style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}>{t("platform.usageLabel", "已使用")}</span>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
            <span className="skeleton" style={{ width: 80, height: 24 }} aria-label={t("platform.usageLoading", "用量加载中")} />
            <span className="skeleton" style={{ width: 70, height: 24 }} />
            <span className="skeleton" style={{ width: 60, height: 24 }} />
          </div>
        </div>
      )}
    </>
  );
}

// ── 平台快操作按钮组 ──

function PlatformActionButtons({
  canRefresh,
  refreshing,
  testing,
  platform,
  actions,
}: {
  canRefresh: boolean;
  refreshing: boolean;
  testing: boolean;
  platform: Platform;
  actions: PlatformCardActions;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", gap: 4, flexShrink: 0, alignItems: "center" }}>
      {canRefresh && (
        <Button
          variant="ghost"
          size="icon"
          style={{ padding: 4, height: "auto", minWidth: "auto" }}
          disabled={refreshing}
          title={t("platform.quotaRefresh", "刷新额度")}
          onClick={(e) => { e.stopPropagation(); actions.onRefreshQuota(platform); }}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"
            style={refreshing ? { animation: "spin 0.9s linear infinite" } : undefined}>
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <polyline points="21 3 21 9 15 9" />
          </svg>
        </Button>
      )}
      <div
        className={`toggle ${platform.status === "enabled" ? "active" : ""}`}
        style={{ cursor: "pointer" }}
        onClick={(e) => { e.stopPropagation(); actions.onToggleEnabled(platform); }}
        title={platform.status === "enabled"
          ? t("platform.disable", "禁用")
          : platform.status === "auto_disabled"
            ? t("platform.reenable", "重新启用")
            : t("platform.enable", "启用")}
      />
      <div style={{ display: "inline-flex", fontSize: 11 }}>
        <Button
          variant="ghost"
          style={{ fontSize: 11, gap: 4, padding: "3px 8px", height: "auto", borderRadius: "6px 0 0 6px", borderRight: "1px solid var(--border)" }}
          disabled={testing}
          onClick={(e) => { e.stopPropagation(); actions.onQuickTest(platform); }}
          title={t("platform.quickTest", "快速测试默认模型")}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" stroke="none">
            <path d="M13 2L4 14h7l-2 8 9-12h-7l2-8z"/>
          </svg>
        </Button>
        <Button
          variant="ghost"
          style={{ fontSize: 11, padding: "3px 6px", height: "auto", borderRadius: "0 6px 6px 0" }}
          onClick={(e) => { e.stopPropagation(); actions.onCustomTest(platform); }}
          title={t("platform.customTest", "自定义测试")}
        >
          <svg width="10" height="10" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 5l4 4 4-4" />
          </svg>
        </Button>
      </div>
      <Button variant="ghost" size="icon" style={{ height: "auto" }} title={t("platform.viewLogs", "查看日志")} onClick={(e) => { e.stopPropagation(); actions.onViewLogs(platform); }}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M2 2h10v10H2z" />
          <path d="M4 5h6M4 7h4M4 9h5" />
        </svg>
      </Button>
      <Button variant="ghost" size="icon" style={{ height: "auto" }} onClick={(e) => { e.stopPropagation(); actions.onEdit(platform); }}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M10 2l2 2-7 7H3v-2l7-7z" />
        </svg>
      </Button>
      <Button variant="ghost" size="icon" style={{ height: "auto" }} title={t("platform.share.button", "分享")} onClick={(e) => { e.stopPropagation(); actions.onShare(platform); }}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="11" cy="3" r="1.6" />
          <circle cx="3" cy="7" r="1.6" />
          <circle cx="11" cy="11" r="1.6" />
          <path d="M4.4 6.1l5.2-2.4M4.4 7.9l5.2 2.4" />
        </svg>
      </Button>
      <Button variant="ghost" size="icon" style={{ height: "auto" }} title={t("platform.duplicate", "复制")} onClick={(e) => { e.stopPropagation(); actions.onDuplicate(platform); }}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="4" y="4" width="8" height="8" rx="1.2" />
          <path d="M9 4V3a1 1 0 0 0-1-1H3a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h1" />
        </svg>
      </Button>
      <Button variant="ghost" size="icon" style={{ height: "auto", color: "var(--color-danger)" }} onClick={(e) => { e.stopPropagation(); actions.onDelete(platform.id); }}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
        </svg>
      </Button>
    </div>
  );
}

// ── 最近一次测试徽章 ──

/** 毫秒 epoch → 相对时间简写（N 分钟/小时/天）。
 * ponytail: 保留本地实现而非 formatRelativeTime(formatters.ts)，
 * 因为后者返回完整文案（"刚刚"/"N 分钟前"）本组件需简写（"3m"/"5h"/"2d"）且 <1 分钟时空字符串。
 * 改用 formatRelativeTime 会导致 LastTestBadge 快照变化（增加 "刚刚"）破坏回归。
 */
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

// ── per-group 优先级编辑控件（stepper，1~10，10=最高优先） ──

export function LevelPriorityControl({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const { t } = useTranslation();
  // 按钮即调：上下按钮=明确确认动作，保留 clamp + 立即 onChange
  const set = (v: number) => {
    const next = clamp(v, 1, 10);
    if (next !== value) onChange(next);
  };
  // ponytail: 输入框走本地态编辑，blur/Enter 时 clamp+提交，避免按键中间态打后端
  const [local, setLocal] = useState(String(value));
  // 编辑标记：blur 提交前的输入态，防外部 value 变化(回滚)覆盖正在编辑的 local
  const editingRef = useRef(false);
  useEffect(() => {
    if (!editingRef.current) setLocal(String(value));
  }, [value]);
  const commit = () => {
    editingRef.current = false;
    const v = parseInt(local, 10);
    if (Number.isNaN(v)) { setLocal(String(value)); return; }
    const next = clamp(v, 1, 10);
    if (next !== value) onChange(next);
    setLocal(String(next)); // 同步显示为 clamp 后值（范围外静默纠正：99→10, 0→1）
  };
  const btnStyle: React.CSSProperties = {
    width: 22, height: 22, minWidth: 22, padding: 0, lineHeight: 0,
    display: "inline-flex", alignItems: "center", justifyContent: "center",
  };
  return (
    <div
      style={{ display: "flex", alignItems: "center", gap: 8, paddingLeft: 24, flexWrap: "wrap" }}
      onClick={e => e.stopPropagation()}
    >
      <span
        className="text-tertiary"
        style={{ fontSize: 10, fontWeight: 600, letterSpacing: 0.3 }}
        title={t("group.levelPriorityHint", "数值越大优先级越高（10=最高优先），仅在本分组生效")}
      >
        {t("group.levelPriority", "优先级")}
      </span>
      <div style={{ display: "inline-flex", alignItems: "center", gap: 2 }}>
        <Button
          variant="ghost"
          size="icon"
          style={btnStyle}
          disabled={value <= 1}
          title={t("group.levelPriorityDown", "降低优先级")}
          onClick={e => { e.stopPropagation(); set(value - 1); }}
        >
          <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M3 7h8" /></svg>
        </Button>
        <Input
          type="number"
          min={1}
          max={10}
          value={local}
          onClick={e => e.stopPropagation()}
          onFocus={() => { editingRef.current = true; }}
          onChange={e => setLocal(e.target.value)}
          onBlur={commit}
          onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); (e.target as HTMLInputElement).blur(); } }}
          style={{
            width: 38, height: "auto", textAlign: "center", fontSize: 12, fontWeight: 700,
            padding: "2px 4px", borderRadius: "var(--radius-sm)",
            background: "var(--bg-glass)", border: "1px solid var(--border)",
            color: "var(--text-primary)",
          }}
        />
        <Button
          variant="ghost"
          size="icon"
          style={btnStyle}
          disabled={value >= 10}
          title={t("group.levelPriorityUp", "提高优先级")}
          onClick={e => { e.stopPropagation(); set(value + 1); }}
        >
          <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M7 3v8M3 7h8" /></svg>
        </Button>
      </div>
      <span className="text-tertiary" style={{ fontSize: 9, opacity: 0.7 }}>
        {t("group.levelPriorityMax", "10=最高优先")}
      </span>
    </div>
  );
}

function LastTestBadge({ result }: { result: LastTestResult }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const ok = result.success;
  const color = ok ? "var(--color-success)" : "var(--color-danger)";
  const rel = relativeTime(result.created_at);
  const errorText = !ok && result.error ? result.error.slice(0, 30) : "";
  const hasBody = result.response_body.trim().length > 0;
  return (
    <div style={{ marginTop: 3, maxWidth: "100%" }}>
      <Button
        type="button"
        variant="ghost"
        onClick={hasBody ? () => setOpen(o => !o) : undefined}
        style={{
          display: "inline-flex", alignItems: "center", gap: 4,
          fontSize: 10, fontWeight: 600, color,
          background: `color-mix(in srgb, ${color} 12%, transparent)`,
          border: `1px solid color-mix(in srgb, ${color} 30%, transparent)`,
          borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap", maxWidth: "100%",
          height: "auto",
          cursor: hasBody ? "pointer" : "default",
        }}
        title={ok
          ? t("platform.lastTestOkHint", "最近测试通过 · {{time}}", { time: formatDateTime(result.created_at) ?? "" })
          : t("platform.lastTestFailHint", "最近测试失败 · {{time}}{{error}}", {
              time: formatDateTime(result.created_at) ?? "",
              error: result.error ? `\n${result.error}` : "",
            })}
      >
        <span style={{ fontWeight: 700 }}>{ok ? "✓" : "✗"}</span>
        {result.duration_ms > 0 && <span>{result.duration_ms}ms</span>}
        {rel && <span style={{ opacity: 0.85 }}>· {rel}</span>}
        {!ok && errorText && (
          <span style={{ opacity: 0.85, overflow: "hidden", textOverflow: "ellipsis", maxWidth: 120 }}>{errorText}</span>
        )}
        {hasBody && <span style={{ opacity: 0.7 }}>{open ? "▾" : "▸"}</span>}
      </Button>
      {open && hasBody && (
        <div className="glass-surface" style={{ marginTop: 4, padding: "6px 8px", borderRadius: 6, maxWidth: "100%" }}>
          <TestResultBody body={result.response_body} />
        </div>
      )}
    </div>
  );
}
