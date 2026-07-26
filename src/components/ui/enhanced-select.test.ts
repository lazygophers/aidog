// @vitest-environment node
import { describe, it, expect } from "vitest"

import {
  filterOptions,
  groupOptions,
  toggleValue,
  formatDisplay,
} from "./enhanced-select"

const opt = (
  value: string,
  label: string,
  group?: string,
): { value: string; label: string; group?: string } => ({ value, label, group })

describe("filterOptions", () => {
  it("empty query returns all", () => {
    const opts = [opt("a", "Apple"), opt("b", "Banana")]
    expect(filterOptions(opts, "")).toBe(opts)
    expect(filterOptions(opts, "   ")).toBe(opts)
  })
  it("matches label substring (case-insensitive)", () => {
    const opts = [opt("a", "Apple"), opt("b", "Banana")]
    expect(filterOptions(opts, "app")).toEqual([opts[0]])
    expect(filterOptions(opts, "BAN")).toEqual([opts[1]])
  })
  it("matches value field", () => {
    const opts = [opt("glm", "GLM"), opt("gpt", "GPT")]
    expect(filterOptions(opts, "gl")).toEqual([opts[0]])
  })
  it("pinyin match for Chinese labels", () => {
    const opts = [opt("bailian", "百炼"), opt("xiaomi", "小米")]
    expect(filterOptions(opts, "li")).toEqual([opts[0]]) // 百炼 = bailian
    expect(filterOptions(opts, "mi")).toEqual([opts[1]]) // 小米 = xiaomi
  })
  it("no match returns empty array", () => {
    const opts = [opt("a", "Apple")]
    expect(filterOptions(opts, "zzz")).toEqual([])
  })
})

describe("groupOptions", () => {
  it("groups by group field preserving first-seen order", () => {
    const opts = [
      opt("a", "A", "X"),
      opt("b", "B", "Y"),
      opt("c", "C", "X"),
    ]
    expect(groupOptions(opts)).toEqual([
      ["X", [opts[0], opts[2]]],
      ["Y", [opts[1]]],
    ])
  })
  it("no-group items go into empty-string bucket", () => {
    const opts = [opt("a", "A")]
    expect(groupOptions(opts)).toEqual([["", [opts[0]]]])
  })
  it("mixed grouped + ungrouped preserves order", () => {
    const opts = [opt("a", "A"), opt("b", "B", "X"), opt("c", "C")]
    const out = groupOptions(opts)
    expect(out).toEqual([
      ["", [opts[0], opts[2]]],
      ["X", [opts[1]]],
    ])
  })
})

describe("toggleValue", () => {
  it("appends when not present", () => {
    expect(toggleValue([], "a")).toEqual(["a"])
    expect(toggleValue(["b"], "a")).toEqual(["b", "a"])
  })
  it("removes when present (preserves order of others)", () => {
    expect(toggleValue(["a", "b", "c"], "b")).toEqual(["a", "c"])
  })
  it("idempotent toggle returns to original state", () => {
    const start = ["a", "b"]
    const toggled = toggleValue(toggleValue(start, "c"), "c")
    expect(toggled).toEqual(start)
  })
})

describe("formatDisplay", () => {
  const opts = [opt("a", "Apple"), opt("b", "Banana")]

  it("single mode returns matched label", () => {
    expect(formatDisplay(opts, "a", false, "ph")).toBe("Apple")
  })
  it("single mode empty/undefined returns placeholder", () => {
    expect(formatDisplay(opts, undefined, false, "ph")).toBe("ph")
    expect(formatDisplay(opts, "", false, "ph")).toBe("ph")
  })
  it("single mode falls back to value when label missing", () => {
    expect(formatDisplay([], "x", false, "ph")).toBe("x")
  })
  it("multi mode joins labels with ', '", () => {
    expect(formatDisplay(opts, ["a", "b"], true, "ph")).toBe("Apple, Banana")
  })
  it("multi mode empty array returns placeholder", () => {
    expect(formatDisplay(opts, [], true, "ph")).toBe("ph")
  })
  it("multi mode non-array value falls back to placeholder", () => {
    expect(formatDisplay(opts, "a", true, "ph")).toBe("ph")
  })
  it("multi mode missing label falls back to value", () => {
    expect(formatDisplay([], ["x", "y"], true, "ph")).toBe("x, y")
  })
})
