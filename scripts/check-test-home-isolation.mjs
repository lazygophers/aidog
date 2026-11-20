#!/usr/bin/env node
// 测试 HOME 隔离检测脚本。
//
// 机制：把 HOME / CODEX_HOME / CLAUDE_CONFIG_DIR（与 aidog_db test_support.rs HomeGuard 同源的
// 三个变量）指向 tempdir 后跑一轮 cargo test；跑完对比真实 ~/.aidog 的文件清单快照，出现
// 「新文件」即判失败（泄漏实锤）。
//
// 噪声对策与检测口径（双侧）：
// 1. fakeHome 侧（主信号）：HOME 指向 tempdir 后，走 env HOME 的漏隔离测试写的是 fakeHome
//    而非真实 HOME——cargo 跑完后 fakeHome 下出现任何文件 = 有测试写 HOME 未用 HomeGuard，实锤。
// 2. 真实 ~/.aidog 侧（兜底，抓硬编码绝对路径的泄漏）：只比「新增文件」不比 mtime（常驻
//    AiDog 每秒改已有文件，mtime 全是噪声），且排除两类常驻侧新建噪声：logs/aidog.*.log
//    （整点日志轮转会新建文件，2026-11-20 实测误报）与 subagent-cap/<会话id>（会话注册）。
//    抓到即报，人工判断（残余误报 = 用户本人窗口内新建其它 ~/.aidog 文件，罕见）。
//
// 用法：
//   node scripts/check-test-home-isolation.mjs                                # 全量
//   node scripts/check-test-home-isolation.mjs -- -p aidog_db test_rw_pool    # 子集，同一流程
//
// 手动 / 定期执行，不挂 make lint（全量一轮太重）。
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const REAL_HOME = process.env.HOME;
const REAL_AIDOG = path.join(REAL_HOME, ".aidog");
const LOCK = "/tmp/aidog-cargo.lock";
// fakeHome 侧只查 aidog 相关点目录：cargo/mise 等 toolchain 会往 fake HOME 写缓存
// （2026-11-20 实测 mise 写 Library/Caches/mise/*），全目录对比全是工具噪声。
const FAKE_LEAK_DIRS = [".aidog", ".codex", ".claude", ".pi"];
const PROXY_VARS = [
  "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY",
  "http_proxy", "https_proxy", "all_proxy",
];

// 递归列真实 ~/.aidog 下的文件相对路径；跳过符号链接（不跟随），目录不可读按空处理。
function walkFiles(dir, prefix = "") {
  const acc = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return acc;
  }
  for (const e of entries) {
    if (e.isSymbolicLink()) continue;
    const rel = prefix ? `${prefix}/${e.name}` : e.name;
    if (e.isDirectory()) {
      for (const f of walkFiles(path.join(dir, e.name), rel)) acc.push(f);
    } else if (e.isFile()) {
      acc.push(rel);
    }
  }
  return acc;
}

function acquireLock() {
  for (;;) {
    try {
      fs.mkdirSync(LOCK);
      return;
    } catch (e) {
      if (e.code !== "EEXIST") throw e;
    }
  }
}

function releaseLock() {
  try {
    fs.rmdirSync(LOCK);
  } catch {
    /* 已被他人移除 */
  }
}

// ---------- main ----------
const sep = process.argv.indexOf("--");
const cargoArgs =
  sep >= 0 ? process.argv.slice(sep + 1) : ["--workspace", "--no-fail-fast"];

const before = new Set(walkFiles(REAL_AIDOG));
const fakeHome = fs.mkdtempSync(path.join(os.tmpdir(), "aidog-home-check-"));

let cargoExit = null;
acquireLock();
try {
  const env = { ...process.env };
  env.HOME = fakeHome;
  env.CODEX_HOME = path.join(fakeHome, ".codex");
  env.CLAUDE_CONFIG_DIR = path.join(fakeHome, ".claude");
  env.MISE_EXEC_AUTO_INSTALL = "0";
  // 固定 cargo/rustup home 到真实位置，防 HOME 改向后全量重下载 crate。
  env.CARGO_HOME = process.env.CARGO_HOME || path.join(REAL_HOME, ".cargo");
  env.RUSTUP_HOME = process.env.RUSTUP_HOME || path.join(REAL_HOME, ".rustup");
  for (const v of PROXY_VARS) delete env[v];
  const r = spawnSync("cargo", ["test", ...cargoArgs], {
    cwd: path.join(REPO, "src-tauri"),
    env,
    stdio: "inherit",
  });
  cargoExit = r.status;
} finally {
  releaseLock();
}

const leaks = [];

// 侧 1（主信号）：fakeHome 的 aidog 相关点目录下出现文件 = 有测试写 env HOME 未用 HomeGuard。
const fakeLeak = FAKE_LEAK_DIRS.flatMap((d) =>
  walkFiles(path.join(fakeHome, d), d),
);
if (fakeLeak.length > 0) {
  leaks.push(
    `fakeHome（env HOME 泄漏，测试写 HOME 未用 HomeGuard）：`,
    ...fakeLeak.map((f) => "  " + f),
  );
}

// 侧 2（兜底）：真实 ~/.aidog 新增文件 = 硬编码真实路径泄漏；排除常驻侧新建噪声。
// settings.<group>.json（2026-11-20 22:20 实测：用户新建分组 p1-auto 时常驻应用落盘新文件，
// 恰落在 cargo 窗口内）。硬编码路径泄漏若写 settings 文件同样被排除——接受，fakeHome 侧
// 已兜住全部 env HOME 泄漏，硬编码路径泄漏本身属罕见 bug。
const NOISE = [/^logs\/aidog\./, /^subagent-cap\//, /^settings\./];
const realLeak = walkFiles(REAL_AIDOG).filter(
  (f) => !before.has(f) && !NOISE.some((re) => re.test(f)),
);
if (realLeak.length > 0) {
  leaks.push(
    `真实 ~/.aidog 新增（硬编码路径泄漏嫌疑）：`,
    ...realLeak.map((f) => "  " + f),
  );
}

// 先清理自己的 tempdir（两个退出分支都要走；泄漏样本路径已打印进输出）。
fs.rmSync(fakeHome, { recursive: true, force: true });

if (leaks.length > 0) {
  console.log("\nFAIL: 满足泄漏判据，检测不通过：");
  for (const line of leaks) console.log(line);
  process.exit(1);
}

console.log(`\nOK: 双侧无泄漏，检测通过；cargo test EXIT=${cargoExit}`);
if (cargoExit !== 0) {
  console.log("cargo test Exit non-zero: tests themselves red; attribute separately from HOME leak detection.");
  process.exit(2);
}
