import test from "node:test";
import assert from "node:assert/strict";
import { checkAnnotations } from "./check-command-annotations.mjs";

const wrap = (body) => `macro_rules! rpc_routes { () => {} }\n\nrpc_routes! {\n${body}\n}\n`;

test("坏样本：缺 readonly|mutating 标注 → 报错并带行号", () => {
  const errors = checkAnnotations(wrap("    readonly a::b::ok_cmd,\n    a::b::missing_cmd,"));
  assert.equal(errors.length, 1);
  assert.match(errors[0], /missing_cmd/);
  assert.match(errors[0], /缺标注/);
});

test("坏样本：性质词写错 → 报错", () => {
  assert.ok(checkAnnotations(wrap("    readwrite a::b::cmd,")).some((e) => /缺标注/.test(e)));
});

test("坏样本：命令名重复 → 报错", () => {
  const errors = checkAnnotations(wrap("    readonly a::x::cmd,\n    mutating b::y::cmd,"));
  assert.ok(errors.some((e) => /重复/.test(e)));
});

test("好样本：注释、空行、[exports_secret] 都放行", () => {
  const errors = checkAnnotations(
    wrap("    // 注释\n\n    readonly a::b::cmd1,\n    mutating a::b::cmd2 [exports_secret],"),
  );
  assert.deepEqual(errors, []);
});

test("真实命令表通过", async () => {
  const { readFileSync } = await import("node:fs");
  assert.deepEqual(checkAnnotations(readFileSync("src-tauri/crates/aidog_rpc/src/lib.rs", "utf8")), []);
});
