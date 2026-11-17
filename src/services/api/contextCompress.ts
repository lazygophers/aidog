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

/** rtk `gain --format json` 的 summary。 */
export interface RtkGainStats {
  total_commands: number;
  total_input: number;
  total_output: number;
  total_saved: number;
  avg_savings_pct: number;
}

/** sqz `stats --json`（聚合）。 */
export interface SqzStats {
  totalCompressions: number;
  tokensIn: number;
  tokensOut: number;
  tokensSaved: number;
  avgReduction: number;
}

/** 工具未装 / 解析失败的一侧为 null。 */
export interface ContextCompressStats {
  rtk: RtkGainStats | null;
  sqz: SqzStats | null;
}

export const contextCompressApi = {
  status: () => invoke<ContextCompressStatus>("context_compress_status"),
  stats: () => invoke<ContextCompressStats>("context_compress_stats"),
  install: (tool: CompressTool) => invoke<ContextCompressStatus>("context_compress_install", { tool }),
  setRtk: (agent: RtkAgent, on: boolean) =>
    invoke<ContextCompressStatus>("context_compress_rtk_set", { agent, on }),
  setSqz: (on: boolean) => invoke<ContextCompressStatus>("context_compress_sqz_set", { on }),
};
