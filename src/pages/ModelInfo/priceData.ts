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
 *  Rust ui_entry 读取出口已把未重同步的旧顶层形状归一化，这里只认新形状）。
 *  非 chat 模态（图像/视频/搜索工具类）用 `unit` + `unit_price` 计价（$/张、$/秒、$/次），
 *  token 单价字段不适用；chat 计费链不消费这类条目（无 token 价自动 fallback）。 */
export interface ModelPriceData extends PriceTier {
  /** 计价单位，缺省 token。 */
  unit?: "token" | "image" | "second" | "request" | null;
  /** 非 token 条目的单价（$/unit）。 */
  unit_price?: number | null;
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

/** registry 模型条目顶层的非价格标记（`price_data` 整份原文里的可选 bool）。
 *  缺省 = 条目未标注，展示层渲染 "-"（不与 false 混同）。 */
export interface EntryFlags {
  /** 是否支持 thinking/推理模式。 */
  thinking_supported?: boolean | null;
  /** thinking 是否可由请求参数开关（false = 强制思考）。 */
  thinking_toggleable?: boolean | null;
}

/** 解析 `price_data` 取顶层标记字段；空串 / 非法 JSON 返回空对象。 */
export function parseEntryFlags(raw: string): EntryFlags {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const { thinking_supported, thinking_toggleable } = parsed as EntryFlags;
      return { thinking_supported, thinking_toggleable };
    }
  } catch {
    // 同 parsePriceData：数据损坏不炸页，未标注即 "-"
  }
  return {};
}

/** $/unit（张/秒/次）展示串（`$x.xx /张` 等）；缺值 → "-"。 */
export function fmtPricePerUnit(v?: number | null, unit?: string | null): string {
  return typeof v === "number" && Number.isFinite(v) ? formatCostUsd(v) + ` /${unit ?? "unit"}` : "-";
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
