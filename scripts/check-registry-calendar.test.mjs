import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import Ajv from "ajv";

const registry = new URL("../src-tauri/defaults/registry/", import.meta.url);
const read = (path) => JSON.parse(readFileSync(new URL(path, registry), "utf8"));

test("calendar reset modes are validated by the platform schema", () => {
  const validate = new Ajv({ allErrors: true, strict: false }).compile(read("schema/platform.schema.json"));
  const platform = read("platforms/qianfan_coding/platform.json");
  assert.equal(validate(platform), true, JSON.stringify(validate.errors));
  const budget = platform.plan_quotas[0].budgets.find((entry) => entry.reset_mode);
  assert.ok(budget);
  budget.reset_mode = "unknown_mode";
  assert.equal(validate(platform), false);
  assert.ok(validate.errors.some((error) => error.instancePath.endsWith("/reset_mode")));
});

test("legacy rolling and fixed budgets keep existing semantics", () => {
  const validate = new Ajv({ allErrors: true, strict: false }).compile(read("schema/platform.schema.json"));
  for (const kind of ["rolling", "fixed"]) {
    const platform = read("platforms/qianfan_coding/platform.json");
    const budget = platform.plan_quotas[0].budgets[1];
    delete budget.reset_mode;
    Object.assign(budget, { kind, unit: "token", window_hours: 2, window_unit: "month" });
    assert.equal(validate(platform), true, JSON.stringify(validate.errors));
  }
});

for (const [mode, property, invalid] of [
  ["calendar_week", "window_unit", "month"],
  ["subscription_month", "window_unit", "week"],
  ["calendar_week", "window_hours", 2],
  ["subscription_month", "unit", "token"],
]) {
  test(`${mode} rejects ${property}=${invalid}`, () => {
    const validate = new Ajv({ allErrors: true, strict: false }).compile(read("schema/platform.schema.json"));
    const platform = read("platforms/qianfan_coding/platform.json");
    const budget = platform.plan_quotas[0].budgets.find((entry) => entry.reset_mode === mode);
    budget[property] = invalid;
    assert.equal(validate(platform), false);
  });
}
