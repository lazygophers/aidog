import { pinyin as pinyinFn } from "pinyin-pro";

/** 拼音转换 LRU 缓存：搜索高频复用同一 target，避免每 keystroke 重算 */
const PINYIN_CACHE_MAX = 500;
const pinyinCache = new Map<string, string>();

function cachedToPinyin(text: string): string {
  const hit = pinyinCache.get(text);
  if (hit !== undefined) {
    // LRU: 命中后移到末尾（最近使用）
    pinyinCache.delete(text);
    pinyinCache.set(text, hit);
    return hit;
  }
  const value = computePinyin(text);
  pinyinCache.set(text, value);
  if (pinyinCache.size > PINYIN_CACHE_MAX) {
    // 淘汰最旧（Map 迭代顺序 = 插入顺序）
    const oldest = pinyinCache.keys().next().value;
    if (oldest !== undefined) pinyinCache.delete(oldest);
  }
  return value;
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

function toPinyin(text: string): string {
  return cachedToPinyin(text);
}

/**
 * 拼音模糊匹配：支持纯拼音 / 纯中文 / 中英混合搜索
 *
 * 例: target="百炼"
 *   "bailian" ✓  "bai" ✓  "百" ✓  "百lian" ✓  "炼" ✓
 *
 * 例: target="小米"
 *   "xiaomi" ✓  "xiao米" ✓  "ao米" ✓  "xi" ✓  "小m" ✓
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

  return false;
}
