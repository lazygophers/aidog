import { type SkillAgent } from "../../services/api";
import claudeIcon from "../../assets/platforms/claude_code.svg";
import codexIcon from "../../assets/platforms/openai.svg";

// ponytail: slug 与 Mcp AGENTS 不同（Mcp 用 claude-code），不合，各留本地常量。
export const AGENTS: SkillAgent[] = ["claude", "codex"];
export const AGENT_ICONS: Record<SkillAgent, string> = { claude: claudeIcon, codex: codexIcon };
