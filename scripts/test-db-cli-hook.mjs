// 自测 lazygophers.json 里 PreToolUse/Bash 的数据库 CLI 钩子：取 JSON 里的 command，sh -c 真跑，stdin 喂 tool_input。
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import assert from "node:assert/strict";
import test from "node:test";

const file = new URL("../src-tauri/defaults/settings/lazygophers.json", import.meta.url);
const cfg = JSON.parse(readFileSync(file, "utf8"));
const hook = cfg.hooks.PreToolUse.flatMap((g) => (g.matcher === "Bash" ? g.hooks : [])).find((h) =>
  h.command.includes("mysqldump"),
);
assert.ok(hook, "未找到数据库 CLI 钩子");

const ask = (command) => {
  const r = spawnSync("sh", ["-c", hook.command], { input: JSON.stringify({ tool_input: { command } }), encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  if (r.stdout === "") return false;
  const o = JSON.parse(r.stdout).hookSpecificOutput;
  assert.equal(o.permissionDecision, "ask");
  assert.equal(o.hookEventName, "PreToolUse");
  assert.equal(o.permissionDecisionReason, "数据库 CLI 直操作或经 ssh/docker exec 嵌套执行，需确认");
  return true;
};

const block = [
  "psql -c x",
  "cd a && mysql -e y",
  'ssh h "redis-cli keys *"',
  "ssh -p 22 -i k h psql",
  "docker exec -it pg psql",
  "docker run --rm -e A=1 img psql -c x",
  "kubectl exec -it pod -n ns -- psql",
  "sudo -u postgres psql",
  'bash -c "sqlite3 a.db"',
  "echo $(mysqldump db)",
  "echo `mysqldump db`",
  'echo "$(mysqldump db)"',
  "FOO=1 pg_dump x",
  "/usr/local/bin/psql",
  "cat f | xargs -n1 psql",
  "timeout 5 mongosh",
  "env -u A mysql",
  "cat <<EOF | psql\nx\nEOF",
  "psql <<EOF\nselect 1;\nEOF",
  "git status\npsql x",
  "if psql -c x; then echo ok; fi",
  "cat <(mongodump)",
  "docker run --entrypoint psql img",
  "cat <<'EOF'\npsql x", // 未结束 heredoc 回落旧正则：宁可多问
];
const pass = [
  "cat >> f.md <<EOF\npsql x\nEOF",
  "grep sqlite3 src/",
  "echo mysql",
  'git commit -m "fix psql"',
  'git commit -m "$(cat <<\'EOF\'\nfix psql\nEOF\n)"',
  "ls /usr/lib/mongo",
  "vim mysql.conf",
  "docker exec mysql ls",
  'rg "redis-cli" .',
  "cargo test rusqlite",
  "cat > mysql.conf",
  "command -v psql",
  "ssh h ls psql_dir",
];

for (const c of block) test(`ask: ${JSON.stringify(c)}`, () => assert.equal(ask(c), true));
for (const c of pass) test(`pass: ${JSON.stringify(c)}`, () => assert.equal(ask(c), false));
