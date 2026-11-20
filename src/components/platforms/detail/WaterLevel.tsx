// T5 水位板块（详情页 F）：额度窗口逐条水位（方向 5 密度终端 .qw 卡片形态）。
// 数据 = platform_quota_window_levels（enabled 窗口，后端现算已用/剩余/重置时刻）。
// 限频占位（并发/RPM/TPM）为 ProxyState 内存计数，命令侧不可达，本板块只渲染额度窗口。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PlatformQuotaWindowLevel } from "../../../services/api";
import { platformApi } from "../../../services/api";
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

export function WaterLevel({ platformId }: { platformId: number }) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<PlatformQuotaWindowLevel[]>([]);

  useEffect(() => {
    let alive = true;
    platformApi.quotaWindowLevels(platformId)
      .then(r => { if (alive && Array.isArray(r)) setRows(r); })
      .catch(() => {});
    return () => { alive = false; };
  }, [platformId]);

  return (
    <Blk testid="detail-t5-water" title={t("platform.detail.water", "限频与额度窗口水位")}
      hint={t("platform.detail.live", "实时")}>
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
