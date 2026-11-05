import { test } from "node:test";
import assert from "node:assert/strict";
import { buildCatalog, categoryOf, parseSignature } from "./gen-api-catalog.mjs";

const rpc = `
rpc_routes! {
    readonly aidog_core::platform_cmd::platform::platform_get,
    mutating aidog_backup::commands::backup_run [exports_secret],
}
`;
const rust = [`
crate::tauri_command! {
pub async fn platform_get(id: u64, app: tauri::AppHandle) -> Result<Option<Platform>, String> { 1 }
}
pub async fn backup_run(
    include_keys: bool, names: Vec<(String, u8)>) -> Result<(), String> { 2 }
pub fn tray_only(win_name: String) -> Result<String, String> { 3 }
`];
const startup = `generate_handler![ aidog_core::platform_cmd::platform::platform_get, aidog_backup::commands::backup_run, aidog_core::tray::tray_only, ])`;

test("signature: camelCase params, desktop-injected skipped, Result unwrapped", () => {
  const s = parseSignature(rust[0], "backup_run");
  assert.deepEqual(s.params, [{ name: "includeKeys", type: "bool" }, { name: "names", type: "Vec<(String, u8)>" }]);
  assert.equal(s.returns, "()");
  assert.deepEqual(parseSignature(rust[0], "platform_get"), { params: [{ name: "id", type: "u64" }], returns: "Option<Platform>" });
});

test("category from path", () => {
  assert.equal(categoryOf("aidog_core::platform_cmd::platform::platform_get"), "platform");
  assert.equal(categoryOf("aidog_backup::commands::backup_run"), "backup");
  assert.equal(categoryOf("aidog_core::settings::x"), "settings");
});

test("catalog: rpc rows + desktop-only rows, flags", () => {
  const c = buildCatalog(rpc, rust, startup).commands;
  assert.deepEqual(c.map((r) => [r.name, r.kind, r.exportsSecret, r.desktopOnly]), [
    ["backup_run", "mutating", true, false],
    ["platform_get", "readonly", false, false],
    ["tray_only", null, false, true],
  ]);
});

test("catalog: unknown signature throws", () => {
  assert.throws(() => buildCatalog(rpc, [""], startup), /找不到命令/);
});

test("signature: line comments inside params are dropped", () => {
  const s = parseSignature("pub fn f(a: u8,\n    // note: x\n    b: Option<String>) -> Result<(), String> {}", "f");
  assert.deepEqual(s.params, [{ name: "a", type: "u8" }, { name: "b", type: "Option<String>" }]);
});
