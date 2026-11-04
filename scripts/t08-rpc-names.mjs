// 票 08 漂移护栏：从 `src-tauri/crates/aidog_rpc/src/lib.rs` 的 `rpc_routes! { ... }`
// 抽出登记的命令名集合（= 每条路径的末段），排序后逐行输出。
//
// 用法（与 t06 两个脚本同规矩：**表头走 stderr，命令名走 stdout**，可以直接 diff）：
//   diff <(node scripts/t06-handler-names.mjs 2>/dev/null) \
//        <(node scripts/t08-rpc-names.mjs     2>/dev/null)
//   必须无输出 —— Tauri 注册表与 管理接口命令表是同一批命令。
import { readFileSync } from "fs";
import { parseRpcTable } from "./lib/rpc-table.mjs";

const path = process.argv[2] ?? "src-tauri/crates/aidog_rpc/src/lib.rs";
const names = parseRpcTable(readFileSync(path, "utf8")).entries.map((e) => e.name);

names.sort();
console.error(`# ${path}: ${names.length} commands`);
console.log(names.join("\n"));
