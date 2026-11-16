// contextCompress.ts — 扩展 → 上下文压缩（RTK / sqz）。

import { invoke } from "../transport";

export interface ContextCompressStatus {
  rtk_version: string | null;
  /** Claude Code 全局配置里有 `rtk hook claude`。 */
  rtk_claude: boolean;
  /** `$CODEX_HOME/hooks.json` 里有 `rtk hook codex`。 */
  rtk_codex: boolean;
  /** `$CODEX_HOME` 目录存在。 */
  codex_present: boolean;
  sqz_version: string | null;
  sqz_mcp_version: string | null;
  /** 所有必需项齐全才为 true。 */
  sqz_enabled: boolean;
  /** 缺的项：hook:<事件> / self:<agent> / wrap:<agent>:<名称>。 */
  sqz_missing: string[];
}

export type CompressTool = "rtk" | "sqz";
export type RtkAgent = "claude-code" | "codex";

export const contextCompressApi = {
  status: () => invoke<ContextCompressStatus>("context_compress_status"),
  install: (tool: CompressTool) => invoke<ContextCompressStatus>("context_compress_install", { tool }),
  setRtk: (agent: RtkAgent, on: boolean) =>
    invoke<ContextCompressStatus>("context_compress_rtk_set", { agent, on }),
  setSqz: (on: boolean) => invoke<ContextCompressStatus>("context_compress_sqz_set", { on }),
};
