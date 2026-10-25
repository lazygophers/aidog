// 通知规则区就近测试（票 04）：预置开关/复制、用户规则编辑、表达式校验报错显示。
// 选择器锚稳定 id + data-state（frontend-refactor-tests 规则）；文案断言用 i18n key
// （test/render 空 resources 回退 key 本身，不锚译文）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../../test/render";
import { NotificationRulesSection } from "./NotificationRules";
import { notificationApi } from "../../services/api";
import type { NotificationSettings, Rule } from "../../services/api";

vi.mock("../../services/api", async () => {
  const actual = await vi.importActual<typeof import("../../services/api")>("../../services/api");
  return {
    ...actual,
    notificationApi: {
      ...actual.notificationApi,
      validateRuleExpr: vi.fn(),
    },
  };
});

function preset(id: string, typ: string): Rule {
  return {
    id, name: id, enabled: true, preset: true, match_type: typ, condition: null,
    action: { channels: ["inbox"], bypass_dnd: false, dedup_window_secs: 0 },
  };
}
function user(id: string): Rule {
  return { ...preset(id, "error"), preset: false };
}

function renderRules(rules: Rule[]) {
  const settings = {
    rules,
    channels: {},
  } as unknown as NotificationSettings;
  const persist = vi.fn((f: (p: NotificationSettings) => NotificationSettings) => f(settings));
  render(<NotificationRulesSection settings={settings} persist={persist} />);
  return { persist, settings };
}

describe("NotificationRulesSection — 票 04", () => {
  beforeEach(() => vi.clearAllMocks());

  it("预置规则：开关切换走 persist；复制生成非预置副本", () => {
    const { persist } = renderRules([preset("preset-breaker-on", "breaker_on")]);
    const row = document.getElementById("notif-rule-preset-breaker-on")!;
    expect(row.getAttribute("data-enabled")).toBe("yes");
    // 预置行内有复制按钮（锚 key 文案），用户行没有
    expect(screen.getByText("notif.rules.copyAction")).toBeTruthy();
    fireEvent.click(screen.getByText("notif.rules.copyAction"));
    const next = persist.mock.results[0].value as NotificationSettings;
    expect(next.rules).toHaveLength(2);
    expect(next.rules[1].preset).toBe(false);
    expect(next.rules[1].id).toContain("copy-of-");
  });

  it("用户规则：编辑器开合 + 删除", () => {
    const { persist } = renderRules([user("user-1")]);
    fireEvent.click(screen.getByText("notif.rules.edit"));
    expect(document.querySelector('[id^="notif-rule-editor-user-1"]')?.getAttribute("data-state")).toBe("open");
    fireEvent.click(screen.getByText("notif.rules.delete"));
    const next = persist.mock.results.at(-1)!.value as NotificationSettings;
    expect(next.rules).toHaveLength(0);
  });

  it("表达式校验：后端报错原文显示（带定位），data-ok=no", async () => {
    vi.mocked(notificationApi.validateRuleExpr).mockRejectedValueOnce('第 9 字符附近：期望值');
    renderRules([user("user-2")]);
    fireEvent.click(screen.getByText("notif.rules.edit"));
    const cond = document.querySelector<HTMLInputElement>('input[placeholder^="severity =="]')!;
    fireEvent.change(cond, { target: { value: 'severity ==' } });
    fireEvent.click(screen.getByText("notif.rules.validate"));
    const msg = await screen.findByText(/第 9 字符附近/);
    expect(msg.getAttribute("data-ok")).toBe("no");
  });

  it("票 05：可解析条件打开编辑器即可视化模式；行组序列化显示规范式", () => {
    const rules = [{ ...user("user-v"), condition: 'severity == "error" && vars.platform contains "glm"' }];
    const settings = { rules, channels: {} } as unknown as NotificationSettings;
    const persist = vi.fn();
    render(<NotificationRulesSection settings={settings} persist={persist} />);
    fireEvent.click(screen.getByText("notif.rules.edit"));
    expect(document.getElementById("notif-rule-mode-visual")!.getAttribute("data-state")).toBe("active");
    expect(document.getElementById("notif-rule-visual")).toBeTruthy();
    expect(document.getElementById("notif-rule-term-1")).toBeTruthy();
  });

  it("票 05：混用 &&/|| 的文本切可视化 → 降级提示、停留文本、原文保留", () => {
    const mixed = 'type == "x" && severity == "error" || vars.k == "v"';
    const rules = [{ ...user("user-m"), condition: mixed }];
    const settings = { rules, channels: {} } as unknown as NotificationSettings;
    const persist = vi.fn();
    render(<NotificationRulesSection settings={settings} persist={persist} />);
    fireEvent.click(screen.getByText("notif.rules.edit"));
    expect(document.getElementById("notif-rule-mode-text")!.getAttribute("data-state")).toBe("active");
    fireEvent.click(document.getElementById("notif-rule-mode-visual")!);
    const hint = document.getElementById("notif-rule-degrade")!;
    expect(hint.getAttribute("data-state")).toBe("shown");
    expect(hint.textContent).toContain("notif.rules.degradeHint");
    // 停留文本模式，原文未丢
    expect(document.getElementById("notif-rule-mode-text")!.getAttribute("data-state")).toBe("active");
    const cond = document.querySelector<HTMLInputElement>('input[placeholder^="severity =="]')!;
    expect(cond.value).toBe(mixed);
  });

  it("表达式校验通过：validOk 文案 data-ok=yes", async () => {
    vi.mocked(notificationApi.validateRuleExpr).mockResolvedValueOnce(undefined);
    renderRules([user("user-3")]);
    fireEvent.click(screen.getByText("notif.rules.edit"));
    const cond = document.querySelector<HTMLInputElement>('input[placeholder^="severity =="]')!;
    fireEvent.change(cond, { target: { value: 'severity == "error"' } });
    fireEvent.click(screen.getByText("notif.rules.validate"));
    const msg = await screen.findByText("notif.rules.validOk");
    expect(msg.getAttribute("data-ok")).toBe("yes");
  });
});
