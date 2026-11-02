// 词表每个代码（大类 + 变体 + 旧 7 码 + other）在 8 locale 都有非空显示名；未知新码回落原始代码。
import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { clientLabel } from "./clientLabel";

const root = resolve(__dirname, "../..");
const localeDir = resolve(root, "src-tauri/crates/aidog_i18n/locales");
const vocabSrc = readFileSync(resolve(root, "src-tauri/crates/aidog_core/src/client_vocab.rs"), "utf8");
const vocabCodes = [...vocabSrc.matchAll(/^\s*(?:cat|var)\("([a-z0-9_]+)"/gm)].map(m => m[1]);
const legacy = ["claude_code", "codex", "pi", "cursor", "windsurf", "gemini_cli"];
const allCodes = [...new Set([...vocabCodes, ...legacy, "other"])];
// 品牌名本身即代码字面量，显示名与代码相同属正确。
const sameAsCode = new Set(["opencode", "curl"]);

const locales = readdirSync(localeDir).filter(f => f.endsWith(".json"));

describe("clientLabel", () => {
  it("词表码数合理（防正则漂移）", () => {
    expect(vocabCodes.length).toBeGreaterThanOrEqual(40);
    expect(locales.length).toBe(8);
  });

  for (const f of locales) {
    const dict: Record<string, string> = JSON.parse(readFileSync(resolve(localeDir, f), "utf8"));
    const t = (k: string, fb?: string) => dict[k] ?? fb ?? k;
    it(`${f}: 每个词表代码有非空显示名`, () => {
      const bad = allCodes.filter(c => {
        const v = clientLabel(t, c);
        return !v || (v === c && !sameAsCode.has(c));
      });
      expect(bad).toEqual([]);
      expect(clientLabel(t, "")).not.toBe("");
    });
  }

  it("未知新码回落原始代码不抛错", () => {
    expect(clientLabel((k, fb) => fb ?? k, "brand_new_client")).toBe("brand_new_client");
  });

  it("ua:<产品名> 显示原文，不走 i18n", () => {
    const t = (k: string) => `T(${k})`;
    expect(clientLabel(t, "ua:OpenViking")).toBe("OpenViking");
    expect(clientLabel(t, "ua:My_Tool-2.x")).toBe("My_Tool-2.x");
  });
});
