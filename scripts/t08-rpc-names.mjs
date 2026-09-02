// 票 08 漂移护栏：从 `src-tauri/crates/aidog_kernel/src/rpc.rs` 的 `rpc_routes! { ... }`
// 抽出登记的命令名集合（= 每条路径的末段），排序后逐行输出。
//
// 用法（与 t06 两个脚本同规矩：**表头走 stderr，命令名走 stdout**，可以直接 diff）：
//   diff <(node scripts/t06-handler-names.mjs 2>/dev/null) \
//        <(node scripts/t08-rpc-names.mjs     2>/dev/null)
//   必须无输出 —— Tauri 注册表与 /rpc 路由表是同一批命令。
import { readFileSync } from "fs";

const path = process.argv[2] ?? "src-tauri/crates/aidog_kernel/src/rpc.rs";
const src = readFileSync(path, "utf8");

const block = src.match(/rpc_routes!\s*\{([\s\S]*?)\n\}/);
if (!block) {
  console.error(`${path}: 找不到 rpc_routes! { ... } 块`);
  process.exit(1);
}

const names = [];
for (const line of block[1].split("\n")) {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("//")) continue;
  const m = trimmed.match(/^([A-Za-z0-9_:]+),$/);
  if (!m) continue;
  names.push(m[1].split("::").pop());
}

names.sort();
console.error(`# ${path}: ${names.length} commands`);
console.log(names.join("\n"));
