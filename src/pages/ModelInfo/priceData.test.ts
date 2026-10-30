// 防漂移门禁：前端 CNY_PER_USD 须与 Rust 真值源 aidog_db/src/price_resolve.rs 的同名常量相等。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, it, expect } from "vitest";
import { CNY_PER_USD, toUsd } from "./priceData";

describe("CNY_PER_USD", () => {
  it("与 Rust price_resolve.rs 常量一致", () => {
    const src = readFileSync(resolve(process.cwd(), "src-tauri/crates/aidog_db/src/price_resolve.rs"), "utf8");
    const m = /pub const CNY_PER_USD: f64 = ([0-9.]+);/.exec(src);
    expect(m).not.toBeNull();
    expect(CNY_PER_USD).toBe(Number(m![1]));
  });
  it("toUsd 仅换算 CNY", () => {
    expect(toUsd(CNY_PER_USD, "CNY")).toBe(1);
    expect(toUsd(5, "USD")).toBe(5);
    expect(toUsd(5)).toBe(5);
  });
});
