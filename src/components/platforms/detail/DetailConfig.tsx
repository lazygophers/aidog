// 配置板块 H（只读）：端点 / 模型槽位链 / 限频 / 熔断 / 高峰倍率 / 分组 / 客户端白名单。
// 数据全部来自 platform 字段 + extra 解析 + registry preset 派生（useProtocolMeta），无新 RPC。
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { Platform } from "../../../services/api";
import {
  parseAllowedClients, parsePlatformBreaker,
  parseRateLimitsBundle, rateLimitsSummary,
} from "../../../services/api";
import { MODEL_SLOTS, PROTOCOL_LABELS } from "../../../domains/platforms";
import { slotList } from "../../../domains/platforms/slotValue";
import { useProtocolMeta } from "../../../domains/platforms/useProtocolMeta";
import { rtlIsolate } from "../../../utils/formatters";
import { Blk } from "./Blk";

function Row({ k, v }: { k: ReactNode; v: ReactNode }) {
  return (
    <div style={{
      display: "grid", gridTemplateColumns: "92px 1fr", gap: 10,
      padding: "6px 0", borderBottom: "1px dashed var(--border)", fontSize: 14, alignItems: "baseline",
    }}>
      <span style={{ color: "var(--text-secondary)" }}>{k}</span>
      <span style={{
        color: "var(--text-secondary)", fontFamily: "var(--font-mono, monospace)",
        fontSize: 14, wordBreak: "break-all",
      }}>{v}</span>
    </div>
  );
}

export function DetailConfig({ platform: p, membership }: {
  platform: Platform;
  membership?: string[];
}) {
  const { t, i18n } = useTranslation();
  const { labelMap, peakWindows } = useProtocolMeta(p.platform_type, p.extra ?? "", i18n.language);
  const rl = parseRateLimitsBundle(p.extra ?? "");
  const rlText = rateLimitsSummary(rl, n => t("platform.rateLimits.windowsCount", "{{n}} 个窗口", { n }));
  const breaker = parsePlatformBreaker(p.extra ?? "");
  const breakerText = breaker.failure_threshold === 0 && breaker.half_open_max === 0
    ? t("platform.detail.inherit", "继承默认")
    : `${breaker.failure_threshold} / ${breaker.half_open_max}`;
  const peakMax = peakWindows.reduce((m, w) => Math.max(m, w.multiplier ?? 1), 1);
  const clients = parseAllowedClients(p.extra ?? "");
  const slotRows = MODEL_SLOTS
    .map(s => ({ label: t(s.labelKey), chain: slotList(p.models?.[s.key]) }))
    .filter(r => r.chain.length > 0);

  return (
    <Blk title={t("platform.detail.config", "配置")} testid="detail-config">
      <Row k={t("platform.endpoints", "协议端点")} v={(p.endpoints ?? []).length > 0
        ? p.endpoints!.map((ep, ei) => (
            <span key={ei}>
              {ei > 0 && " · "}
              {labelMap?.[ep.protocol] || PROTOCOL_LABELS[ep.protocol] || ep.protocol}
              {ep.coding_plan && <span style={{ color: "var(--color-success)", fontWeight: 700 }}> Code</span>}
              {" " + (ep.base_url || "")}
            </span>
          ))
        : p.base_url} />
      <Row k={t("platform.models", "模型配置")} v={slotRows.length > 0
        ? slotRows.map((r, ri) => (
            <div key={ri} style={{ display: "flex", gap: 6, alignItems: "baseline" }}>
              <span style={{ color: "var(--text-tertiary)", minWidth: 64 }}>{r.label}</span>
              <span>
                {r.chain.map((m, mi) => (
                  <span key={mi}>
                    {mi > 0 && <span style={{ color: "var(--text-tertiary)" }}>{" → "}</span>}
                    <span style={mi === 0
                      ? { color: "var(--accent)", fontWeight: 600 }
                      : { color: "var(--text-tertiary)" }}>
                      {m}
                    </span>
                  </span>
                ))}
              </span>
            </div>
          ))
        : "-"} />
      <Row k={t("platform.detail.rateLimits", "限频")} v={rlText ? rtlIsolate(rlText) : "-"} />
      <Row k={t("platform.detail.breaker", "熔断")} v={rtlIsolate(breakerText)} />
      <Row k={t("platform.detail.peak", "高峰倍率")} v={peakWindows.length > 0
        ? `${peakWindows.length} ×${peakMax}`
        : "-"} />
      <Row k={t("platform.detail.groups", "所属分组")} v={membership && membership.length > 0
        ? membership.join(" · ")
        : t("platform.detail.noGroup", "未分组")} />
      <Row k={t("platform.detail.whitelist", "客户端白名单")} v={clients.length > 0
        ? clients.join(" · ")
        : t("platform.detail.allClients", "全放行")} />
    </Blk>
  );
}
