// node --test scripts/check-no-cjk-literal.test.mjs（yarn test:no-cjk-literal）
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { collect, scan } from "./check-no-cjk-literal.mjs";

const root = mkdtempSync(join(tmpdir(), "cjk-"));
mkdirSync(join(root, "src/services/api/types/generated"), { recursive: true });
const put = (name, body) => {
  const p = join(root, "src", name);
  writeFileSync(p, body);
  return p;
};

test("字符串与模板字面量命中", () => {
  const f = put("str.ts", 'const a = "ok";\nconst b = "小时前";\nconst c = `${n} 分钟前`;\nconst d = \'かな\';');
  assert.deepEqual(scan([f], root, []).map((h) => h.line), [2, 3, 4]);
});

test("全角标点字面量命中", () => {
  const f = put("punct.ts", 'const a = "Name（x）";\nconst b = "key：";\nconst c = "a　b";\nconst d = "ok (x): y";');
  assert.deepEqual(scan([f], root, []).map((h) => h.line), [1, 2, 3]);
});

test("JSX 文本命中", () => {
  const f = put("jsx.tsx", "export const X = () => (\n  <div>\n    保存\n  </div>\n);");
  const hits = scan([f], root, []);
  assert.equal(hits.length, 1);
  assert.equal(hits[0].line, 3);
});

test("注释不命中", () => {
  const f = put("cmt.tsx", '// 中文注释\n/* 块注释 中文 */\n/** 文档 */\nconst a = "x"; // 行尾中文\nexport const Y = () => <div>{/* 中文 */}x</div>;');
  assert.equal(scan([f], root, []).length, 0);
});

test("测试文件与生成文件被排除", () => {
  put("a.test.ts", 'const a = "中文";');
  put("b.test.tsx", 'const a = "中文";');
  writeFileSync(join(root, "src/services/api/types/generated/T.ts"), 'const a = "中文";');
  const keep = put("keep.ts", 'const a = "中文";');
  const files = collect(join(root, "src"), root);
  assert.ok(files.includes(keep));
  assert.ok(!files.some((f) => /\.test\.tsx?$/.test(f) || f.includes("generated")));
});

test("白名单文件豁免、按行内容匹配仅豁免该行", () => {
  const all = put("allow.ts", 'const a = "中文";');
  const part = put("part.ts", 'const a = "中文";\nconst b = "简体中文";');
  const hits = scan([all, part], root, [
    { file: "src/allow.ts", reason: "test" },
    { file: "src/part.ts", match: "简体中文", reason: "test" },
  ]);
  assert.deepEqual(hits.map((h) => `${h.file}:${h.line}`), ["src/part.ts:1"]);
});

test("按内容匹配只豁免与 match 重叠的字面量，同行其它字面量仍报", () => {
  const f = put("same.ts", 'const x = { label: "前缀", placeholder: "缓存 " };\nconst y = `${p ?? "缓存 "}37%`;');
  const hits = scan([f], root, [{ file: "src/same.ts", match: '"缓存 "', reason: "test" }]);
  assert.deepEqual(hits.map((h) => `${h.line}:${h.text.includes("前缀")}`), ["1:true"]);
});
