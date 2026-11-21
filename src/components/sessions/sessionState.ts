// 会话运行状态本地派生（扩展-会话页 票 05，spec S7 / S8）。
// 后端给出 state + source + lastActivityMs；source = activity 的会话由前端按阈值随时间重新派生，
// pid_file / gateway 来源只随数据事件更新。阈值与 Rust `client_usage_cmd/state.rs` 对称。
import type { ClientUsageSessionState, SessionRunState } from "../../services/api/clientUsage";

export const RUNNING_WITHIN_MS = 2 * 60_000;
export const IDLE_WITHIN_MS = 30 * 60_000;

export function activityState(lastActivityMs: number, nowMs: number): SessionRunState {
  const age = nowMs - lastActivityMs;
  return age <= RUNNING_WITHIN_MS ? "running" : age <= IDLE_WITHIN_MS ? "idle" : "stopped";
}

/** 会话当前状态：无后端结果时按最近调用时间派生。 */
export function deriveState(s: ClientUsageSessionState | undefined, lastCallMs: number, nowMs: number): SessionRunState {
  if (s && s.source !== "activity") return s.state;
  return activityState(s ? s.lastActivityMs : lastCallMs, nowMs);
}

/** 下一个阈值到期时刻（距 now 的毫秒数）；没有会再变化的会话返回 null。 */
export function nextFlipDelay(lastActivities: number[], nowMs: number): number | null {
  let best: number | null = null;
  for (const last of lastActivities) {
    for (const edge of [last + RUNNING_WITHIN_MS, last + IDLE_WITHIN_MS]) {
      const d = edge - nowMs + 1;
      if (d > 0 && (best === null || d < best)) best = d;
    }
  }
  return best;
}
