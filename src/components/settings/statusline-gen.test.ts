// @vitest-environment node
// Snapshot-locks the statusline script generator so useStatusLinePanel and the
// Settings.tsx save path (both consuming materializeStatusline) can never
// silently drift from each other or from a prior release's output.
import { describe, it, expect } from "vitest";
import {
  generateStatusLineScript,
  generateSubagentStatusLineScript,
  materializeStatusline,
} from "./statusline-gen";
import {
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
  type StatusLineSegment,
} from "./statusline-segments";

describe("generateStatusLineScript", () => {
  it("matches snapshot for the typical (default) 3-row main layout", () => {
    expect(generateStatusLineScript(DEFAULT_SEGMENTS)).toMatchSnapshot();
  });

  it("matches snapshot for the boundary case: no active segments", () => {
    const allDisabled = DEFAULT_SEGMENTS.map(s => ({ ...s, enabled: false }));
    expect(generateStatusLineScript(allDisabled)).toMatchSnapshot();
  });
});

describe("generateSubagentStatusLineScript", () => {
  it("matches snapshot for the default subagent layout", () => {
    expect(generateSubagentStatusLineScript(DEFAULT_SUBAGENT_SEGMENTS)).toMatchSnapshot();
  });
});

describe("materializeStatusline", () => {
  it("main statusline: enabled+builtin with explicit segments — full generation, byte-identical to generateStatusLineScript", () => {
    const segments: StatusLineSegment[] = DEFAULT_SEGMENTS.map(s => ({ ...s }));
    const stored = { enabled: true, mode: "builtin", segments };
    const result = materializeStatusline(stored, "statusline");
    expect(result.scriptContent).toBe(generateStatusLineScript(segments));
    expect(result).toMatchSnapshot();
  });

  it("subagent: enabled+builtin missing segments falls back to DEFAULT_SUBAGENT_SEGMENTS", () => {
    const result = materializeStatusline({ enabled: true }, "subagent");
    expect(result.scriptContent).toBe(generateSubagentStatusLineScript(DEFAULT_SUBAGENT_SEGMENTS));
  });

  it("boundary: disabled — no script generated regardless of mode", () => {
    const result = materializeStatusline({ enabled: false, mode: "builtin", segments: DEFAULT_SEGMENTS }, "statusline");
    expect(result).toEqual({ enabled: false, mode: "builtin", scriptContent: null, customCommand: "" });
  });

  it("boundary: custom mode — no script generated even when enabled", () => {
    const result = materializeStatusline({ enabled: true, mode: "custom", customCommand: "~/my.sh" }, "statusline");
    expect(result).toEqual({ enabled: true, mode: "custom", scriptContent: null, customCommand: "~/my.sh" });
  });

  it("boundary: undefined stored block — disabled builtin defaults", () => {
    const result = materializeStatusline(undefined, "statusline");
    expect(result).toEqual({ enabled: false, mode: "builtin", scriptContent: null, customCommand: "" });
  });
});
