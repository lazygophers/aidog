// ── 维度热力图（#39 批次二，spec §A1 热力×2 之二）：维度（行）× 时间（列）CSS grid，
// DOM+CSS 自研零 canvas，与 HourHeatmap 同族（时刻热力管「星期×小时」，这里管任意维度×时间）。
// 色带复用公共层 heatColor；行/列序由数据决定（行 = 首现序，列 = 时间升序）。
import { useMemo, type ReactNode } from "react";
import { ChartCard } from "./ChartCard";
import { heatColor } from "./palette";
import { pad } from "@/utils/formatters";

export interface DimensionHeatmapProps {
  /** 格子数据：name 维度值（行）、day 列时间（ms 时间戳）、value 任意非负量。缺格按 0。 */
  data: { name: string; day: number; value: number }[];
  /** hover 提示里的数值格式化，缺省原值。 */
  formatValue?: (n: number) => string;
  /** 列标签（day ms → 串），缺省本地时区 "MM-DD"。 */
  formatDay?: (ms: number) => string;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
}

const defaultFormatDay = (ms: number) => {
  const d = new Date(ms);
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
};

export function DimensionHeatmap({
  data,
  formatValue,
  formatDay = defaultFormatDay,
  title,
  subtitle,
  emptyHint,
  className,
}: DimensionHeatmapProps) {
  // 行 = 首现序（调用方已按总量降序排），列 = 时间升序
  const names = useMemo(
    () => [...new Set(data.map((d) => d.name))],
    [data],
  );
  const days = useMemo(
    () => [...new Set(data.map((d) => d.day))].sort((a, b) => a - b),
    [data],
  );
  const cells = useMemo(
    () => new Map(data.map((d) => [`${d.name}\x00${d.day}`, d.value])),
    [data],
  );
  const max = useMemo(() => Math.max(0, ...data.map((d) => d.value)), [data]);
  const fmt = formatValue ?? ((n: number) => String(n));

  // 列刻度稀疏标注：只标每 labelEvery 列，防 "MM-DD" 串挤叠
  const labelEvery = Math.max(1, Math.ceil(days.length / 10));

  return (
    <ChartCard title={title} subtitle={subtitle} empty={data.length === 0} emptyHint={emptyHint} className={className}>
      <div
        role="img"
        aria-label={typeof title === "string" ? title : undefined}
        style={{
          display: "grid",
          gridTemplateColumns: `minmax(6em, 12em) repeat(${days.length}, 1fr)`,
          gap: 2,
          fontSize: 14,
          color: "var(--text-tertiary)",
          fontVariantNumeric: "tabular-nums",
          overflowX: days.length > 31 ? "auto" : undefined,
        }}
      >
        {/* 列刻度行：首格空占位对齐行名 */}
        <div />
        {days.map((d, i) => (
          <div key={d} style={{ textAlign: "center", whiteSpace: "nowrap" }}>
            {i % labelEvery === 0 ? formatDay(d) : ""}
          </div>
        ))}
        {names.map((name) => (
          <HeatRow
            key={name}
            name={name}
            days={days}
            cells={cells}
            max={max}
            fmt={fmt}
            formatDay={formatDay}
          />
        ))}
      </div>
    </ChartCard>
  );
}

/** 单行 = 一个维度：值查不到按 0，t = value / max（max=0 全图回落色带最低档）。 */
function HeatRow({
  name,
  days,
  cells,
  max,
  fmt,
  formatDay,
}: {
  name: string;
  days: number[];
  cells: Map<string, number>;
  max: number;
  fmt: (n: number) => string;
  formatDay: (ms: number) => string;
}) {
  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={name}
      >
        {name}
      </div>
      {days.map((d) => {
        const v = cells.get(`${name}\x00${d}`) ?? 0;
        return (
          <div
            key={d}
            data-cell={`${name}|${d}`}
            title={`${name} ${formatDay(d)} · ${fmt(v)}`}
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
