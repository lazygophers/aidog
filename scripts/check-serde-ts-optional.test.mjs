import test from "node:test";
import assert from "node:assert/strict";
import { checkSource } from "./check-serde-ts-optional.mjs";

const wrap = (field, extra = "") =>
  `#[derive(Serialize, TS)]\n${extra}pub struct S {\n${field}\n}\n`;

test("坏样本：skip_serializing_if 无 ts(optional) → 报错并带字段名", () => {
  const e = checkSource(
    wrap('    #[serde(default, skip_serializing_if = "Vec::is_empty")]\n    pub model_chain: Vec<String>,'),
    "x.rs",
  );
  assert.equal(e.length, 1);
  assert.match(e[0], /model_chain/);
});

test("坏样本：只有 ts(type=..) 没有 optional → 报错", () => {
  const e = checkSource(
    wrap('    #[serde(skip_serializing_if = "Option::is_none")]\n    #[ts(type = "number")]\n    pub a: Option<i64>,'),
    "x.rs",
  );
  assert.equal(e.length, 1);
});

test("好样本：ts(optional) / ts(optional, type) / ts(skip) / 结构体级 optional_fields / 非 TS 结构体", () => {
  const sk = '    #[serde(skip_serializing_if = "Option::is_none")]\n';
  assert.deepEqual(checkSource(wrap(`${sk}    #[ts(optional)]\n    pub a: Option<i64>,`), "x.rs"), []);
  assert.deepEqual(checkSource(wrap(`${sk}    #[ts(optional, type = "number")]\n    pub a: Option<i64>,`), "x.rs"), []);
  assert.deepEqual(checkSource(wrap(`${sk}    #[ts(skip)]\n    pub a: Option<i64>,`), "x.rs"), []);
  assert.deepEqual(checkSource(wrap(`${sk}    pub a: Option<i64>,`, "#[ts(optional_fields)]\n"), "x.rs"), []);
  assert.deepEqual(
    checkSource(`#[derive(Serialize)]\npub struct N {\n${sk}    pub a: Option<i64>,\n}\n`, "x.rs"),
    [],
  );
});
