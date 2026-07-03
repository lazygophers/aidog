// ─── MCP 管理页 (facade) ──────────────────────────────────
// 顶层侧栏入口。集中管 Claude Code / Codex 的 MCP server 配置。
// - 扫描导入：拉两 agent 配置 → 去重合并 → 勾选入 aidog DB（enabled = 来源 agent）。
// - per-agent 启用/禁用：每行右侧 claude/codex 图标，启用=accent，禁用=grayscale。
// - 删除：从 DB + 所有 enabled agent 配置移除（二次确认）。
//
// env/headers 敏感值经后端 mask_env 脱敏（***）。写 agent 配置用 DB 原值。
// transport: stdio（command+args）/ http|sse（url+headers）。codex 仅支持 stdio。
//
// 拆分（arch 阶段6 S4）：state/actions 外迁 Mcp/useMcpData，JSX 分 McpView（主列表）+
// McpModals（5 个 portal 弹窗），行级组件/工具在 Mcp/primitives，常量/样式在 Mcp/constants + styles。
// 外部 import 路径（App.tsx `from "./pages/Mcp"`）零 churn。

import { useMcpData } from "./Mcp/useMcpData";
import { McpView } from "./Mcp/McpView";
import { McpModals } from "./Mcp/McpModals";

export function Mcp() {
  const d = useMcpData();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      <McpView d={d} />
      <McpModals d={d} />
    </div>
  );
}
