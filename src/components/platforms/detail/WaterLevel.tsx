// T5 水位板块（详情页 F）：限频实时占用 + 额度窗口逐条水位（方向 5 密度终端 .qw 卡片形态）。
// 限频占用 = platform_rate_occupancy（SchedulerState.rate 内存计数快照，事件驱动：
// 后端占位计数变化发 rate-occupancy-updated，前端 1s debounce 重拉；隐藏期间跳过、恢复可见补拉。
// 代理从未启动过 → 空列表，不虚构 0 之外的值）。额度窗口 = platform_quota_window_levels（后端现算已用/剩余/重置时刻）。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PlatformQuotaWindowLevel, PlatformRateOccupancy } from "../../../services/api";
import { platformApi, parseRateLimitsBundle } from "../../../services/api";
import { listen } from "../../../services/transport";
import { formatNumber, formatMoney } from "../../../utils/formatters";
import { Blk } from "./Blk";

const MODE_KEY: Record<string, string> = {
  sliding: "platform.detail.modeSliding",
  calendar_day: "platform.detail.modeCalendarDay",
  calendar_hour: "platform.detail.modeCalendarHour",
};
const DIM_KEY: Record<string, string> = {
  tokens: "platform.detail.qwTokens",
  usd: "platform.detail.qwUsd",
  count: "platform.detail.qwCount",
};

export function amountText(w: PlatformQuotaWindowLevel): string {
  const fmt = (v: number) => (w.dimension === "usd" ? formatMoney(v) : formatNumber(v));
  return `${fmt(w.used)} / ${fmt(w.budget)}`;
}

// 已用比例条配色（设计 .fill norm/warn/hot）：<70 常态、70–89 逼近、≥90 热。
export function fillColor(ratio: number): string {
  const pct = ratio * 100;
  if (pct >= 90) return "var(--color-danger)";
  if (pct >= 70) return "var(--color-warning)";
  return "var(--data-primary)";
}

/** 限频占用行文本：并发/RPM/TPM 三计数并列（60 秒滑动窗口）。 */
export function occupancyText(o: PlatformRateOccupancy): string {
  return `${o.in_flight} / ${o.rpm_used} / ${formatNumber(o.tpm_used)}`;
}

export function WaterLevel({ platformId, extra }: { platformId: number; extra?: string }) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<PlatformQuotaWindowLevel[]>([]);
  const [occ, setOcc] = useState<PlatformRateOccupancy[]>([]);

  // 限频占用事件驱动（内存计数变化快，后端发 rate-occupancy-updated）；额度窗口一次性加载（变化慢，重置时刻由后端现算）。
  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let pendingWhileHidden = false;
    const loadOcc = () => platformApi.rateOccupancy(platformId)
      .then(r => { if (alive && Array.isArray(r)) setOcc(r); })
      .catch(() => {});
    // debounce 1s 合并突发事件；隐藏期间跳过拉取，恢复可见补拉一次
    const run = () => {
      if (document.hidden) { pendingWhileHidden = true; return; }
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        if (document.hidden) { pendingWhileHidden = true; return; }
        loadOcc();
      }, 1000);
    };
    let unlisten: () => void = () => {};
    listen("rate-occupancy-updated", () => { run(); }).then((fn) => {
      if (alive) unlisten = fn; else fn();
    });
    const onVisible = () => {
      if (!document.hidden && (pendingWhileHidden || timer)) {
        pendingWhileHidden = false;
        if (timer) { clearTimeout(timer); timer = null; }
        loadOcc();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    document.addEventListener("focus", onVisible);
    loadOcc();
    return () => {
      alive = false;
      unlisten();
      document.removeEventListener("visibilitychange", onVisible);
      document.removeEventListener("focus", onVisible);
      if (timer) clearTimeout(timer);
    };
  }, [platformId]);

  useEffect(() => {
    let alive = true;
    platformApi.quotaWindowLevels(platformId)
      .then(r => { if (alive && Array.isArray(r)) setRows(r); })
      .catch(() => {});
    return () => { alive = false; };
  }, [platformId]);

  const platformLimits = parseRateLimitsBundle(extra ?? "").rate_limits;
  const limitOf = (model: string) =>
    model === "" ? platformLimits : undefined;

  return (
    <Blk testid="detail-t5-water" title={t("platform.detail.water", "限频与额度窗口水位")}
      hint={t("platform.detail.live", "实时")}>
      {/* 限频占用：平台级桶 + 各模型桶（真实内存计数；无占用 / 代理未启动 → 不渲染行） */}
      {occ.map((o, i) => {
        const lim = limitOf(o.model);
        return (
          <div key={o.model || "__platform"} data-testid="detail-t5-occ-row"
            style={{ border: "1px solid var(--border)", borderRadius: 7, padding: "8px 10px", marginBottom: rows.length > 0 || i < occ.length - 1 ? 10 : 0 }}>
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: 14 }}>
              <span style={{ color: "var(--text-secondary)" }}>
                {o.model === "" ? t("platform.detail.rlPlatformLevel", "平台级") : o.model}
              </span>
              <span style={{ fontFamily: "var(--font-mono, monospace)" }}>{occupancyText(o)}</span>
            </div>
            {lim && (
              <div style={{ fontSize: 14, color: "var(--muted-foreground)", marginTop: 4 }}>
                {t("platform.detail.rlConfigured", "配置上限：并发 {{c}} · RPM {{r}} · TPM {{t}}")
                  .replace("{{c}}", String(lim.concurrency ?? "—"))
                  .replace("{{r}}", String(lim.rpm ?? "—"))
                  .replace("{{t}}", String(lim.tpm ?? "—"))}
              </div>
            )}
          </div>
        );
      })}
      {rows.length === 0 ? (
        <div data-testid="detail-t5-qw-empty" style={{ fontSize: 14, color: "var(--muted-foreground)", padding: "8px 0" }}>
          {t("platform.detail.noData", "暂无数据")}
        </div>
      ) : rows.map((w, i) => {
        const usedPct = w.budget > 0 ? ((w.used / w.budget) * 100).toFixed(1) : "0";
        const remainPct = (w.remaining_ratio * 100).toFixed(1).replace(/\.0$/, "");
        return (
          <div key={w.key} data-testid="detail-t5-qw-row"
            style={{ border: "1px solid var(--border)", borderRadius: 7, padding: "8px 10px", marginTop: i === 0 ? 0 : 10 }}>
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: 14, marginBottom: 6 }}>
              <span style={{ color: "var(--text-secondary)" }}>
                {t(DIM_KEY[w.dimension] ?? w.dimension, w.dimension)}
                <span style={{ color: "var(--muted-foreground)" }}>
                  {" "}（{t(MODE_KEY[w.reset_mode] ?? w.reset_mode, w.reset_mode)}）
                </span>
              </span>
              <span style={{ fontFamily: "var(--font-mono, monospace)" }}>
                {amountText(w)} · {t("platform.detail.qwRemain", "剩 {{pct}}%").replace("{{pct}}", `${remainPct}`)}
              </span>
            </div>
            <div style={{ height: 6, borderRadius: 3, background: "var(--muted)", overflow: "hidden" }}>
              <div data-testid={`detail-t5-qw-fill-${i}`} title={new Date(w.resets_at_ms).toLocaleString()}
                style={{ width: `${usedPct}%`, height: "100%", borderRadius: 3, background: fillColor(w.used / (w.budget || 1)) }} />
            </div>
            <div style={{ fontSize: 14, color: "var(--muted-foreground)", marginTop: 4 }}>
              {t("platform.detail.qwResetIn", "{{time}} 后重置").replace("{{time}}", new Date(w.resets_at_ms).toLocaleTimeString())}
            </div>
          </div>
        );
      })}
    </Blk>
  );
}
