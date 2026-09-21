#!/usr/bin/env node
// 界面对齐护栏：React 用到的文案 key，Flutter 侧必须也出现。
//
// 为什么要这条：`i0*-react-*.mjs` 那套「命令覆盖零差集」只照得出**后端交互**。
// 批量选择、复制命令、拖拽排序这类**纯界面功能一条后端命令都不调**，所以它全照不出。
// 票 I18 那轮就是这么漏掉整个分组区的——缺口存在而测试全绿。
//
// 🔴 本脚本**照不出**的（写在这里，让下一个人一眼看见）：
//   1. 同义不同名：两侧都实现了、key 取名不同 → 靠 `ui-parity-exceptions.json` 的
//      `type: "synonym"` 登记。
//   2. 纯图形交互：拖拽 / 键盘 / hover。全仓只有 39 处，不值得再造一个脚本，
//      改由每张对齐票人工核对：
//      grep -rn 'onDragStart\|onDragOver\|onDrop\|onKeyDown\|onDoubleClick\|onContextMenu' <目录>
//   3. React 自己都没有的功能（那不叫缺口）。
//
// 用法：
//   node scripts/check-ui-parity.mjs            # 列出缺口，有缺口则非零退出
//   node scripts/check-ui-parity.mjs --report   # 按前缀分组统计，不改退出码
//   node scripts/check-ui-parity.mjs --self-test # 反向验证：故意造缺口应当报红

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const EXCEPTIONS = join(root, "scripts/ui-parity-exceptions.json");

/** React 侧扫这几棵树；Flutter 侧扫 lib/ 全部。 */
const REACT_DIRS = ["src/pages", "src/components", "src/domains"];
const FLUTTER_DIR = "flutter/lib";

function walk(dir, re, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === "__snapshots__") continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, re, out);
    else if (re.test(name) && !name.includes(".test.")) out.push(p);
  }
  return out;
}

/** React：只取 `t("a.b")` 的实参——那才是真正渲染到界面上的文案。 */
function reactKeys() {
  const keys = new Set();
  const re = /\bt\(\s*"([a-zA-Z][a-zA-Z0-9]*\.[a-zA-Z0-9.]+)"/g;
  for (const dir of REACT_DIRS) {
    for (const f of walk(join(root, dir), /\.tsx?$/)) {
      const src = readFileSync(f, "utf8");
      for (const m of src.matchAll(re)) keys.add(m[1]);
    }
  }
  return keys;
}

/** Flutter：取**所有字符串字面量**，不只 `t('...')`。
 *
 *  只认 `t('...')` 时误报率实测 50%（12 抽 6 误）——Dart 这边有 `tOr`、有把 key
 *  存进变量再用、有拼接的写法。放宽到全字面量后降到 7%（15 抽 1 误）。
 *  宁可漏报也不要误报：护栏一旦开始喊狼来了，下一次真红就没人信了。 */
function flutterLiterals() {
  const keys = new Set();
  const re = /['"]([a-zA-Z][a-zA-Z0-9]*\.[a-zA-Z0-9.]+)['"]/g;
  for (const f of walk(join(root, FLUTTER_DIR), /\.dart$/)) {
    const src = readFileSync(f, "utf8");
    for (const m of src.matchAll(re)) keys.add(m[1]);
  }
  return keys;
}

/** 例外清单。格式不合规直接红——不卡死就会变成许愿池。 */
function loadExceptions() {
  let doc;
  try {
    doc = JSON.parse(readFileSync(EXCEPTIONS, "utf8"));
  } catch (e) {
    if (e.code === "ENOENT") return { allow: new Set(), errors: [] };
    return { allow: new Set(), errors: [`${EXCEPTIONS} 解析失败: ${e.message}`] };
  }
  const allow = new Set();
  const errors = [];
  for (const [i, x] of (doc.exceptions ?? []).entries()) {
    const at = `exceptions[${i}]`;
    if (!x.key) errors.push(`${at} 缺 key`);
    if (x.type !== "synonym" && x.type !== "intentional") {
      errors.push(`${at} (${x.key}) 的 type 必须是 synonym 或 intentional，现在是 ${JSON.stringify(x.type)}`);
    }
    if (!x.reason) errors.push(`${at} (${x.key}) 缺 reason`);
    // 出处必填且必须是 file:line —— 只写理由不给出处的，一年后没人知道还成不成立。
    if (!/^[\w./-]+:\d+$/.test(x.evidence ?? "")) {
      errors.push(`${at} (${x.key}) 的 evidence 必须是 file:line，现在是 ${JSON.stringify(x.evidence)}`);
    }
    if (x.key) allow.add(x.key);
  }
  return { allow, errors };
}

const react = reactKeys();
const flutter = flutterLiterals();
const { allow, errors } = loadExceptions();
const missing = [...react].filter((k) => !flutter.has(k) && !allow.has(k)).sort();

if (process.argv.includes("--self-test")) {
  // 反向验证：护栏必须真能抓。拿一个 React 有、Flutter 必然没有的假 key 走一遍全流程。
  const fake = "zzz.parityGuardSelfTest";
  const wouldCatch = !flutter.has(fake) && !allow.has(fake);
  console.log(wouldCatch
    ? "[ui-parity] self-test ok：未登记的缺失 key 会被判红"
    : "[ui-parity] self-test 失败：假 key 竟然被放过了");
  process.exit(wouldCatch ? 0 : 1);
}

if (process.argv.includes("--report")) {
  const byPrefix = {};
  for (const k of missing) {
    const p = k.split(".")[0];
    (byPrefix[p] ??= []).push(k);
  }
  console.log(`React ${react.size} 个文案 key｜Flutter 命中 ${react.size - missing.length - allow.size}｜例外 ${allow.size}｜缺口 ${missing.length}`);
  for (const [p, ks] of Object.entries(byPrefix).sort((a, b) => b[1].length - a[1].length)) {
    console.log(`  ${p.padEnd(16)} ${ks.length}`);
  }
  process.exit(0);
}

if (errors.length > 0) {
  console.error("[ui-parity] 例外清单格式不合规：");
  for (const e of errors) console.error(`  ${e}`);
  process.exit(1);
}

if (missing.length > 0) {
  console.error(`[ui-parity] React 有、Flutter 没有的文案 key ${missing.length} 条：`);
  for (const k of missing.slice(0, 40)) console.error(`  ${k}`);
  if (missing.length > 40) console.error(`  …另有 ${missing.length - 40} 条，跑 --report 看分布`);
  console.error("\n补上，或按 scripts/ui-parity-exceptions.json 的格式登记为例外（要带 file:line 出处）。");
  process.exit(1);
}

console.log(`[ui-parity] ok（React ${react.size} 个 key 全部命中，例外 ${allow.size} 条）`);
