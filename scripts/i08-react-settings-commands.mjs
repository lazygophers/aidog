// 解析出「设置这一批页面在 React 版实际调用的全部命令名」，写成清单给 Flutter 侧
// 的零差集测试当真值源（`flutter/test/settings/command_coverage_test.dart`）。
//
// 为什么要脚本而不是手列：这批是 13 个子页 + 30 多个组件，命令散在
// `src/services/api/` 的命名空间对象里（`platformApi.list()` 这种），手列必漏。
// 漏一条 = Flutter 少一个功能而测试还是绿的，正是本票要防的东西。
//
// 解析口径见 `scripts/lib/react-commands.mjs`（票 I07 把这段解析抽出去共用，
// 本文件只剩一张文件清单；输出与抽出前逐字节一致）。
//
// 用法：
//   node scripts/i08-react-settings-commands.mjs           # 写清单
//   node scripts/i08-react-settings-commands.mjs --check    # 只比对，不一致 exit 1
//   node scripts/i08-react-settings-commands.mjs --report   # 打印逐文件明细

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { dirFiles, emit, scanFiles } from './lib/react-commands.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT = path.join(root, 'flutter', 'test', 'settings', 'react_settings_commands.txt');

/** 本票范围：`.scratch/flutter-frontend/impl/I08-pages-c.md` 点名的那批。 */
const FILES = [
  'src/pages/Settings.tsx',
  'src/pages/AppSettings.tsx',
  'src/pages/CodexSettings.tsx',
  'src/pages/PiSettings.tsx',
  'src/pages/TrayConfigTab.tsx',
  'src/pages/PopoverConfigTab.tsx',
  ...dirFiles(root, 'src/components/settings', /\.tsx$/),
  // 票 I19b：`editors/` 下的字段渲染器与 statusline 面板也直接 invoke
  // （`_shared.tsx` 的 fs_autocomplete、`StatusLineSection/useStatusLinePanel.ts`
  // 的 preview_statusline_script）。不扫这两层，C5 / C6 的缺口测试就照不出来。
  // 票 I19b：`editors/` 下的字段渲染器与 statusline 面板也直接 invoke
  // （`_shared.tsx` 的 fs_autocomplete、`StatusLineSection/useStatusLinePanel.ts`
  // 的 preview_statusline_script）。`dirFiles` 现在递归，子目录不必逐个列。
  ...dirFiles(root, 'src/components/settings/editors'),
  ...dirFiles(root, 'src/pages/AppSettings'),
  ...dirFiles(root, 'src/pages/PopoverConfigTab'),
  ...dirFiles(root, 'src/components/settings/ImportExport'),
];

const { perFile, union, notes } = scanFiles(root, FILES);

emit({
  root,
  out: OUT,
  header:
    '# React 版「设置」13 子页实际调用的命令' +
    '（由 scripts/i08-react-settings-commands.mjs 生成，禁手改）',
  perFile,
  union,
  notes,
  argv: process.argv,
});
