import { describe, it, expect } from "vitest";
import { buildCodexCommand, buildPiCommand } from "./commands";

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
