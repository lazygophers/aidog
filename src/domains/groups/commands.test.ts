import { describe, it, expect } from "vitest";
import { buildCodexCommand, buildPiCommand, buildResumeCommand } from "./commands";

describe("buildResumeCommand", () => {
  it("resumes with the group settings file, both args shell-quoted", () => {
    expect(buildResumeCommand("deadbeef", "teamA")).toBe(
      "claude --resume 'deadbeef' --settings '~/.aidog/settings.teamA.json'",
    );
  });

  it("drops --settings for groupless sessions", () => {
    expect(buildResumeCommand("deadbeef", "")).toBe("claude --resume 'deadbeef'");
  });

  it("quotes shell metacharacters in session id and group key", () => {
    const cmd = buildResumeCommand("x'; rm -rf /", "g' ; calc");
    expect(cmd).toContain("'x'\\''; rm -rf /'");
    expect(cmd).toContain("'~/.aidog/settings.g'\\'' ; calc.json'");
  });
});

describe("buildPiCommand", () => {
  it("selects the aidog provider for the group", () => {
    expect(buildPiCommand("teamA")).toBe("pi --provider 'aidog-teamA'");
  });

  it("carries no routing env — token lives in models.json apiKey", () => {
    expect(buildPiCommand("teamA")).not.toContain("AIDOG_KEY");
    expect(buildPiCommand("teamA")).not.toContain("ANTHROPIC_");
  });

  it("does not prepend removed group environment variables", () => {
    expect(buildPiCommand("g")).toBe("pi --provider 'aidog-g'");
    expect(buildCodexCommand("g")).toContain("AIDOG_KEY='g' codex");
    expect(buildCodexCommand("g")).not.toContain("export ");
  });

  it("quotes a group key containing shell metacharacters", () => {
    // 分组名是用户自由输入，未加引号的 `;` 会把后半段当成第二条命令执行。
    expect(buildPiCommand("a'b; rm -rf /")).toBe("pi --provider 'aidog-a'\\''b; rm -rf /'");
  });
});
