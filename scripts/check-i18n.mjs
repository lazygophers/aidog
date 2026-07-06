#!/usr/bin/env node
/**
 * i18n 覆盖检查 (根治裸 key 的自动化防线)。
 *
 * 检查项:
 *   A. t() 静态 key 覆盖 — 扫 src 下所有 .ts/.tsx 的 t("literal") 字面量,
 *      每个 key 必须在所有 LOCALES 存在 (任一缺 → 裸 key 显示)。
 *   B. locale 间对齐 — 8 locale key 集合必须等于并集 (任一 locale 缺并集 key → 该 locale 切换时 fallback)。
 *   C. 动态模板清单 — 输出所有 t(`prefix${var}`) 模板 + 基准 locale(en-US) 匹配 key 数,
 *      供人工审计变量取值是否全覆盖 (动态模板无法全自动展开)。
 *   D. labelKey/group 属性字面量覆盖 — t(变量) 路径 (t(item.labelKey)/t(g.key)) 数据源,
 *      扫 NAV_ITEMS/sections 配置的 labelKey/group 字面量, 每 key 必须 8 locale 存在。
 *      (A/B/C 都要求 t( 后跟引号/反引号, t(变量) 完全漏扫 → D 堵此盲区)。
 *
 * 用法: node scripts/check-i18n.mjs
 * 退出码: 0 = 零缺失; 非 0 = 有缺失 (check 阶段 fail)。
 *
 * 规约见 .trellis/spec/frontend/conventions.md i18n 章节。
 */
import { readFileSync } from 'fs';
import { execFileSync } from 'child_process';

const LOCALES = ['zh-CN', 'en-US', 'ar-SA', 'fr-FR', 'de-DE', 'ru-RU', 'ja-JP', 'es-ES'];
const BASE = 'en-US'; // 动态模板覆盖度基准

function loadLocale(l) {
  const obj = JSON.parse(readFileSync(`src/locales/${l}.json`, 'utf8'));
  return new Set(Object.keys(obj));
}

const localeSet = Object.fromEntries(LOCALES.map(l => [l, loadLocale(l)]));
const union = new Set();
for (const l of LOCALES) for (const k of localeSet[l]) union.add(k);

// ── 扫描 src 所有 t() 调用 ──────────────────────────────
// execFileSync 无 shell，参数数组直传 find，避免 shell 注入面。
const files = execFileSync('find', ['src', '-type', 'f', '(', '-name', '*.ts', '-o', '-name', '*.tsx', ')'], { encoding: 'utf8' }).trim().split('\n');
const staticRe = /\bt\(\s*['"]([^'"]+)['"]/g;          // t("lit") / t('lit')
const dynRe = /\bt\(\s*`([^`]+)`/g;                    // t(`tpl`)

const staticKeys = new Map();   // key -> Set<file>
const dynTemplates = new Map(); // template -> Set<file>
for (const f of files) {
  const src = readFileSync(f, 'utf8');
  let m;
  while ((m = staticRe.exec(src))) {
    const k = m[1];
    if (!staticKeys.has(k)) staticKeys.set(k, new Set());
    staticKeys.get(k).add(f);
  }
  while ((m = dynRe.exec(src))) {
    const tpl = m[1];
    if (!tpl.includes('${')) { // 无插值 = 静态, 归 staticKeys
      if (!staticKeys.has(tpl)) staticKeys.set(tpl, new Set());
      staticKeys.get(tpl).add(f);
      continue;
    }
    if (!dynTemplates.has(tpl)) dynTemplates.set(tpl, new Set());
    dynTemplates.get(tpl).add(f);
  }
}

// ── A. 静态 key 覆盖检查 ──────────────────────────────
const staticMissing = []; // {key, missingLocales, files}
for (const [k, fileSet] of staticKeys) {
  const miss = LOCALES.filter(l => !localeSet[l].has(k));
  if (miss.length > 0) staticMissing.push({ key: k, miss, files: [...fileSet] });
}

// ── B. locale 间对齐检查 ──────────────────────────────
const alignMissing = []; // {locale, keys:[...]}
for (const l of LOCALES) {
  const miss = [...union].filter(k => !localeSet[l].has(k));
  if (miss.length > 0) alignMissing.push({ locale: l, count: miss.length, sample: miss.slice(0, 8) });
}

// ── E. platform-presets.json protocol name/desc 8 locale 完整性 ──────
// 每 protocol 必须 8 locale 都有非空 name+desc（缺失 → 切语言时协议选择器裸 key）
const DEFAULTS_LOCALES = ['en-US', 'zh-Hans', 'ar-SA', 'fr-FR', 'de-DE', 'ru-RU', 'ja-JP', 'es-ES'];
const presetsRaw = JSON.parse(readFileSync('src-tauri/defaults/platform-presets.json', 'utf8'));
const presetsProtocols = presetsRaw.protocols || {};
const protocolMissing = []; // {protocol, field, miss}
for (const [proto, entry] of Object.entries(presetsProtocols)) {
  for (const field of ['name', 'desc']) {
    const obj = entry?.[field] || {};
    const miss = DEFAULTS_LOCALES.filter(l => !obj[l] || !String(obj[l]).trim());
    if (miss.length > 0) protocolMissing.push({ protocol: proto, field, miss });
  }
}

// ── D. labelKey/group 属性字面量覆盖 (t(变量) 盲区) ──────
// t(item.labelKey)/t(g.key)/t(section.labelKey) 数据源 = NAV_ITEMS/sections 配置。
// staticRe/dynRe 都要求 t( 后跟引号/反引号, t(变量) 完全漏扫 → 扫属性字面量堵盲区。
const propRe = /\b(?:labelKey|group)\s*:\s*["']([a-zA-Z][a-zA-Z0-9_]*(?:\.[a-zA-Z0-9_]+)+)["']/g;
const propKeys = new Map(); // key -> Set<file>
for (const f of files) {
  const src = readFileSync(f, 'utf8');
  let m;
  while ((m = propRe.exec(src))) {
    const k = m[1];
    // 排除品牌/非 i18n: 首字符大写 (APIKEY.FUN / CTok.ai 等 Platforms 预设 label)
    if (/^[A-Z]/.test(k)) continue;
    if (!propKeys.has(k)) propKeys.set(k, new Set());
    propKeys.get(k).add(f);
  }
}
const propMissing = []; // {key, miss, files}
for (const [k, fileSet] of propKeys) {
  const miss = LOCALES.filter(l => !localeSet[l].has(k));
  if (miss.length > 0) propMissing.push({ key: k, miss, files: [...fileSet] });
}

// ── 输出 ──────────────────────────────────────────────
let problems = 0;
const log = s => console.log(s);

log(`\n# i18n 检查报告`);
log(`扫描 ${files.length} 文件 | ${staticKeys.size} 静态 key | ${dynTemplates.size} 动态模板 | ${propKeys.size} 属性字面量 key | ${union.size} locale 并集 key\n`);
log(`platform-presets.json: ${Object.keys(presetsProtocols).length} protocols × name/desc × ${DEFAULTS_LOCALES.length} locale 校验\n`);

// A
log(`## A. t() 静态 key 缺失: ${staticMissing.length}`);
for (const { key, miss, files } of staticMissing.sort((a, b) => a.key.localeCompare(b.key))) {
  const scope = miss.length === LOCALES.length ? '全缺' : `[${miss.join(',')}]`;
  log(`  🔴 ${key} 缺${scope}  ← ${files[0]}${files.length > 1 ? ` +${files.length - 1}` : ''}`);
  problems++;
}

// B
log(`\n## B. locale 间不对齐: ${alignMissing.length} locale 缺 key`);
for (const { locale, count, sample } of alignMissing.sort((a, b) => b.count - a.count)) {
  log(`  🟡 ${locale}: 缺 ${count} key (样例: ${sample.join(', ')}${count > sample.length ? '...' : ''})`);
  problems++;
}

// C
log(`\n## C. 动态模板清单 (人工审计变量取值覆盖): ${dynTemplates.size}`);
for (const [tpl, fileSet] of [...dynTemplates].sort()) {
  const prefix = tpl.split('${')[0];
  const covered = [...localeSet[BASE]].filter(k => k.startsWith(prefix)).length;
  log(`  t(\`${tpl}\`) → ${BASE} 匹配 ${covered} key  ← ${[...fileSet][0]}`);
}

// D
log(`\n## D. labelKey/group 属性字面量缺失 (t(变量) 盲区): ${propMissing.length}`);
for (const { key, miss, files } of propMissing.sort((a, b) => a.key.localeCompare(b.key))) {
  const scope = miss.length === LOCALES.length ? '全缺' : `[${miss.join(',')}]`;
  log(`  🔴 ${key} 缺${scope}  ← ${files[0]}${files.length > 1 ? ` +${files.length - 1}` : ''}`);
  problems++;
}

// E
log(`\n## E. platform-presets protocol name/desc locale 缺失: ${protocolMissing.length}`);
for (const { protocol, field, miss } of protocolMissing) {
  log(`  🔴 ${protocol}.${field} 缺 [${miss.join(',')}]`);
  problems++;
}

log(`\n${problems === 0 ? '✅ 零缺失' : `❌ ${problems} 处缺失 (见上)`}\n`);
process.exit(problems === 0 ? 0 : 1);
