// 模型 select 行尾价格标签（grill-select-price spec，2026-10-08）：
// 置灰小字，不抢模型名主信息。数据形状见 domains/platforms/modelPrices.ts。

import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import {
  compactPriceLabel,
  fullPriceLines,
  type ModelPriceInfo,
} from "../../domains/platforms/modelPrices";

const BASE: CSSProperties = {
  color: "var(--text-tertiary)",
  fontSize: 11,
  lineHeight: 1.35,
  whiteSpace: "nowrap",
  flexShrink: 0,
};

/**
 * `info` 为 null/undefined（无价格条目）时整块不渲染。
 * - `compact`（默认）：单行 `in $3 · out $15`，用于选中框内 / 窄位。
 * - 完整态：两行四价 + peak 追加，用于下拉行尾。
 */
export function ModelPriceTag({ info, compact, style }: {
  info: ModelPriceInfo | null | undefined;
  compact?: boolean;
  style?: CSSProperties;
}) {
  const { t } = useTranslation();
  if (!info) return null;
  if (info.free) {
    return <span style={{ ...BASE, ...style }}>{t("common.priceFree", "免费")}</span>;
  }
  if (info.unitLabel) {
    return <span style={{ ...BASE, ...style }}>{info.unitLabel}</span>;
  }
  if (compact) {
    const s = compactPriceLabel(info);
    return s ? <span style={{ ...BASE, ...style }}>{s}</span> : null;
  }
  const lines = fullPriceLines(info, t("common.pricePeak", "峰"));
  if (!lines.length) return null;
  return (
    <span style={{ ...BASE, ...style, display: "inline-flex", flexDirection: "column", alignItems: "flex-end" }}>
      {lines.map((l, i) => <span key={i}>{l}</span>)}
    </span>
  );
}
