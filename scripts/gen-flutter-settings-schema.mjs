// 把三份设置 schema（claude / codex / pi）从 TS 导出成 JSON 资产，供 Flutter 侧读取。
//
// 为什么生成而不是手抄：这三份 schema 是**纯数据**（465 个字段定义、80 KB），
// 手抄一份 Dart 就是第二个真值源，字段一改就静默漂移。生成物进仓库、
// `--check` 模式做零差集比对（进 make lint），React 改了 schema 而没重新生成 → 当场红。
//
// 用法：
//   node scripts/gen-flutter-settings-schema.mjs          # 写入
//   node scripts/gen-flutter-settings-schema.mjs --check   # 只比对，不一致 exit 1
//
// RECOMMENDED_CONFIG 不在这里生成：它 = src-tauri/defaults/settings.json + 运行时
// 语言检测，Flutter 侧直接读同一个 JSON 再覆盖 language，与 React 同一真值源。

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as esbuild from 'esbuild';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT = path.join(root, 'flutter', 'assets', 'settings_schema.json');
// Flutter 的 asset 不能指向包目录之外，所以后端内置默认要复制进来。
// 复制而不是手抄：它仍由 --check 盯着，源文件一改不重新生成就红。
const DEFAULTS_SRC = path.join(root, 'src-tauri', 'defaults', 'settings.json');
const DEFAULTS_OUT = path.join(root, 'flutter', 'assets', 'claude_default_settings.json');

const ENTRY = `
export { SECTIONS, ENV_VAR_DEFS, ENV_VAR_GROUP_ORDER, ENV_VAR_GROUP_LABEL_KEYS, LANGUAGE_GROUPS }
  from ${JSON.stringify(path.join(root, 'src/services/claude-settings-schema.ts'))};
export { CODEX_SECTIONS, CODEX_RECOMMENDED_CONFIG }
  from ${JSON.stringify(path.join(root, 'src/services/codex-settings-schema.ts'))};
export { PI_SECTIONS, PI_RECOMMENDED_CONFIG }
  from ${JSON.stringify(path.join(root, 'src/services/pi-settings-schema.ts'))};
`;

const built = await esbuild.build({
  stdin: { contents: ENTRY, resolveDir: root, loader: 'ts' },
  bundle: true,
  format: 'esm',
  platform: 'node',
  write: false,
  loader: { '.json': 'json' },
});

const mod = await import(
  'data:text/javascript;base64,' + Buffer.from(built.outputFiles[0].text).toString('base64')
);

const payload = {
  claude: {
    sections: mod.SECTIONS,
    envVarDefs: mod.ENV_VAR_DEFS,
    envVarGroupOrder: mod.ENV_VAR_GROUP_ORDER,
    envVarGroupLabelKeys: mod.ENV_VAR_GROUP_LABEL_KEYS,
    languageGroups: mod.LANGUAGE_GROUPS,
  },
  codex: { sections: mod.CODEX_SECTIONS, recommended: mod.CODEX_RECOMMENDED_CONFIG },
  pi: { sections: mod.PI_SECTIONS, recommended: mod.PI_RECOMMENDED_CONFIG },
};

const text = JSON.stringify(payload, null, 2) + '\n';
const defaultsText = fs.readFileSync(DEFAULTS_SRC, 'utf8');

if (process.argv.includes('--check')) {
  const stale = [
    [OUT, text],
    [DEFAULTS_OUT, defaultsText],
  ].filter(([p, want]) => (fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : '') !== want);
  if (stale.length > 0) {
    console.error(
      `[settings-schema] 与 TS 真值源不一致：${stale.map(([p]) => path.relative(root, p)).join(', ')}` +
        `\n  跑 \`node scripts/gen-flutter-settings-schema.mjs\` 重新生成。`,
    );
    process.exit(1);
  }
  console.log('[settings-schema] ok');
} else {
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, text);
  fs.writeFileSync(DEFAULTS_OUT, defaultsText);
  const n = payload.claude.sections.reduce((a, s) => a + s.fields.length, 0);
  console.log(
    `[settings-schema] ${path.relative(root, OUT)}: claude ${payload.claude.sections.length} 节 / ${n} 字段 / ${payload.claude.envVarDefs.length} 环境变量，` +
      `codex ${payload.codex.sections.length} 节，pi ${payload.pi.sections.length} 节`,
  );
}
