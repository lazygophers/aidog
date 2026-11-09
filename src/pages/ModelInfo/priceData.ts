// registry 模型条目 `price_data` 解析层。
//
// `ModelEntry` 只把查询/排序用得到的字段提成列，价格维度整份留在 `price_data`（registry 模型 JSON 原文）。
// 展示层需要默认价 / 高峰绝对价 / 上下文阶梯价，故在此统一解析一次，禁各组件各写 JSON.parse。

import { formatMoney, formatNumber } from "../../utils/formatters";

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
  /** 计价币种，缺省 USD；CNY 条目 Rust 计费链按汇率换算 USD。 */
  currency?: string | null;
  /** 计价单位，缺省 token。 */
  unit?: "token" | "image" | "second" | "request" | "char" | "mp" | null;
  /** 非 token 条目的单价（$/unit）。 */
  unit_price?: number | null;
  /** 显式免费（官方来源）：计费价恒为 0。全 0 价未标 free 视同未定价。 */
  free?: boolean | null;
  /** 高峰绝对价：命中平台 `peak` 窗口时整体替换默认价。 */
  peak?: PriceTier | null;
  /** 上下文阶梯价：按请求 input_tokens 选档，`min_tokens` 为起档阈值。 */
  context_tiers?: Array<PriceTier & { min_tokens?: number | null }> | null;
  /** 按次计价分档（`tier` 标签 + 单价，顺序=官方档位顺序）；展示取第一档为现价。
   *  与 Rust `price_resolve` 同口径：计费链拿不到尺寸/分辨率参数，第一档=缺参兜底档。 */
  unit_tiers?: Array<{ tier: string; unit_price: number }> | null;
  /** time_tiers 档（展示层只消费按次条目的 `unit_price` 切换）。 */
  time_tiers?: Array<{ start_at?: number | null; unit_price?: number | null }> | null;
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

/** 与 Rust `price_resolve::CNY_PER_USD` 同值：CNY 计价条目（`price.currency == "CNY"`）先 ÷ 此值成 USD，
 *  再交 formatMoney 按显示币种换算（formatMoney 入参恒为 USD，禁对 CNY 原价直接乘显示汇率）。
 *  不取用户可改的显示汇率：条目原价→USD 是计费口径，须与后端一致。 */
export const CNY_PER_USD = 6.6675;

/** 条目原币种价 → USD；非 CNY（含缺省）原样。 */
export function toUsd(v: number, currency?: string | null): number {
  return currency === "CNY" ? v / CNY_PER_USD : v;
}

/** $/unit（张/秒/次）展示串（`$x.xx /张` 等）；缺值 → "-"。`currency` 为条目计价币种。 */
export function fmtPricePerUnit(v?: number | null, unit?: string | null, currency?: string | null): string {
  if (typeof v !== "number" || !Number.isFinite(v)) return "-";
  const m = formatMoney(toUsd(v, currency));
  // char 单位存 $/1K chars；mp 展示为 $/MP；其余 $/unit
  if (unit === "char") return m + " /1K chars";
  if (unit === "mp") return m + " /MP";
  return m + ` /${unit ?? "unit"}`;
}

/** 按次条目现价（$/unit，条目原币种）：time 档 `unit_price`（start_at <= nowMs，latest wins）
 *  > `unit_tiers` 第一档 > 顶层 `unit_price`；无任何值返回 null。与 Rust `price_resolve`
 *  按次分支同口径（`nowMs` 缺省 = 当前时间）。 */
export function effectiveUnitPrice(p: ModelPriceData, nowMs = Date.now()): number | null {
  let up = typeof p.unit_price === "number" ? p.unit_price : null;
  if (Array.isArray(p.unit_tiers) && p.unit_tiers.length > 0) {
    const first = p.unit_tiers[0]?.unit_price;
    if (typeof first === "number") up = first;
  }
  if (Array.isArray(p.time_tiers) && nowMs > 0) {
    let best: { at: number; up: number | null } | null = null;
    for (const t of p.time_tiers) {
      const at = typeof t.start_at === "number" ? t.start_at : null;
      if (at != null && at * 1000 <= nowMs && (best == null || at >= best.at)) {
        best = { at, up: typeof t.unit_price === "number" ? t.unit_price : null };
      }
    }
    if (best?.up != null) up = best.up;
  }
  return up;
}

/** $/token → $/M tokens；非有限数返回 null。 */
export function perMillion(v?: number | null): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v * 1_000_000 : null;
}

/** $/token → `$x.xx` 每百万 token 展示串；缺值 → "-"。 */
export function fmtPricePerM(v?: number | null, currency?: string | null): string {
  const m = perMillion(v);
  return m == null ? "-" : formatMoney(toUsd(m, currency));
}

/** token 数展示（131072 → "131.1K"）；缺值 → "-"。 */
export function fmtTokens(v?: number | null): string {
  return typeof v === "number" && Number.isFinite(v) ? formatNumber(v) : "-";
}
