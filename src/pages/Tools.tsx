// ─── 工具页（扩展 → 工具）：AiDog 自带工具的配置（搜索 / 生图 / 生视频），第三方 MCP 在 MCP 页 ──
import { ToolsSection } from "./Tools/ToolsSection";

export function Tools() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      <ToolsSection />
    </div>
  );
}
