// aidog-subagent-cap.mjs 自检（node 零依赖）：spawn 脚本喂 stdin payload，隔离 HOME。
// 跑法：node src-tauri/crates/aidog_hooks/scripts/aidog-subagent-cap.test.mjs
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import assert from "node:assert";

const script = join(import.meta.dirname, "aidog-subagent-cap.mjs");
const home = mkdtempSync(join(tmpdir(), "subagent-cap-test-"));
const file = (sid) => join(home, ".aidog", "subagent-cap", `${sid}.json`);

const run = (payload, env = {}) =>
  spawnSync(process.execPath, [script], {
    input: JSON.stringify(payload),
    env: { HOME: home, ...env },
    encoding: "utf8",
  });

const denyMsg = (out) => {
  const j = JSON.parse(out.stdout);
  assert.equal(j.hookSpecificOutput.permissionDecision, "deny");
  assert.ok(j.hookSpecificOutput.permissionDecisionReason.includes("已满"));
};
const allow = (r) => assert.equal(r.stdout, "");
const start = (sid, id) => allow(run({ session_id: sid, hook_event_name: "SubagentStart", agent_id: id }));
const stop = (sid, id) => allow(run({ session_id: sid, hook_event_name: "SubagentStop", agent_id: id }));
const gate = (sid, tool, input) => run({ session_id: sid, hook_event_name: "PreToolUse", tool_name: tool, tool_input: input });

const S = "sess-1";
try {
  // 1. 默认上限 2：登记两个后新 spawn 拒（缺 session_id / 未知事件静默放行）。
  allow(run({ hook_event_name: "PreToolUse", tool_name: "Agent", tool_input: {} }));
  start(S, "a1");
  start(S, "a2");
  denyMsg(gate(S, "Agent", {}));

  // 2. SendMessage：给在跑 agent 发消息满额放行；未登记目标（resume 已结束 agent）满额拒。
  allow(gate(S, "SendMessage", { to: "a2" }));
  denyMsg(gate(S, "SendMessage", { to: "new-guy" }));

  // 3. 销账：停掉一个后新 spawn 放行；再登记又满。
  stop(S, "a1");
  allow(gate(S, "Agent", {}));
  start(S, "a3");
  denyMsg(gate(S, "Agent", {}));
  allow(gate(S, "SendMessage", { to: "a3" }));

  // 4. 过期兜底：登记超 4h 视为已死，满额解除。
  const st = JSON.parse(readFileSync(file(S), "utf8"));
  st.agents = st.agents.map((a) => ({ ...a, ts: Date.now() - 4 * 60 * 60 * 1000 - 1000 }));
  writeFileSync(file(S), JSON.stringify(st));
  allow(gate(S, "Agent", {}));

  // 5. env 上限可配：cap=1 第二个拒；非法值回落默认 2。
  allow(gate("s2", "Agent", {}));
  start("s2", "b1");
  denyMsg(run({ session_id: "s2", hook_event_name: "PreToolUse", tool_name: "Agent", tool_input: {} }, { CLAUDE_CODE_SUBAGENT_HARD_CAP: "1" }));
  allow(gate("s3", "Agent", {}));
  start("s3", "c1");
  start("s3", "c2");
  denyMsg(run({ session_id: "s3", hook_event_name: "PreToolUse", tool_name: "Agent", tool_input: {} }, { CLAUDE_CODE_SUBAGENT_HARD_CAP: "abc" }));

  // 6. --reset all 清零后放行。
  spawnSync(process.execPath, [script, "--reset", "all"], { env: { HOME: home }, encoding: "utf8" });
  allow(gate(S, "Agent", {}));

  // 7. SessionEnd 清本会话账本。
  start(S, "a9");
  assert.ok(JSON.parse(readFileSync(file(S), "utf8")).agents.some((a) => a.id === "a9"));
  run({ session_id: S, hook_event_name: "SessionEnd" });
  assert.throws(() => readFileSync(file(S), "utf8"), /ENOENT/);

  console.log("subagent-cap self-test: 7 groups all green");
} finally {
  rmSync(home, { recursive: true, force: true });
}
