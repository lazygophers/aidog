// 模型 select 价格展示的数据层（2026-10-08 grill-select-price spec）。
// 真值源 model_entry.price_data（registry 模型 JSON 原文），解析复用 ModelInfo 的
// priceData.ts；本层只做「条目 → 下拉可显示的形状」归一 + RPC 缓存，展示组件 ModelPriceTag。

import { useEffect, useState } from "react";
import { modelInfoApi, onRegistryUpdated } from "../../services/api/pricing";
import {
  parsePriceData,
  fmtPricePerM,
  fmtPricePerUnit,
  type ModelPriceData,
} from "../../pages/ModelInfo/priceData";

/** 下拉展示用的价格形状（registry `price` 子树归一；undefined 字段 = registry 未标，展示层省略）。 */
export interface ModelPriceInfo {
  /** registry `price.free`：官方显式免费（恒 0 计费，非全 0 未定价）。 */
  free: boolean;
  input?: number;
  output?: number;
  cacheRead?: number;
  cacheWrite?: number;
  /** 高峰绝对价（命中平台 peak 窗口时整体替换默认价）。 */
  peakInput?: number;
  peakOutput?: number;
  /** 非 token 计价条目（$/张、$/秒等）的现成展示串；有值时优先于 token 四价。 */
  unitLabel?: string;
}

/** `price` 子树 → 展示形状；无任何可展示价格返回 null（调用方留空渲染）。 */
export function toModelPriceInfo(p: ModelPriceData): ModelPriceInfo | null {
  if (p.unit && p.unit !== "token") {
    return p.unit_price != null
      ? { free: false, unitLabel: fmtPricePerUnit(p.unit_price, p.unit) }
      : null;
  }
  const info: ModelPriceInfo = {
    free: p.free === true,
    input: p.input ?? undefined,
    output: p.output ?? undefined,
    cacheRead: p.cache_read ?? undefined,
    cacheWrite: p.cache_write ?? undefined,
    peakInput: p.peak?.input ?? undefined,
    peakOutput: p.peak?.output ?? undefined,
  };
  const hasAny =
    info.free ||
    info.input != null ||
    info.output != null ||
    info.cacheRead != null ||
    info.cacheWrite != null ||
    info.peakInput != null ||
    info.peakOutput != null;
  return hasAny ? info : null;
}

/** `price_data` 原文 → 展示形状；空串 / 非法 JSON / 无 price 一律 null。 */
export function parseModelPrice(raw: string): ModelPriceInfo | null {
  return toModelPriceInfo(parsePriceData(raw));
}

const fmt = (v?: number) => (v == null ? null : fmtPricePerM(v));

const join = (parts: Array<string | null>) =>
  parts.filter((p): p is string => p != null).join(" · ");

/** 紧凑单行标签（选中框内 / 窄位）：`in $3 · out $15`；无可显示价返回 null。 */
export function compactPriceLabel(info: ModelPriceInfo): string | null {
  const s = join([fmt(info.input) && `in ${fmt(info.input)}`, fmt(info.output) && `out ${fmt(info.output)}`]);
  return s || null;
}

/** 完整两行标签（下拉行尾）：
 *  L1 `in $3 · out $15 · 峰 $6/$30`（peak 仅在条目带时追加）
 *  L2 `cr $0.3 · cw $3.75 /M`（无缓存价时单行，/M 标在最后一行行尾一次）。
 *  返回 [] 视为无可显示价。 */
export function fullPriceLines(info: ModelPriceInfo, peakWord: string): string[] {
  const peakIn = fmt(info.peakInput);
  const peakOut = fmt(info.peakOutput);
  const peak =
    peakIn || peakOut
      ? `${peakWord} ${[peakIn, peakOut].filter(Boolean).join("/")}`
      : null;
  const l1 = join([
    fmt(info.input) && `in ${fmt(info.input)}`,
    fmt(info.output) && `out ${fmt(info.output)}`,
    peak,
  ]);
  const l2 = join([
    fmt(info.cacheRead) && `cr ${fmt(info.cacheRead)}`,
    fmt(info.cacheWrite) && `cw ${fmt(info.cacheWrite)}`,
  ]);
  const lines = l2 ? [l1, l2].filter(Boolean) : l1 ? [l1] : [];
  if (lines.length) lines[lines.length - 1] += " /M";
  return lines;
}

// ─── RPC 缓存与 hooks ───────────────────────────────────────
// 同 defaults.ts docPromise idiom：模块级单次 promise 缓存，registry-updated 时整表失效。

type PriceMap = ReadonlyMap<string, ModelPriceInfo>;
const cache = new Map<string, Promise<unknown>>();
const EMPTY_MAP: PriceMap = new Map();
const EMPTY_ALL_MAP: ReadonlyMap<string, PriceMap> = new Map();

function fetchPlatform(platformCode: string): Promise<PriceMap> {
  const key = `list:${platformCode}`;
  let p = cache.get(key) as Promise<PriceMap> | undefined;
  if (!p) {
    p = modelInfoApi.list(platformCode).then((entries) => {
      const m = new Map<string, ModelPriceInfo>();
      for (const e of entries) {
        const info = parseModelPrice(e.price_data);
        if (info) m.set(e.model_id, info);
      }
      return m;
    });
    // 失败不缓存，下次挂载重试
    p.catch(() => cache.delete(key));
    cache.set(key, p);
  }
  return p;
}

function fetchAll(): Promise<ReadonlyMap<string, PriceMap>> {
  const key = "snapshot";
  let p = cache.get(key) as Promise<ReadonlyMap<string, PriceMap>> | undefined;
  if (!p) {
    p = modelInfoApi.snapshot().then((snap) => {
      const byPlatform = new Map<string, Map<string, ModelPriceInfo>>();
      for (const g of snap.groups) {
        for (const e of g.entries) {
          const info = parseModelPrice(e.price_data);
          if (!info) continue;
          let m = byPlatform.get(e.platform_code);
          if (!m) byPlatform.set(e.platform_code, (m = new Map()));
          m.set(e.model_id, info);
        }
      }
      return byPlatform;
    });
    p.catch(() => cache.delete(key));
    cache.set(key, p);
  }
  return p;
}

/** 单平台价格表：model_id → 价格。用于平台编辑页（矩阵 + 时段档共用一个下拉）。 */
export function useModelPrices(platformCode?: string | null): PriceMap {
  const [map, setMap] = useState<PriceMap>(EMPTY_MAP);
  useEffect(() => {
    if (!platformCode) return;
    let alive = true;
    const run = () => {
      fetchPlatform(platformCode).then(
        (m) => { if (alive) setMap(m); },
        () => { /* 失败静默：价格展示缺席，不阻塞编辑页 */ },
      );
    };
    run();
    const off = onRegistryUpdated(() => {
      cache.clear();
      run();
    });
    return () => { alive = false; off(); };
  }, [platformCode]);
  return map;
}

/** 全平台价格表：platform_code → (model_id → 价格)。用于分组映射（逐行目标平台不同）。 */
export function useAllModelPrices(): ReadonlyMap<string, PriceMap> {
  const [map, setMap] = useState<ReadonlyMap<string, PriceMap>>(EMPTY_ALL_MAP);
  useEffect(() => {
    let alive = true;
    const run = () => {
      fetchAll().then(
        (m) => { if (alive) setMap(m); },
        () => { /* 失败静默 */ },
      );
    };
    run();
    const off = onRegistryUpdated(() => {
      cache.clear();
      run();
    });
    return () => { alive = false; off(); };
  }, []);
  return map;
}
