import React, { useState, useEffect } from "react";
import type { Protocol } from "../../services/api";
import { useProtocolLogo } from "./useProtocolLogo";
import { getProtocolColorMap } from "./defaults";

/** Protocol brand logo（缓存命中）+ 首字母圆圈 fallback。
 *
 *  用于 SearchableProtocolSelect 等只需 logo 一层展示的场景。
 *  PlatformCard 内部另含 bundled SVG / favicon 多层 fallback，不使用此组件。
 *
 *  ponytail: 单层缓存 logo + fallback，不引入多层抽象。 */
export function ProtocolLogo({
  protocol,
  size = 24,
}: {
  protocol: Protocol;
  size?: number;
}): React.ReactElement {
  const { logoSrc, fallbackInitial } = useProtocolLogo(protocol);
  // `<img>` onError（缓存文件存在但渲染失败 / 格式坏）→ 触发首字母圆圈 fallback
  const [imgFailed, setImgFailed] = useState(false);
  // 品牌色（async 派生自 platform-presets.json）；首帧 fallback var(--accent)。
  const [color, setColor] = useState<string>("var(--accent)");
  useEffect(() => {
    let cancelled = false;
    getProtocolColorMap().then(m => {
      if (!cancelled && m[protocol]) setColor(m[protocol]!);
    });
    return () => { cancelled = true; };
  }, [protocol]);

  if (logoSrc && !imgFailed) {
    return (
      <img
        src={logoSrc}
        alt={protocol}
        width={size}
        height={size}
        style={{
          width: size,
          height: size,
          objectFit: "contain",
          borderRadius: "var(--radius-sm)",
          flexShrink: 0,
        }}
        onError={() => setImgFailed(true)}
      />
    );
  }

  return (
    <span
      aria-hidden
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        background: `${color}25`,
        color,
        fontSize: Math.round(size * 0.45),
        fontWeight: 700,
        flexShrink: 0,
        userSelect: "none",
      }}
    >
      {fallbackInitial}
    </span>
  );
}
