#!/usr/bin/env node
// 静默化 make lint 输出：成功一行，失败只打印失败段。完整日志已由 Makefile 落 .tmp/lint.log。
import { readFileSync } from 'node:fs';

const [logPath, rcArg] = process.argv.slice(2);
if (!logPath || rcArg === undefined) {
  console.error('用法: node scripts/lint-report.mjs <日志路径> <退出码>');
  process.exit(2);
}
const rc = Number(rcArg);
const lines = readFileSync(logPath, 'utf8')
  .split('\n')
  .map((l) => l.replace(/\x1b\[[0-9;]*m/g, ''));
// make 自身噪音：进/出目录回显与 `*** [Makefile] Error N`
const noise = /^make(\[\d+\])?:\s/;
const errIdx = lines.findIndex((l) => /^make(\[\d+\])?:\s.*Error \d+/.test(l));

if (rc === 0) {
  const n = lines.filter((l) => l.includes('▶')).length;
  console.log(
    n > 0
      ? `make lint: 全部通过（${n} 项检查，日志 ${logPath}）`
      : `make lint: 全部通过（日志 ${logPath}）`
  );
  process.exit(0);
}

if (errIdx === -1) {
  // 无 make 错误标记（如被信号杀掉）：兜底打日志尾部 30 行
  console.log(lines.filter((l) => !noise.test(l)).slice(-30).join('\n'));
  console.log(`完整日志: ${logPath}`);
  process.exit(rc);
}

// 失败段 = 失败点（首个 make Error 行）之前最近的 ▶ 段标题起，到 Error 行前
let start = 0;
for (let i = errIdx - 1; i >= 0; i--) {
  if (lines[i].includes('▶')) {
    start = i;
    break;
  }
}
for (let i = start; i < errIdx; i++) {
  if (!noise.test(lines[i])) console.log(lines[i]);
}
console.log(`完整日志: ${logPath}`);
process.exit(rc);
