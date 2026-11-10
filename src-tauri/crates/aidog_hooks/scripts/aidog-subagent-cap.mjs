#!/usr/bin/env node
// aidog subagent 硬并发上限闸门（lazygophers 预设 hooks 分发，node 零依赖）。
// 上限：env CLAUDE_CODE_SUBAGENT_HARD_CAP（纯数字生效，非法/缺省 = 2）。
// 账本：~/.aidog/subagent-cap/<session_id>.json = {"agents":[{"id","ts"}]}。
// 漂移兜底：登记超 4 小时视为已死；SessionEnd 清本会话；`--reset <session_id|all>` 手动清零。
// 销账取舍：SubagentStart/SubagentStop 按 payload.agent_id 精确登记/销账（CC 2.1.319
// 官方文档保证两事件都带 agent_id）；payload 缺 agent_id 时不动账本——不猜「最早一条」，
// 靠 4h 过期与 SessionEnd 兜底。内部代理（prompt 建议等）也会触发 SubagentStart/Stop，
// 短暂占一个槽，结束即销，可接受。
// 并发说明：同一会话并行 hooks 可能丢一次登记（写写竞态），闸门偏松不偏紧，可接受。

import { readFileSync, writeFileSync, renameSync, mkdirSync, unlinkSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const EXPIRE_MS = 4 * 60 * 60 * 1000;

const cap = () => {
  const n = parseInt(process.env.CLAUDE_CODE_SUBAGENT_HARD_CAP, 10);
  return Number.isInteger(n) && n > 0 ? n : 2;
};

const stateDir = () => join(homedir(), ".aidog", "subagent-cap");
const stateFile = (sid) => join(stateDir(), `${sid}.json`);

const readState = (sid) => {
  try {
    const s = JSON.parse(readFileSync(stateFile(sid), "utf8"));
    return Array.isArray(s.agents) ? s : { agents: [] };
  } catch {
    return { agents: [] };
  }
};

const writeState = (sid, state) => {
  mkdirSync(stateDir(), { recursive: true });
  const tmp = `${stateFile(sid)}.tmp`;
  writeFileSync(tmp, JSON.stringify(state));
  renameSync(tmp, stateFile(sid));
};

const prune = (state, now = Date.now()) => {
  state.agents = state.agents.filter((a) => a && a.id && now - a.ts < EXPIRE_MS);
  return state;
};

const deny = () => {
  const msg = `并发 subagent 上限 ${cap()} 已满，请等现有 agent 结束或让它完成后重试`;
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: msg,
      },
    }),
  );
  process.exit(0);
};

// --reset <session_id|all>：手动清零。
if (process.argv[2] === "--reset") {
  const target = process.argv[3];
  if (target === "all") {
    try {
      for (const f of readdirSync(stateDir())) unlinkSync(join(stateDir(), f));
    } catch {}
  } else if (target) {
    try {
      unlinkSync(stateFile(target));
    } catch {}
  }
  process.exit(0);
}

let payload = {};
try {
  payload = JSON.parse(readFileSync(0, "utf8"));
} catch {}
const sid = payload.session_id;
if (!sid) process.exit(0);
const event = payload.hook_event_name;

if (event === "SessionEnd") {
  try {
    unlinkSync(stateFile(sid));
  } catch {}
  process.exit(0);
}

if (event === "SubagentStart") {
  const id = payload.agent_id;
  if (id) {
    const st = prune(readState(sid));
    if (!st.agents.some((a) => a.id === id)) st.agents.push({ id, ts: Date.now() });
    writeState(sid, st);
  }
  process.exit(0);
}

if (event === "SubagentStop") {
  const id = payload.agent_id;
  if (id) {
    const st = prune(readState(sid));
    st.agents = st.agents.filter((a) => a.id !== id);
    writeState(sid, st);
  }
  process.exit(0);
}

if (event === "PreToolUse") {
  const st = prune(readState(sid));
  const tool = payload.tool_name;
  const input = payload.tool_input || {};
  if (tool === "Agent") {
    // 新 spawn 必占新槽。
    if (st.agents.length >= cap()) deny();
  } else if (tool === "SendMessage") {
    // 给已登记（正在跑）的 agent 发消息不占新槽；目标是未登记 agent（resume 已结束
    // agent / 新 teammate）占新槽。只按 agent_id 匹配：按名字 SendMessage 的活 agent
    // 在满额时会被误拒（保守方向，安全优先）。
    const target = String(input.to ?? input.agentId ?? input.id ?? "");
    const live = target && st.agents.some((a) => a.id === target);
    if (!live && st.agents.length >= cap()) deny();
  }
}
process.exit(0);
