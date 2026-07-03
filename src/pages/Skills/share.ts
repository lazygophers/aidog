import { type SkillInfo } from "../../services/api";

/** skill → catalog id（`owner/repo@skill`）。无 source（手动 symlink，非 catalog 来源）→ null，不可分享。 */
export function skillCatalogId(s: SkillInfo): string | null {
  return s.source && s.name ? `${s.source}@${s.name}` : null;
}

/** 分享载荷：skill catalog id 列表（接收端 base64 → JSON → skills_install）。 */
export interface SkillSharePayload {
  skills: string[];
}

/** 解码 base64 分享文本 → skill id 列表（接受 {skills: [...]} 包裹或裸 [...]）。
 *  返回 null = 非法格式。明文 base64(JSON) 与 ShareModal.copyUrl 产出对齐。 */
export function decodeSkillShare(text: string): string[] | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  let json = trimmed;
  // 形如 base64（[A-Za-z0-9+/=] + 足够长）→ 尝试 atob；失败保持原文本走 JSON.parse。
  if (/^[A-Za-z0-9+/=\s]+$/.test(trimmed) && trimmed.length > 16) {
    try {
      json = atob(trimmed.replace(/\s/g, ""));
    } catch {
      // 非 base64，走原文本 JSON.parse。
    }
  }
  try {
    const parsed: unknown = JSON.parse(json);
    const ids = Array.isArray(parsed)
      ? parsed
      : Array.isArray((parsed as { skills?: unknown })?.skills)
        ? (parsed as { skills: unknown[] }).skills
        : null;
    if (!ids) return null;
    // 每项必须是 owner/repo@skill 形态（含 @）。
    const valid = ids.every((id) => typeof id === "string" && id.includes("@"));
    return valid ? (ids as string[]) : null;
  } catch {
    return null;
  }
}
