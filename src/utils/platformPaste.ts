// 平台添加「智能识别」解析器（纯函数，无副作用，便于推理 / 手测）。
// 从论坛分享类杂乱文案中抽取 3 类字段：apikey / base_url / 平台（匹配内置 preset）。
// 设计覆盖样例：小米 MIMO（双 base_url）、防爬汉字 key、kimicode（多 key + url:）、base64 编码 key。

/** base_url 的协议倾向（仅用于展示分组 / 排序，非平台类型）。 */
export type ParsedProtocol = "anthropic" | "openai" | "gemini" | "unknown";

export interface ParsedBaseUrl {
  url: string;
  protocol: ParsedProtocol;
}

/** Platforms.tsx 的 preset 引用（解析器只需 value/label/keywords/hosts/codingPlan 字段）。 */
export interface PastePresetRef {
  value: string;
  label: string;
  keywords?: string[];
  /** base_url hostname 命中这些子串时优先匹配（比 keyword 文本扫更准）。
   *  存注册域（如 "xiaomimimo.com"）或完整 hostname，多 preset 重叠时最长 host 胜出。 */
  hosts?: string[];
  /** coding plan 变体标记：透传到 applyPaste → handleProtocolChange(value, codingPlan)，
   *  否则同 value 的普通/coding 两 preset 命中后 endpoints 取错（拿普通 base_url）。 */
  codingPlan?: boolean;
}

export interface ParsedPaste {
  /** 去重后的候选 apikey（已剔除混入 CJK、已尝试 base64 解码）。 */
  apiKeys: string[];
  /** 去重后的候选 base_url（含协议倾向）。 */
  baseUrls: ParsedBaseUrl[];
  /** 匹配到的内置平台 preset；无匹配为 null（调用方据此决定是否改平台选择）。
   *  codingPlan 标记透传给 applyPaste 选对普通/coding 变体 endpoints。 */
  platform: { value: string; label: string; codingPlan?: boolean } | null;
}

/** 已知 apikey 前缀（长在前，避免 sk- 抢先吃掉 sk-ant-）。 */
const KEY_PREFIXES = ["sk-ant-", "sk-kimi-", "sk-or-", "sk-proj-", "sk-", "tp-"];

/** CJK 及全角标点区段（用于剔除 key 中混入的防爬汉字）。
 *  \p{Script=Han} 覆盖全部汉字变体（基本区 + 扩展 A-F + 兼容汉字），比手写区段全；
 *  另含平假名/片假名 + CJK 标点 + 全角区段。需 u flag。 */
const CJK_RE = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯]/gu;

/** 前缀锚定 token：前缀 + 后续 alnum/_/-，允许中间穿插 CJK（防爬），后面整体 stripCjk 剔除。
 *  字符类含 \p{Script=Han} 防 CJK 扩展区汉字（如 𠀀）截断匹配。 */
const PREFIX_TOKEN_RE =
  /(sk-ant-|sk-kimi-|sk-or-|sk-proj-|sk-|tp-)[A-Za-z0-9_\-\.\p{Script=Han}　-〿＀-￯]{12,}/gu;

/** 赋值锚定：API_KEY= / apikey: / 秘药： / key= 等后跟值。 */
const ASSIGN_RE =
  /["']?(?:api[\s_-]*key|secret|token|秘药|密钥|key|auth[\s_-]*token|(?:[\w-]+)[\s_-]*(?:auth[\s_-]*token|api[\s_-]*key)|api)["']?\s*[:：=]\s*["'\u2018\u2019《「]?\s*([A-Za-z0-9_\-+/=.\p{Script=Han}　-〿＀-￯]{12,})/giu;

/** 纯 base64 token 形态（无已知前缀时用于 base64 解码启发式）。 */
const BASE64_RE = /^[A-Za-z0-9+/]{20,}={0,2}$/;

/** 裸 base64 token（无标签兜底扫描；非首尾锚定，扫整段 alnum+/=）。 */
const BARE_BASE64_RE = /[A-Za-z0-9+/]{24,}={0,2}/g;

/** base64 旁注标记（如「KEY（base64编码）：」中的「（base64编码）」）。
 *  夹在 key 标签与分隔符之间阻断 ASSIGN_RE 匹配，regex 前先剔除。
 *  支持全角（）/ 半角 ()，后缀「编码」可选。 */
const BASE64_NOTE_RE = /[（(]\s*base64[^）)]*[）)]/giu;

/** 解码后键形（短前缀 + 长串），裸 base64 兜底的误报守卫。
 *  命中「tt-xxx」「sk-xxx」等，排除解码噪声 / URL 片段。 */
const DECODED_KEY_SHAPE = /^[a-z]{2,8}-[A-Za-z0-9_\-]{20,}$/;

function stripCjk(s: string): string {
  return s.replace(CJK_RE, "");
}

function hasKnownPrefix(s: string): boolean {
  return KEY_PREFIXES.some((p) => s.startsWith(p));
}

/** base64 解码（浏览器 atob，非法输入返回 null）。 */
function tryBase64Decode(s: string): string | null {
  if (!BASE64_RE.test(s)) return null;
  try {
    const decoded = atob(s);
    // 解码结果必须是可打印 ASCII（排除二进制噪声）
    if (!/^[\x20-\x7e]+$/.test(decoded)) return null;
    return decoded;
  } catch {
    return null;
  }
}

/**
 * 归一化用于平台关键字匹配：小写 + 非 alnum/CJK → 空格 + 折叠空白。
 * 与 Platforms.tsx 既有「空格分词 substring」关键字惯例对齐。
 */
export function normalizeForMatch(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9一-鿿]+/gi, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function pushUnique(arr: string[], v: string) {
  if (v && !arr.includes(v)) arr.push(v);
}

/** 抽取 apikey 候选。 */
function extractApiKeys(text: string): string[] {
  const keys: string[] = [];

  // 旁注（如「（base64编码）」）会夹在 key 标签与分隔符之间阻断 ASSIGN_RE，
  // 先剔除。该短语永不出现于真实 key/url，全局 strip 安全。
  const cleaned = text.replace(BASE64_NOTE_RE, "");

  // 1) 前缀锚定（覆盖 sk-/tp-/sk-kimi-，含防爬汉字穿插）
  for (const m of cleaned.matchAll(PREFIX_TOKEN_RE)) {
    const clean = stripCjk(m[0]);
    if (clean.length >= 16) pushUnique(keys, clean);
  }

  // 2) 赋值锚定（覆盖 API_KEY= / 秘药： 等；含无标准前缀 + base64 编码 key）
  for (const m of cleaned.matchAll(ASSIGN_RE)) {
    const raw = stripCjk(m[1]);
    if (!raw) continue;
    if (hasKnownPrefix(raw)) {
      if (raw.length >= 16) pushUnique(keys, raw);
      continue;
    }
    // 无已知前缀 → 尝试 base64 解码
    const decoded = tryBase64Decode(raw);
    if (decoded && decoded.length >= 12) {
      pushUnique(keys, decoded);
    } else if (raw.length >= 24) {
      // 解不出也保留原始长串（可能是非标准前缀的明文 key）
      pushUnique(keys, raw);
    }
  }

  // 3) 裸 base64 兜底（无 key 标签 / 旁注的整段 base64）。
  //    decoded 须键形（短前缀 + 长串）或带已知前缀才采纳，排除解码噪声 / URL 片段误报。
  for (const m of cleaned.matchAll(BARE_BASE64_RE)) {
    const decoded = tryBase64Decode(m[0]);
    if (!decoded || decoded.length < 12) continue;
    if (hasKnownPrefix(decoded) || DECODED_KEY_SHAPE.test(decoded)) {
      pushUnique(keys, decoded);
    }
  }

  return keys;
}

export function guessProtocol(url: string): ParsedProtocol {
  const u = url.toLowerCase();
  if (/anthrop/.test(u)) return "anthropic"; // 容错截断 "anthropi"
  if (/gemini|generativelanguage/.test(u)) return "gemini";
  if (/openai|\/v1(\/|\b)/.test(u)) return "openai";
  return "unknown";
}

/** 抽取 base_url 候选。 */
function extractBaseUrls(text: string): ParsedBaseUrl[] {
  const out: ParsedBaseUrl[] = [];
  const seen = new Set<string>();
  const URL_RE = /https?:\/\/[^\s"'“”‘’《」】\])，。；、>]+/g;
  for (const m of text.matchAll(URL_RE)) {
    let url = m[0].replace(/[.,;:。，；、)）"'"'“”‘’》」】>]+$/, "");
    if (!url) continue;
    // 跳过图片 / 静态资源
    if (/\.(png|jpe?g|gif|webp|svg|ico)(\?|$)/i.test(url)) continue;
    if (seen.has(url)) continue;
    seen.add(url);
    out.push({ url, protocol: guessProtocol(url) });
  }
  return out;
}

/** 匹配内置平台 preset。
 *  优先级 1：base_url 完整 URL 子串命中 preset.hosts（最强信号，多 preset 重叠时最长串胜出）。
 *          hosts 存 hostname（如 api.deepseek.com）或含 path 的 URL 子串（如
 *          open.bigmodel.cn/api/coding 区分 coding/普通同 host 分裂）。hostname 是 URL 子串
 *          的特例，故向后兼容。
 *  优先级 2：keyword 文本扫描（fallback，按 presets 列表顺序首个命中）。
 *  返回 codingPlan 标记（透传到 applyPaste 选对普通/coding 变体的 endpoints）。 */
export function matchPlatform(
  text: string,
  presets: PastePresetRef[],
  baseUrls?: ParsedBaseUrl[],
): { value: string; label: string; codingPlan?: boolean } | null {
  // 1) base_url URL 子串优先匹配：收集所有命中，取最长串（最特异）对应的 preset。
  //    例：粘贴 token-plan-cn.xiaomimimo.com 时，coding preset（host token-plan-cn.xiaomimimo.com）
  //    比普通 preset（host api.xiaomimimo.com）更特异而胜出，避免被普通版误匹配。
  //    同 host 分裂（如 glm open.bigmodel.cn）靠 path 子串（/api/coding vs /api/paas/v4）区分。
  if (baseUrls && baseUrls.length) {
    const urls = baseUrls.map((b) => b.url.toLowerCase());
    if (urls.length) {
      let best: { value: string; label: string; codingPlan?: boolean } | null = null;
      let bestLen = 0;
      for (const p of presets) {
        for (const h of p.hosts ?? []) {
          const hl = h.toLowerCase();
          if (urls.some((u) => u.includes(hl)) && hl.length > bestLen) {
            best = { value: p.value, label: p.label, codingPlan: p.codingPlan };
            bestLen = hl.length;
          }
        }
      }
      if (best) return best;
    }
  }

  // 2) fallback: keyword 文本扫描（与 presets 列表顺序一致，首个命中胜出）。
  const hay = normalizeForMatch(text);
  for (const p of presets) {
    for (const kw of p.keywords ?? []) {
      const needle = normalizeForMatch(kw);
      if (needle && hay.includes(needle)) {
        return { value: p.value, label: p.label, codingPlan: p.codingPlan };
      }
    }
  }
  return null;
}

/**
 * 解析粘贴文本 → {apiKeys, baseUrls, platform}。
 * @param text 用户粘贴的原始文案
 * @param presets Platforms.tsx 的 PLATFORM_PRESETS（提供 value/label/keywords/hosts）
 */
export function parsePlatformPaste(
  text: string,
  presets: PastePresetRef[],
): ParsedPaste {
  if (!text || !text.trim()) {
    return { apiKeys: [], baseUrls: [], platform: null };
  }
  const baseUrls = extractBaseUrls(text);
  return {
    apiKeys: extractApiKeys(text),
    baseUrls,
    platform: matchPlatform(text, presets, baseUrls),
  };
}
