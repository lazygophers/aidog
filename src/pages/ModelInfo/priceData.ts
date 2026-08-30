// registry 模型条目 `price_data` 解析层。
//
// `ModelEntry` 只把查询/排序用得到的字段提成列，价格维度整份留在 `price_data`（registry 模型 JSON 原文）。
// 展示层需要默认价 / 高峰绝对价 / 上下文阶梯价，故在此统一解析一次，禁各组件各写 JSON.parse。

import { formatCostUsd, formatNumber } from "../../utils/formatters";

/** 一档价格（单位 $/token，registry 原始单位，字段名 = registry `price` 子树简名）。 */
export interface PriceTier {
  input?: number | null;
  output?: number | null;
  cache_read?: number | null;
  cache_write?: number | null;
}

/** registry `price` 子树形状（2026-08-30 起价格收归于条目 `price` 字段；
 *  Rust ui_entry 读取出口已把未重同步的旧顶层形状归一化，这里只认新形状）。 */
export interface ModelPriceData extends PriceTier {
  /** 高峰绝对价：命中平台 `peak` 窗口时整体替换默认价。 */
  peak?: PriceTier | null;
  /** 上下文阶梯价：按请求 input_tokens 选档，`min_tokens` 为起档阈值。 */
  context_tiers?: Array<PriceTier & { min_tokens?: number | null }> | null;
}

/** 解析 `price_data` 取 `price` 子树；空串 / 非法 JSON / 缺 price 一律返回空对象
 *  （展示层按「无价格」渲染）。 */
export function parsePriceData(raw: string): ModelPriceData {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const price = (parsed as { price?: unknown }).price;
      if (price && typeof price === "object" && !Array.isArray(price)) {
        return price as ModelPriceData;
      }
    }
  } catch {
    // registry 数据损坏不该炸整页；缺价格即显示 "-"
  }
  return {};
}

/** $/token → $/M tokens；非有限数返回 null。 */
export function perMillion(v?: number | null): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v * 1_000_000 : null;
}

/** $/token → `$x.xx` 每百万 token 展示串；缺值 → "-"。 */
export function fmtPricePerM(v?: number | null): string {
  const m = perMillion(v);
  return m == null ? "-" : formatCostUsd(m);
}

/** token 数展示（131072 → "131.1K"）；缺值 → "-"。 */
export function fmtTokens(v?: number | null): string {
  return typeof v === "number" && Number.isFinite(v) ? formatNumber(v) : "-";
}
