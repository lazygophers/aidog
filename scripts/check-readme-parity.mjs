import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(root, "README.md");
const locales = ["en", "ar", "de", "es", "fr", "ja", "ru"];
const files = [sourcePath, ...locales.map((locale) => resolve(root, `README.${locale}.md`))];
const source = readFileSync(sourcePath, "utf8");
const expectedHeadings = headings(source);
const expectedLanguageLinks = ["README.md", ...locales.map((locale) => `README.${locale}.md`)];
let failed = false;

for (const path of files.slice(1)) {
  const name = path.slice(root.length + 1);
  const errors = [];
  if (!existsSync(path)) {
    console.log(`${name}: missing`);
    failed = true;
    continue;
  }

  const text = readFileSync(path, "utf8");
  const actualHeadings = headings(text);
  if (actualHeadings.length !== expectedHeadings.length) {
    errors.push(`## 标题数量 ${actualHeadings.length}，预期 ${expectedHeadings.length}`);
  }
  const unmatched = actualHeadings.map((_, i) => i + 1).filter((index) => index > expectedHeadings.length);
  if (unmatched.length || actualHeadings.some((_, i) => i < expectedHeadings.length && !actualHeadings[i])) {
    errors.push("## 标题顺序或位置不匹配");
  }
  const languageLine = text.split(/\r?\n/).find((line) => /README(?:\.[a-z]{2})?\.md/.test(line)) ?? "";
  const currentLocale = name.match(/^README\.([a-z]{2})\.md$/)?.[1];
  for (const target of expectedLanguageLinks) {
    if (target === `README.${currentLocale}.md`) continue;
    if (!languageLine.includes(`(${target})`)) errors.push(`语言切换行缺少 ${target}`);
  }
  if (!languageLine) errors.push("缺少语言切换行");

  for (const [, target] of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    // `/...` 是 docs 站点绝对路径（8 个 README 既有约定），不做文件存在性检查
    if (/^(?:[a-z]+:|\/|#|\/\/)/i.test(target)) continue;
    const file = target.split(/[?#]/, 1)[0];
    if (file && !existsSync(resolve(dirname(path), file))) errors.push(`相对链接目标不存在: ${target}`);
  }

  const fences = [...text.matchAll(/^\s*(`{3,}|~{3,})/gm)].map((match) => match[1][0]);
  if (fences.length % 2) errors.push(`代码围栏未配对: ${fences.length} 个`);
  if (errors.length) {
    failed = true;
    console.log(`${name}: DIFF\n  ${errors.join("\n  ")}`);
  } else console.log(`${name}: OK`);
}
process.exitCode = failed ? 1 : 0;

function headings(text) {
  return [...text.matchAll(/^## (.+)$/gm)].map((match) => match[1]);
}
