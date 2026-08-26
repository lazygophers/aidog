// 能力徽标：registry capabilities 枚举 → i18n 标签。
// 枚举值 text / vision / image_gen / tool_use / reasoning / audio / video / embedding，
// 未知值（registry 先行加了新枚举而前端 locale 未跟上）原样显示裸值，不隐藏。

import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";

/** 已知能力枚举；决定 `modelInfo.cap.*` i18n key 覆盖面。 */
export const CAPABILITIES = [
  "text", "vision", "image_gen", "tool_use", "reasoning", "audio", "video", "embedding",
] as const;

export function capabilityLabel(t: (k: string) => string, cap: string): string {
  if (!(CAPABILITIES as readonly string[]).includes(cap)) return cap;
  return t(`modelInfo.cap.${cap}`);
}

export function CapabilityBadges({ capabilities }: { capabilities: string[] }) {
  const { t } = useTranslation();
  if (capabilities.length === 0) return <span className="text-tertiary">-</span>;
  return (
    <span style={{ display: "inline-flex", gap: 4, flexWrap: "wrap" }}>
      {capabilities.map(c => (
        <Badge key={c} variant="secondary" style={{ fontSize: 10, padding: "1px 6px", border: "none" }}>
          {capabilityLabel(t, c)}
        </Badge>
      ))}
    </span>
  );
}
