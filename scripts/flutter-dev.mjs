#!/usr/bin/env node
// `make run-flutter` 的驱动：起 `flutter run`，并在 lib/ 下的 .dart 存盘时**自动**热重载。
//
// 为什么要这个脚本：`flutter run` 自己不监听文件，它等你在终端里按 `r`。官方给的自动化口子是
// `--pid-file`——帮助原文：「You can send SIGUSR1 to trigger a hot reload and SIGUSR2 to
// trigger a hot restart.」所以这里只做两件事：看着文件、发信号。
//
// 终端仍然是 `flutter run` 的（stdio 直通），`r` / `R` / `q` 照常能按。

import { spawn } from 'node:child_process';
import { readFileSync, watch } from 'node:fs';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const device = process.argv[2] ?? 'macos';
const repoRoot = new URL('..', import.meta.url).pathname;
const flutterDir = join(repoRoot, 'flutter');
const runDir = mkdtempSync(join(tmpdir(), 'aidog-flutter-dev-'));
const pidFile = join(runDir, 'flutter.pid');

const child = spawn(
  'flutter',
  ['run', '-d', device, '--pid-file', pidFile],
  { cwd: flutterDir, stdio: 'inherit' },
);

// 防抖：编辑器存盘常常一次触发多个事件（写临时文件 + rename），不合并就会连发好几次。
let timer = null;
let pendingRestart = false;
const DEBOUNCE_MS = 150;

// SIGUSR1 = 热重载（保留应用状态，毫秒级）；SIGUSR2 = 热重启（丢状态，秒级）。
// 资产（8 语言文案、schema JSON）不在 Dart 代码里，热重载读不到新内容，必须热重启。
function signal(kind) {
  // pid 文件是 flutter 挂好信号处理器之后才写的，所以每次都现读——启动瞬间存盘时它可能还不存在。
  let pid;
  try {
    pid = Number(readFileSync(pidFile, 'utf8').trim());
  } catch {
    return; // 还没起来，这次存盘就不重载了，下次存盘会补上
  }
  const sig = kind === 'restart' ? 'SIGUSR2' : 'SIGUSR1';
  const what = kind === 'restart' ? '热重启（资产变了）' : '热重载';
  try {
    process.kill(pid, sig);
    process.stderr.write(`\x1b[36m[flutter-dev] 检测到改动，已触发${what}\x1b[0m\n`);
  } catch (e) {
    process.stderr.write(`[flutter-dev] ${what}信号发不出去: ${e.message}\n`);
  }
}

function schedule(kind) {
  if (kind === 'restart') pendingRestart = true;
  clearTimeout(timer);
  timer = setTimeout(() => {
    signal(pendingRestart ? 'restart' : 'reload');
    pendingRestart = false;
  }, DEBOUNCE_MS);
}

// 三个监听点，少一个就会出现「改了没反应」：
//   flutter/lib      —— 应用代码
//   design/tokens/lib —— 设计 token 的 Dart 包（path 依赖，改字号/行高/配色走这里）
//   flutter/assets   —— schema JSON，属资产，要热重启
//   aidog_i18n/locales —— 8 语言文案，以 path 包 `aidog_i18n_locales` 当资产挂进来，同样要热重启
const watchers = [
  [join(flutterDir, 'lib'), 'reload'],
  [join(repoRoot, 'design/tokens/lib'), 'reload'],
  [join(flutterDir, 'assets'), 'restart'],
  [join(repoRoot, 'src-tauri/crates/aidog_i18n/locales'), 'restart'],
].map(([dir, kind]) =>
  watch(dir, { recursive: true }, (_event, filename) => {
    if (kind === 'reload' && !filename?.endsWith('.dart')) return;
    schedule(kind);
  }),
);

function cleanup() {
  for (const w of watchers) w.close();
  rmSync(runDir, { recursive: true, force: true });
}

child.on('exit', (code, signal) => {
  cleanup();
  process.exit(signal ? 1 : (code ?? 0));
});

// Ctrl-C 交给 flutter 自己处理（它要清理设备连接），这里只负责别把临时目录留下。
for (const sig of ['SIGINT', 'SIGTERM']) {
  process.on(sig, () => {
    child.kill(sig);
  });
}
