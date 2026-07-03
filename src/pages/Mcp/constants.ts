import { type McpAgentSlug } from "../../services/api";
import claudeIcon from "../../assets/platforms/claude_code.svg";
import codexIcon from "../../assets/platforms/openai.svg";

export const AGENTS: McpAgentSlug[] = ["claude-code", "codex"];
export const AGENT_ICONS: Record<McpAgentSlug, string> = {
  "claude-code": claudeIcon,
  codex: codexIcon,
};

/** codex 仅 stdio；http/sse MCP 不能给 codex 启用。 */
export function agentSupported(transport: string, agent: McpAgentSlug): boolean {
  if (agent === "codex") return transport === "stdio";
  return true; // claude-code 支持 stdio/http/sse
}

/** transport 配色 badge。 */
export function transportStyle(transport: string): { bg: string; fg: string } {
  switch (transport) {
    case "http":
    case "sse":
      // 远程传输走 accent 系；区分靠 transport 文字本身。
      return { bg: "var(--accent-subtle)", fg: "var(--accent)" };
    default:
      return { bg: "var(--bg-elevated)", fg: "var(--text-tertiary)" };
  }
}

/** 摘要：stdio→command + 首参；http/sse→url。 */
export function summaryOf(m: { transport: string; command: string; args: string[]; url: string }): string {
  if (m.transport === "stdio") {
    const first = m.args[0] ?? "";
    return [m.command, first].filter(Boolean).join(" ");
  }
  return m.url || "—";
}
