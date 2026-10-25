// 可视化 ⇄ 文本双向转换测试（通知体系票 05）：往返等价 + 不可转换降级判定。
import { describe, it, expect } from "vitest";
import { parseToVisual, roundTrips, toText, type Visual } from "./ruleExpr";

const v = (join: "&&" | "||", ...terms: [string, "==" | "!=" | "contains", string][]): Visual => ({
  join,
  terms: terms.map(([field, op, value]) => ({ field, op, value })),
});

describe("ruleExpr — 票 05", () => {
  it("可视化 → 文本：规范式（值双引号）", () => {
    expect(toText(v("&&", ["severity", "==", "error"]))).toBe('severity == "error"');
    expect(toText(v("||", ["type", "!=", "task_complete"], ["vars.platform", "contains", "glm"])))
      .toBe('type != "task_complete" || vars.platform contains "glm"');
  });

  it("往返等价：toText → parseToVisual 结构不变", () => {
    const cases = [
      v("&&", ["severity", "==", "error"]),
      v("&&", ["severity", "==", "warn"], ["vars.platform_id", "!=", "7"]),
      v("||", ["type", "contains", "breaker"], ["severity", "==", "error"]),
      // 值含空格/中文也稳定
      v("&&", ["vars.cwd", "==", "/Users/x/我的 项目"]),
    ];
    for (const c of cases) {
      expect(roundTrips(c), toText(c)).toBe(true);
      const r = parseToVisual(toText(c));
      expect(r.ok && JSON.stringify(r.visual) === JSON.stringify(c)).toBe(true);
    }
  });

  it("文本 → 可视化：裸词值 / 引号值同构", () => {
    const a = parseToVisual("severity == error");
    expect(a.ok && a.visual.terms[0].value).toBe("error");
    const b = parseToVisual('vars.n == "3"');
    expect(b.ok && b.visual.terms[0].value).toBe("3");
  });

  it("不可转换：&& 与 || 混用 → 明确降级理由", () => {
    const r = parseToVisual('type == "x" && severity == "error" || vars.k == "v"');
    expect(r.ok).toBe(false);
    expect(!r.ok && r.reason).toContain("混用");
  });

  it("不可转换：括号分组 / 语法错 / 空", () => {
    expect(parseToVisual("(a == 1 && b == 2) || c == 3").ok).toBe(false);
    expect(parseToVisual("severity ~ x").ok).toBe(false);
    expect(parseToVisual('severity == "unterminated').ok).toBe(false);
    expect(parseToVisual("").ok).toBe(false);
    expect(parseToVisual("severity ==").ok).toBe(false);
  });
});
