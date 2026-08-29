import { pinyin as pinyinFn } from "pinyin-pro";

/** 拼音转换 LRU 缓存：搜索高频复用同一 target，避免每 keystroke 重算 */
const PINYIN_CACHE_MAX = 500;

/** 通用 LRU 缓存包装（全拼 / 首字母两份 cache 共用同一淘汰逻辑） */
function cachedConvert(cache: Map<string, string>, compute: (text: string) => string, text: string): string {
  const hit = cache.get(text);
  if (hit !== undefined) {
    // LRU: 命中后移到末尾（最近使用）
    cache.delete(text);
    cache.set(text, hit);
    return hit;
  }
  const value = compute(text);
  cache.set(text, value);
  if (cache.size > PINYIN_CACHE_MAX) {
    // 淘汰最旧（Map 迭代顺序 = 插入顺序）
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  return value;
}

const pinyinCache = new Map<string, string>();
const initialsCache = new Map<string, string>();

function toPinyin(text: string): string {
  return cachedConvert(pinyinCache, computePinyin, text);
}

/** 中文字符首字母串（搜索首字母用）
 *  例: "百炼" → "bl", "月之暗面" → "yzam" */
function toInitials(text: string): string {
  return cachedConvert(initialsCache, computeInitials, text);
}

/**
 * 将字符串中的中文字符转为拼音（无声调），非中文字符保留
 * 例: "百炼" → "bailian", "GLM" → "GLM", "小米AI" → "xiaomiAI"
 */
function computePinyin(text: string): string {
  let result = "";
  for (const ch of text) {
    if (/[一-鿿]/.test(ch)) {
      result += pinyinFn(ch, { toneType: "none" });
    } else {
      result += ch;
    }
  }
  return result.toLowerCase();
}

function computeInitials(text: string): string {
  let result = "";
  for (const ch of text) {
    if (/[一-鿿]/.test(ch)) {
      result += pinyinFn(ch, { pattern: "first", toneType: "none" });
    }
  }
  return result.toLowerCase();
}

/**
 * 拼音模糊匹配：支持纯拼音 / 拼音首字母 / 纯中文 / 中英混合搜索
 *
 * 例: target="百炼"
 *   "bailian" ✓  "bai" ✓  "bl" ✓（首字母）  "百" ✓  "百lian" ✓  "炼" ✓
 *
 * 例: target="小米"
 *   "xiaomi" ✓  "xiao米" ✓  "xm" ✓  "xi" ✓  "小m" ✓
 *
 * 例: target="GLM"
 *   "gl" ✓  "glm" ✓
 */
export function pinyinMatch(query: string, target: string): boolean {
  const q = query.toLowerCase().trim();
  if (!q) return true; // 空查询显示全部

  const t = target.toLowerCase();

  // 1. 直接子串匹配
  if (t.includes(q)) return true;

  // 2. 目标转拼音后匹配
  const targetPinyin = toPinyin(target);
  if (targetPinyin.includes(q)) return true;

  // 3. 查询中的中文字符也转拼音，再匹配
  const queryPinyin = toPinyin(query);
  if (targetPinyin.includes(queryPinyin)) return true;
  if (t.includes(queryPinyin)) return true;

  // 4. 拼音首字母匹配（query 为拉丁字母时，如 "bl" → "百炼"、"yzam" → "月之暗面"）
  if (!/[一-鿿]/.test(q)) {
    if (toInitials(target).includes(q)) return true;
  }

  return false;
}
