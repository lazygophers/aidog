// node --test scripts/json-dup-keys.test.mjs（yarn test:i18n-dup）
import test from "node:test";
import assert from "node:assert/strict";
import { findDuplicateKeys } from "./json-dup-keys.mjs";

test("同对象重复键命中并给出行号", () => {
  const d = findDuplicateKeys('{\n  "a": 1,\n  "b": {"x": 1},\n  "a": 2\n}');
  assert.deepEqual(d, [{ key: "a", line: 4, firstLine: 2 }]);
});

test("嵌套对象内重复键命中", () => {
  const d = findDuplicateKeys('{"o": {"k": 1, "k": 2}}');
  assert.equal(d.length, 1);
  assert.equal(d[0].key, "k");
});

test("不同层级同名、字符串值含冒号逗号引号均不误报", () => {
  const ok = '{"group": {"a": "x, \\"group\\": y"}, "n": {"group": "z"}, "arr": [{"a":1},{"a":2}]}';
  assert.deepEqual(findDuplicateKeys(ok), []);
});
