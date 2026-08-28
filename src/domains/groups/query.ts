import type { Platform, GroupDetail } from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";

/** 平台搜索匹配（与 Platforms 页 standalonePlatforms filter 同口径：name/base_url/platform_type 拼音）。
 *  protocolTerms（getProtocolSearchTermsMap 派生）非空时追加协议搜索词匹配——
 *  用户自填名「GLM」也能被「智谱」「zhipu」「zai」等 registry 词条搜到（跨语言 + 拼音）。 */
export function platformMatchesQuery(
  p: Platform,
  q: string,
  protocolTerms?: Partial<Record<string, string[]>>,
): boolean {
  return pinyinMatch(q, p.name)
    || pinyinMatch(q, p.base_url)
    || pinyinMatch(q, p.platform_type)
    || !!protocolTerms?.[p.platform_type]?.some(t => pinyinMatch(q, t));
}

/** 分组名匹配（命中分组名时整组展开，语义合理保留） */
export function groupMatchesQuery(group: GroupDetail["group"], q: string): boolean {
  return pinyinMatch(q, group.name) || pinyinMatch(q, group.group_key);
}
