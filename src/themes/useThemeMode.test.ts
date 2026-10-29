// @vitest-environment node
import { describe, expect, it } from "vitest";
import { DEFAULT_MODE } from "./index";
import { readMode } from "./useThemeMode";

describe("readMode", () => {
  it("document 不可用（Next 预渲染）时返回缺省 mode", () => {
    expect(typeof document).toBe("undefined");
    expect(readMode()).toBe(DEFAULT_MODE);
  });
});
