import { useState } from "react";
import type { GroupDetail } from "../../services/api";
import { getPlatformLogo, getFaviconUrl } from "../../assets/platforms";

/**
 * Group 图标：仅关联 1 个平台、或组内唯一启用平台时跟随该平台 logo（与 Platforms 页一致），
 * 否则回退分组名首字文字框。口径须与 Rust 侧 `sole_platform`（gateway/router/mod.rs）保持对称，
 * 见 .skein/task/single-enabled-platform-shortcut/design.md §3。
 */
export function GroupIcon({ gps, group }: { gps: GroupDetail["platforms"]; group: GroupDetail["group"] }) {
  const [favFailed, setFavFailed] = useState(false);
  const enabled = gps.filter((gp) => gp.platform.status === "enabled");
  const single = gps.length === 1 ? gps[0].platform : enabled.length === 1 ? enabled[0].platform : null;
  const logo = single ? getPlatformLogo(single.platform_type) : undefined;
  const favicon = single && !logo && !favFailed ? getFaviconUrl(single) : null;
  const box = {
    width: 32, height: 32, borderRadius: "var(--radius-sm)", flexShrink: 0,
    display: "flex", alignItems: "center", justifyContent: "center",
  } as const;
  if (single && (logo || favicon)) {
    return (
      <div style={{ ...box, background: "transparent" }}>
        <img src={(logo || favicon) as string} alt={single.name}
          onError={() => { if (favicon) setFavFailed(true); }}
          style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
      </div>
    );
  }
  return (
    <div style={{
      ...box,
      background: group.auto_from_platform ? "var(--bg-glass)" : "var(--accent-subtle)",
      color: group.auto_from_platform ? "var(--text-secondary)" : "var(--accent)",
      // 自动建的组与手建的组，两档原先只差一层很淡的底色（深色强调色改成近黑之后
      // 是 1.05:1 与 1.15:1，肉眼分不出）。补一圈边把区分放回来：手建组用 accent-edge
      // 的亮边，自动组用普通 border。用户 2026-09-23 明确要求保住这个区分。
      border: `1px solid ${group.auto_from_platform ? "var(--border)" : "var(--accent-edge)"}`,
      fontSize: 13, fontWeight: 700,
    }}>
      {group.name.slice(0, 3)}
    </div>
  );
}
