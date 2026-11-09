// toolLog.ts — 工具日志（日志统计 → 工具日志）：网关代执行 / HTTP MCP 的每次工具调用。

import { invoke } from "../transport";
import type { ToolLog, ToolLogFilter, ToolLogPage } from "./types";

export const toolLogApi = {
  /** 分页列表（倒序，可按工具 / 触发方 / 分组筛选；不带结果正文）。 */
  list: (filter: ToolLogFilter, limit: number, offset: number) =>
    invoke<ToolLogPage>("tool_log_list", { filter, limit, offset }),
  /** 单条详情（带结果正文；记录原始信息开关关时正文为空）。 */
  get: (id: number) => invoke<ToolLog | null>("tool_log_get", { id }),
  /** 保留天数（0 = 永久）。 */
  retentionGet: () => invoke<number>("tool_log_retention_get"),
  retentionSet: (days: number) => invoke<void>("tool_log_retention_set", { days }),
};
