// 票 09 漂移护栏：前端 invoke 的每个命令名，内核 `/rpc` 路由表里必须都有。
//
// 为什么需要：传输层分流后，浏览器形态把 `invoke(name, args)` 打到 `POST /rpc/<name>`。
// 名字不在内核路由表里 → **只有浏览器形态 404**，桌面形态照常工作。这种偏差在桌面上
// 测不出来，必须静态查。
//
// 反向差集（内核有、前端不叫）是正常的：不少命令只由 Rust 侧或托盘触发，不做要求。
//
//   yarn check:rpc
import { readdirSync, readFileSync, statSync } from "fs";
import { join } from "path";

const RPC_FILE = "src-tauri/crates/aidog_kernel/src/rpc.rs";
const SRC_DIRS = ["src"];

// ── 内核 /rpc 路由表（与 scripts/t08-rpc-names.mjs 同一份解析规则） ──
const rpcSrc = readFileSync(RPC_FILE, "utf8");
const block = rpcSrc.match(/rpc_routes!\s*\{([\s\S]*?)\n\}/);
if (!block) {
  console.error(`${RPC_FILE}: 找不到 rpc_routes! { ... } 块`);
  process.exit(1);
}
const rpcNames = new Set();
for (const line of block[1].split("\n")) {
  const m = line.trim().match(/^([A-Za-z0-9_:]+),$/);
  if (m) rpcNames.add(m[1].split("::").pop());
}

// ── 前端 invoke 的命令名 ──
// `invoke<T>("cmd"` / `invoke("cmd"`。命令名一律是字面量；出现变量名时这里抓不到，
// 评审时能一眼看见 invoke 后面不是字符串。
const CALL = /\binvoke\s*(?:<[^(]*?>)?\s*\(\s*"([a-z0-9_]+)"/g;

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.tsx?$/.test(p) && !/\.(test|spec)\.tsx?$/.test(p)) out.push(p);
  }
  return out;
}

const feNames = new Map(); // 命令名 → 首个调用点
for (const dir of SRC_DIRS) {
  for (const file of walk(dir)) {
    for (const m of readFileSync(file, "utf8").matchAll(CALL)) {
      if (!feNames.has(m[1])) feNames.set(m[1], file);
    }
  }
}

const missing = [...feNames].filter(([name]) => !rpcNames.has(name));

console.log(`前端 invoke ${feNames.size} 个命令；内核 /rpc 登记 ${rpcNames.size} 个`);
if (missing.length > 0) {
  console.error(`❌ 以下命令前端会调、内核 /rpc 表里没有（浏览器形态会 404）：`);
  for (const [name, file] of missing) console.error(`   ${name}  ← ${file}`);
  process.exit(1);
}
console.log("✅ 前端调得到的命令，内核 /rpc 表里全都有");
