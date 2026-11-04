// 门禁：管理接口每个对外命令必须在声明处标注 readonly|mutating（可选 [exports_secret]）。
// 缺标注 / 形状不对 / 重名 → 退出码 1。
//   yarn check:annotations
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { RPC_FILE, parseRpcTable, readRpcTable } from "./lib/rpc-table.mjs";

/** 纯函数，便于坏样本测试：返回错误信息数组。 */
export function checkAnnotations(src) {
  const { entries, bad } = parseRpcTable(src);
  const errors = bad.map((b) => `第 ${b.line} 行缺标注或形状不合法（应为 \`readonly|mutating <路径> [exports_secret],\`）：${b.text}`);
  const seen = new Set();
  for (const e of entries) {
    if (seen.has(e.name)) errors.push(`第 ${e.line} 行命令名重复：${e.name}`);
    seen.add(e.name);
  }
  if (entries.length === 0) errors.push("命令表为空");
  return errors;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { entries } = readRpcTable();
  const errors = checkAnnotations(readFileSync(RPC_FILE, "utf8"));
  if (errors.length) {
    for (const e of errors) console.error(`❌ ${e}`);
    process.exit(1);
  }
  const ro = entries.filter((e) => e.kind === "readonly").length;
  const sec = entries.filter((e) => e.exportsSecret).length;
  console.log(`✅ ${entries.length} 个命令均已标注（readonly ${ro} / mutating ${entries.length - ro} / exports_secret ${sec}）`);
}
