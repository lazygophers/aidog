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

/** RTK 双侧开关结果：一侧失败时错误单独回传，另一侧状态照实。 */
export interface RtkSetResult {
  status: ContextCompressStatus;
  claude_error: string | null;
  codex_error: string | null;
}

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

/** `rtk gain -d --format json` 的 daily[] 一项；savings_pct 为当日节省率。 */
export interface RtkDaily {
  date: string;
  commands: number;
  input_tokens: number;
  output_tokens: number;
  saved_tokens: number;
  savings_pct: number;
}

/** `sqz gain` 文本解析的一日（CLI 无每日比率 / in-out，只有节省 tokens）。 */
export interface SqzDaily {
  date: string;
  saved_tokens: number;
}

/** 工具未装 / 解析失败的一侧 summary 为 null；daily 为空表示无按日数据。 */
export interface ContextCompressStats {
  rtk: RtkGainStats | null;
  sqz: SqzStats | null;
  rtk_daily: RtkDaily[];
  sqz_daily: SqzDaily[];
}

export const contextCompressApi = {
  status: () => invoke<ContextCompressStatus>("context_compress_status"),
  stats: () => invoke<ContextCompressStats>("context_compress_stats"),
  install: (tool: CompressTool) => invoke<ContextCompressStatus>("context_compress_install", { tool }),
  setRtk: (on: boolean) => invoke<RtkSetResult>("context_compress_rtk_set", { on }),
  setSqz: (on: boolean) => invoke<ContextCompressStatus>("context_compress_sqz_set", { on }),
};
