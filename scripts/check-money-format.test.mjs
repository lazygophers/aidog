// node --test scripts/check-money-format.test.mjs（yarn test:money）
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { scan } from "./check-money-format.mjs";

const root = mkdtempSync(join(tmpdir(), "money-"));
mkdirSync(join(root, "src"));
const put = (name, body) => {
  const p = join(root, "src", name);
  writeFileSync(p, body);
  return p;
};

test("违规写法全部命中", () => {
  const bad = [
    'const a = "$" + formatCost(n);',
    "const a = '$'+x;",
    "const a = x + \"$\";",
    "const a = `$${x}`;",
    'const a = s.replace("$", "¥");',
    'const a = sym ?? "$";',
    'const a = usd ? "$" : cur;',
  ];
  for (const line of bad) {
    assert.equal(scan([put("bad.ts", line)], root, []).length, 1, line);
  }
});

test("合规写法与注释行不命中", () => {
  const ok = 'const a = formatMoney(n);\n// 旧写法 "$" + x\n * "$" + y\nconst t = `${x}%`;';
  assert.equal(scan([put("ok.ts", ok)], root, []).length, 0);
});

test("白名单文件豁免、其余文件仍命中", () => {
  const allowed = put("allowed.ts", 'const a = "$" + x;');
  const other = put("other.ts", 'const a = "$" + x;');
  const hits = scan([allowed, other], root, [{ file: "src/allowed.ts", why: "test" }]);
  assert.deepEqual(hits.map((h) => h.file), ["src/other.ts"]);
});
