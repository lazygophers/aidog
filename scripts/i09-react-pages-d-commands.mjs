// 解析「票 I09 这一批页面（技能 / MCP / 关于 / 通知 / 模型信息）在 React 版实际调用的
// 全部命令名」，写成清单给 Flutter 侧的零差集测试当真值源
// （`flutter/test/pages/react_pages_d_commands.txt`）。
//
// 口径与票 I07 / I08 完全一致（共用 `scripts/lib/react-commands.mjs` 的解析器），
// 差别只在文件清单。命名空间对象只算本文件真访问过的成员，所以 import 一个
// `skillsApi` 不会把它全部方法都算进来。
//
// 用法：
//   node scripts/i09-react-pages-d-commands.mjs            # 写清单
//   node scripts/i09-react-pages-d-commands.mjs --check     # 只比对，不一致 exit 1
//   node scripts/i09-react-pages-d-commands.mjs --report    # 打印逐文件明细

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { dirFiles, emit, scanFiles } from './lib/react-commands.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT = path.join(root, 'flutter', 'test', 'pages', 'react_pages_d_commands.txt');

/**
 * 本票范围：`.scratch/flutter-frontend/impl/I09-pages-d.md` 点名的那批页面，
 * 加上它们**独占**渲染的子目录（`src/pages/Skills/`、`src/pages/Mcp/`、
 * `src/pages/ModelInfo/`）—— 命令是经这些文件发出去的，不算进来就漏。
 */
const FILES = [
  'src/pages/Skills.tsx',
  'src/pages/SkillDetailView.tsx',
  'src/pages/SkillInstallView.tsx',
  'src/pages/Mcp.tsx',
  'src/pages/About.tsx',
  'src/pages/Notifications.tsx',
  'src/pages/ModelTestPanel.tsx',
  ...dirFiles(root, 'src/pages/Skills'),
  ...dirFiles(root, 'src/pages/Mcp'),
  ...dirFiles(root, 'src/pages/ModelInfo'),
];

const { perFile, union, notes } = scanFiles(root, FILES);

emit({
  root,
  out: OUT,
  header:
    '# React 版「技能 / MCP / 关于 / 通知 / 模型信息」这批页面实际调用的命令' +
    '（由 scripts/i09-react-pages-d-commands.mjs 生成，禁手改）',
  perFile,
  union,
  notes,
  argv: process.argv,
});
