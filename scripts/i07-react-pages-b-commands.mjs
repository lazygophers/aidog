// 解析「票 I07 这一批页面（平台 / 分组 / 请求日志 / 日志）在 React 版实际调用的全部命令名」，
// 写成清单给 Flutter 侧的零差集测试当真值源（`flutter/test/pages/react_pages_b_commands.txt`）。
//
// 口径与票 I08 完全一致（共用 `scripts/lib/react-commands.mjs` 的解析器），差别只在文件清单。
// 为什么要脚本而不是手列：这批的命令散在 `src/services/api/` 的命名空间对象里
// （`platformApi.list()` 这种），且经 usePlatformCards / usePlatformQuota 等 hook 中转，
// 手列必漏。漏一条 = Flutter 少一个功能而测试还是绿的，正是本票要防的东西。
//
// 用法：
//   node scripts/i07-react-pages-b-commands.mjs            # 写清单
//   node scripts/i07-react-pages-b-commands.mjs --check     # 只比对，不一致 exit 1
//   node scripts/i07-react-pages-b-commands.mjs --report    # 打印逐文件明细

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { dirFiles, emit, scanFiles } from './lib/react-commands.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT = path.join(root, 'flutter', 'test', 'pages', 'react_pages_b_commands.txt');

/**
 * 本票范围：`.scratch/flutter-frontend/impl/I07-pages-b.md` 点名的那批页面，
 * 加上它们**独占**渲染的共享组件 —— 命令是经这些组件发出去的，不算进来就漏。
 *
 * 收哪些共享件的判据：只被本批页面渲染。`src/components/platforms/`（平台卡 +
 * 四个批量弹窗 + 智能粘贴）与 `src/domains/{groups,platforms}` 全被 Platforms /
 * Groups 两页消费，故整目录纳入；`src/components/settings/` 是票 I08 的，不纳入。
 */
const FILES = [
  'src/pages/Platforms.tsx',
  'src/pages/Groups.tsx',
  'src/pages/RequestLog.tsx',
  'src/pages/Logs.tsx',
  ...dirFiles(root, 'src/pages/platforms'),
  ...dirFiles(root, 'src/pages/Groups'),
  ...dirFiles(root, 'src/pages/Logs'),
  ...dirFiles(root, 'src/components/platforms'),
  ...dirFiles(root, 'src/domains/groups'),
  ...dirFiles(root, 'src/domains/platforms'),
];

const { perFile, union, notes } = scanFiles(root, FILES);

emit({
  root,
  out: OUT,
  header:
    '# React 版「平台 / 分组 / 请求日志 / 日志」四页实际调用的命令' +
    '（由 scripts/i07-react-pages-b-commands.mjs 生成，禁手改）',
  perFile,
  union,
  notes,
  argv: process.argv,
});
