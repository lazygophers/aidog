// ── 时刻热力图（#35 批次一）：24 小时 × 7 天 CSS grid，DOM+CSS 自研零 canvas（spec §A1）。
// 格子色带走公共层 heatColor（琥珀 alpha 阶梯）；day 用 JS getDay 约定（0=周日），行序渲染周一→周日。
import { useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { pad } from "@/utils/formatters";
import { ChartCard } from "./ChartCard";
import { heatColor } from "./palette";

/** 行序：周一…周日（输入 day 仍按 getDay 0=周日约定）。 */
const DAY_ROWS = [1, 2, 3, 4, 5, 6, 0];
/** 顶部小时刻度只标 0/6/12/18，其余留空防挤。 */
const HOUR_LABEL_AT = new Set([0, 6, 12, 18]);

export interface HourHeatmapProps {
  /** 格子数据：day 0-6（JS getDay，0=周日）、hour 0-23、value 任意非负量。缺格按 0。 */
  data: { day: number; hour: number; value: number }[];
  /** hover 提示里的数值格式化，缺省原值。 */
  formatValue?: (n: number) => string;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
}

export function HourHeatmap({
  data,
  formatValue,
  title,
  subtitle,
  emptyHint,
  className,
}: HourHeatmapProps) {
  const { i18n } = useTranslation();

  const cells = useMemo(
    () => new Map(data.map((d) => [d.day * 24 + d.hour, d.value])),
    [data],
  );
  const max = useMemo(() => Math.max(0, ...data.map((d) => d.value)), [data]);
  const dayLabel = useMemo(() => {
    // 2024-01-07 是周日：7 + d 恰好落在对应星期上，用真实日期过 Intl 拿本地化周名
    const fmt = new Intl.DateTimeFormat(i18n.language, { weekday: "short" });
    return (d: number) => fmt.format(new Date(2024, 0, 7 + d));
  }, [i18n.language]);
  const fmt = formatValue ?? ((n: number) => String(n));

  return (
    <ChartCard title={title} subtitle={subtitle} empty={data.length === 0} emptyHint={emptyHint} className={className}>
      <div
        role="img"
        aria-label={typeof title === "string" ? title : undefined}
        style={{
          display: "grid",
          gridTemplateColumns: "3.5em repeat(24, 1fr)",
          gap: 2,
          fontSize: 9,
          color: "var(--text-tertiary)",
          fontVariantNumeric: "tabular-nums",
        }}
      >
        {/* 小时刻度行：首格空占位对齐日名列 */}
        <div />
        {Array.from({ length: 24 }, (_, h) => (
          <div key={`h${h}`} style={{ textAlign: "center" }}>
            {HOUR_LABEL_AT.has(h) ? pad(h) : ""}
          </div>
        ))}
        {DAY_ROWS.map((d) => (
          <HeatRow key={`d${d}`} day={d} label={dayLabel(d)} cells={cells} max={max} fmt={fmt} />
        ))}
      </div>
    </ChartCard>
  );
}

/** 单行 24 格：值查不到按 0，t = value / max（max=0 全图回落色带最低档）。 */
function HeatRow({
  day,
  label,
  cells,
  max,
  fmt,
}: {
  day: number;
  label: string;
  cells: Map<number, number>;
  max: number;
  fmt: (n: number) => string;
}) {
  return (
    <>
      <div style={{ display: "flex", alignItems: "center" }}>{label}</div>
      {Array.from({ length: 24 }, (_, h) => {
        const v = cells.get(day * 24 + h) ?? 0;
        return (
          <div
            key={h}
            data-cell={`${day}-${h}`}
            title={`${label} ${pad(h)}:00 · ${fmt(v)}`}
            style={{
              aspectRatio: "1",
              borderRadius: 2,
              background: heatColor(max > 0 ? v / max : 0),
            }}
          />
        );
      })}
    </>
  );
}
