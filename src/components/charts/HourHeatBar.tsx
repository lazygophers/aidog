// ── 迷你热力条（spec §C2 浮窗三件套）：今日 0-23 时单行紧凑横条 ──
// 复用公共层 heatColor 琥珀 alpha 阶梯（与 HourHeatmap 同色带）；不派生 HourHeatmap：
// 浮窗今日窗只有 1×24 数据，画 7×24 周格会垫 6 行零值假数据，紧凑横条更诚实。
import { useMemo } from "react";
import { pad } from "@/utils/formatters";
import { heatColor } from "./palette";

export interface HourHeatBarProps {
  /** 今日各小时值：hour 0-23，缺小时按 0（无请求）。 */
  data: { hour: number; value: number }[];
  /** title 提示里的数值格式化（走 utils/formatters），缺省原值。 */
  formatValue?: (n: number) => string;
  /** 无障碍标签（role=img 的名字）。 */
  ariaLabel?: string;
}

/** 今日 24 小时热力横条：DOM+CSS 零 canvas，格色 = heatColor(值/最大值)。 */
export function HourHeatBar({ data, formatValue, ariaLabel }: HourHeatBarProps) {
  const values = useMemo(() => {
    const m = new Array<number>(24).fill(0);
    for (const d of data) {
      if (d.hour >= 0 && d.hour < 24) m[d.hour] = d.value;
    }
    return m;
  }, [data]);
  const max = useMemo(() => Math.max(0, ...values), [values]);
  const fmt = formatValue ?? ((n: number) => String(n));

  return (
    <div
      role="img"
      aria-label={ariaLabel}
      style={{ display: "grid", gridTemplateColumns: "repeat(24, 1fr)", gap: 2, fontSize: 9, color: "var(--text-tertiary)" }}
    >
      {values.map((v, h) => (
        <div
          key={h}
          data-heat-hour={h}
          title={`${pad(h)}:00 · ${fmt(v)}`}
          style={{
            height: 12,
            borderRadius: 2,
            background: heatColor(max > 0 ? v / max : 0),
          }}
        />
      ))}
    </div>
  );
}
