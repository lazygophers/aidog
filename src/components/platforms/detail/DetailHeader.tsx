// 详情页头（板块 A）：平台身份 + 协议/订阅/自动禁用徽标 + 健康三态灯 + 启停开关 + 所属分组。
// 健康三态/徽标判定复用卡片同款（deriveHealth + useProtocolMeta），配色取主题 token。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Platform } from "../../../services/api";
import { getPlatformLogo } from "../../../assets/platforms";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Pill } from "../Pill";
import { HEALTH_COLORS, deriveHealth } from "../../../domains/platforms";
import { useProtocolMeta } from "../../../domains/platforms/useProtocolMeta";
import { useProtocolLogo } from "../../../domains/platforms/useProtocolLogo";
import { getPrimaryBaseUrl } from "../../../pages/platforms/usePlatformQuota";
import { formatDateTime } from "../../../utils/formatters";
import type { PlatformCardService } from "../PlatformCard";

export function DetailHeader({ platform: p, membership, onBack, service }: {
  platform: Platform;
  membership?: string[];
  onBack: () => void;
  service: PlatformCardService;
}) {
  const { t, i18n } = useTranslation();
  const { color, isCpProtocol, protocolLabel } = useProtocolMeta(p.platform_type, p.extra ?? "", i18n.language);
  const health = deriveHealth({ status: p.status, lastError: p.last_error, manual: undefined });
  const logoSvg = getPlatformLogo(p.platform_type);
  const { logoSrc: cachedLogo } = useProtocolLogo(p.platform_type);
  const [cachedLogoFailed, setCachedLogoFailed] = useState(false);
  const cachedLogoUrl = cachedLogo && !cachedLogoFailed ? cachedLogo : null;
  const url = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url;

  return (
    <header style={{
      display: "flex", alignItems: "center", gap: 12, minWidth: 0,
      padding: "14px 20px", borderBottom: "1px solid var(--border)", flexWrap: "wrap",
    }}>
      <Button
        variant="ghost" size="sm"
        data-testid="detail-back"
        onClick={onBack}
        title={t("platform.back", "返回")}
        style={{ flexShrink: 0 }}
      >
        <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M9 2 4 7l5 5" />
        </svg>
        {t("platform.back", "返回")}
      </Button>
      {/* Logo + 健康三态灯（与卡片同款派生） */}
      <div style={{ position: "relative", flexShrink: 0 }}>
        <div style={{
          width: 36, height: 36, borderRadius: "var(--radius-sm)",
          display: "flex", alignItems: "center", justifyContent: "center",
          background: (cachedLogoUrl || logoSvg) ? "transparent" : `${color}15`,
          border: `1px solid ${color}30`,
          color: color, fontSize: 14, fontWeight: 700, overflow: "hidden",
        }}>
          {cachedLogoUrl
            ? <img src={cachedLogoUrl} alt={p.platform_type} style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }}
                onError={() => setCachedLogoFailed(true)} />
            : logoSvg
              ? <img src={logoSvg} alt={p.platform_type} style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
              : p.platform_type.slice(0, 2).toUpperCase()
          }
        </div>
        <div
          data-testid="detail-health-dot"
          style={{
            position: "absolute", top: -3, right: -3,
            width: 10, height: 10, borderRadius: "50%",
            background: HEALTH_COLORS[health],
            border: "2px solid var(--color-card)",
            boxShadow: `0 0 4px ${HEALTH_COLORS[health]}60`,
          }}
          title={p.last_error
            ? t("platform.lastErrorHint", "最近一次失败 · {{time}}\n{{error}}")
                .replace("{{time}}", formatDateTime(p.last_error_at) ?? "")
                .replace("{{error}}", p.last_error)
            : undefined}
        />
      </div>
      {/* 名称 + 徽标 + 协议·base_url 副行 */}
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0, flexWrap: "wrap" }}>
          <span style={{ fontSize: 14, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.name}</span>
          {isCpProtocol && (
            <Pill color="var(--color-success)" title={t("platform.codingPlanHint", "Coding Plan 套餐协议：走独立子域 / 配额计费")}>
              {t("platform.codingPlanBadge", "Coding Plan")}
            </Pill>
          )}
          {p.platform_type === "claude_code" && (
            <Pill color="var(--accent)" title={t("platform.subscriptionHint", "订阅透传：OAuth 订阅绑定，流量经 MITM 观测（不计费，按窗口限额）")}>
              {t("platform.subscriptionBadge", "订阅")}
            </Pill>
          )}
          {p.status === "auto_disabled" && (
            <Pill color="var(--color-warning)" title={t("platform.autoDisabledHint", "401/403 自动禁用，下次试探时间 {{time}}")
              .replace("{{time}}", p.auto_disabled_until > 0 ? (formatDateTime(p.auto_disabled_until) ?? "-") : "-")}>
              {t("platform.autoDisabled", "自动禁用")}
            </Pill>
          )}
        </div>
        <div className="text-secondary" style={{ fontSize: 14, marginTop: 2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontFamily: "var(--font-mono, monospace)" }}>
          {p.platform_type === "unknown" ? t("platform.unknownProtocol", "未知协议") : protocolLabel || p.platform_type} · {url}
        </div>
        {membership && membership.length > 0 && (
          <div style={{ marginTop: 3, display: "flex", gap: 4, flexWrap: "wrap" }}>
            {membership.map(gName => (
              <Badge key={gName} variant="secondary" style={{ fontSize: 14, padding: "1px 6px" }}>
                {gName}
              </Badge>
            ))}
          </div>
        )}
      </div>
      {/* 启停开关（与卡片同款 toggle） */}
      <div
        data-testid="detail-toggle"
        className={`toggle ${p.status === "enabled" ? "active" : ""}`}
        style={{ cursor: "pointer", flexShrink: 0 }}
        onClick={() => service.onToggleEnabled(p)}
        title={p.status === "enabled"
          ? t("platform.disable", "禁用")
          : p.status === "auto_disabled"
            ? t("platform.reenable", "重新启用")
            : t("platform.enable", "启用")}
      />
    </header>
  );
}
