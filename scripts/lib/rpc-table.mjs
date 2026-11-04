// 管理接口命令表（aidog_rpc 的 `rpc_routes! { ... }`）解析：三个脚本共用，不各抄一份正则。
// 每条目：`readonly|mutating <路径> [exports_secret],`；`//` 注释与空行跳过。
import { readFileSync } from "fs";

export const RPC_FILE = "src-tauri/crates/aidog_rpc/src/lib.rs";

/** 返回 { entries: [{name, path, kind, exportsSecret, line}], bad: [{line, text}] }。bad = 形状不合法（含缺标注）的行。 */
export function parseRpcTable(src) {
  const block = src.match(/\nrpc_routes!\s*\{([\s\S]*?)\n\}/);
  if (!block) throw new Error("找不到 rpc_routes! { ... } 块");
  const base = src.slice(0, block.index + 1).split("\n").length; // 块首行行号
  const entries = [];
  const bad = [];
  block[1].split("\n").forEach((raw, i) => {
    const t = raw.trim();
    if (!t || t.startsWith("//")) return;
    const m = t.match(/^(readonly|mutating)\s+([A-Za-z0-9_:]+)(\s+\[exports_secret\])?,$/);
    if (!m) return bad.push({ line: base + i, text: t });
    entries.push({
      kind: m[1],
      path: m[2],
      name: m[2].split("::").pop(),
      exportsSecret: !!m[3],
      line: base + i,
    });
  });
  return { entries, bad };
}

export function readRpcTable(file = RPC_FILE) {
  return parseRpcTable(readFileSync(file, "utf8"));
}
