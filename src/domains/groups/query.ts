import type { Platform, GroupDetail } from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";

/** 平台搜索匹配（与 Platforms.tsx standalonePlatforms filter 同口径：name/base_url/platform_type 拼音） */
export function platformMatchesQuery(p: Platform, q: string): boolean {
  return pinyinMatch(q, p.name)
    || pinyinMatch(q, p.base_url)
    || pinyinMatch(q, p.platform_type);
}

/** 分组名匹配（命中分组名时整组展开，语义合理保留） */
export function groupMatchesQuery(group: GroupDetail["group"], q: string): boolean {
  return pinyinMatch(q, group.name) || pinyinMatch(q, group.group_key);
}
