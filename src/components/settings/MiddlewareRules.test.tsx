// MiddlewareRulesPanel 列表测试（票 02）：渲染、builtin toggle-only、Failed 徽标、
// 「导入默认」入口不存在（统一引擎后内置规则不可删，无导入场景）。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { MiddlewareRulesPanel } from "./MiddlewareRules";
import type { MiddlewareRule } from "../../services/api";

const listRules = vi.hoisted(() => vi.fn());
const updateRule = vi.hoisted(() => vi.fn());
vi.mock("../../services/api/scheduling", () => ({
  middlewareApi: {
    listRules: (...a: unknown[]) => listRules(...a),
    updateRule: (...a: unknown[]) => updateRule(...a),
  },
}));

const leaf = (target: string, pattern: string) => ({
  kind: "leaf" as const,
  target,
  field: "",
  match_type: "contains" as const,
  pattern,
});

const mk = (over: Partial<MiddlewareRule>): MiddlewareRule => ({
  id: 1,
  name: "r",
  description: "",
  conditions: leaf("request_body", "x"),
  actions: [{ kind: "mask", params: { replacement: "****" } }],
  applies_to: { platforms: [], groups: [], models: [] },
  priority: 0,
  enabled: true,
  is_builtin: false,
  failed: false,
  created_at: 0,
  updated_at: 0,
  ...over,
});

beforeEach(() => {
  listRules.mockReset();
  updateRule.mockReset();
});

describe("MiddlewareRulesPanel（统一引擎列表）", () => {
  it("渲染规则摘要（条件 + 动作），无「导入默认」按钮", async () => {
    listRules.mockResolvedValue([mk({ id: 1, name: "user-rule" })]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("user-rule")).toBeTruthy());
    expect(screen.getByText(/req\.body contains \/x\//)).toBeTruthy();
    expect(screen.getByText("middleware.action.mask")).toBeTruthy();
    // 「一键导入默认」前后端入口已废（内置规则不可删，无导入场景）
    expect(screen.queryByText("导入默认规则")).toBeNull();
  });

  it("内置规则：可启停，无编辑/删除按钮（toggle-only）", async () => {
    listRules.mockResolvedValue([mk({ id: 2, name: "内置·密钥脱敏", is_builtin: true })]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("内置·密钥脱敏")).toBeTruthy());
    expect(screen.getByText("middleware.builtin")).toBeTruthy();
    // toggle 存在（role switch）
    expect(screen.getByRole("switch")).toBeTruthy();
    // 编辑（title=编辑）与删除按钮不存在
    expect(screen.queryByTitle("action.edit")).toBeNull();
    expect(screen.queryByTitle("action.delete")).toBeNull();
  });

  it("Failed Rule：显示失效徽标 + 仅删除按钮", async () => {
    listRules.mockResolvedValue([mk({ id: 3, name: "legacy", failed: true })]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("legacy")).toBeTruthy());
    expect(screen.getByText("middleware.failed")).toBeTruthy();
    expect(screen.getByTitle("action.delete")).toBeTruthy();
    expect(screen.queryByTitle("action.edit")).toBeNull();
  });

  it("点击启停调用 updateRule 翻转 enabled", async () => {
    listRules.mockResolvedValue([mk({ id: 4, name: "t", enabled: false })]);
    updateRule.mockResolvedValue(mk({ id: 4, name: "t", enabled: true }));
    const user = userEvent.setup();
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("t")).toBeTruthy());
    await user.click(screen.getByRole("switch"));
    await waitFor(() =>
      expect(updateRule).toHaveBeenCalledWith(
        expect.objectContaining({ id: 4, enabled: true }),
      ),
    );
  });

  it("platformId 过滤：applies_to.platforms 不含该 id 的规则不显示", async () => {
    listRules.mockResolvedValue([
      mk({ id: 5, name: "scoped", applies_to: { platforms: [8], groups: [], models: [] } }),
      mk({ id: 6, name: "wild" }),
    ]);
    render(<MiddlewareRulesPanel platformId={9} embedded />);
    await waitFor(() => expect(screen.getByText("wild")).toBeTruthy());
    expect(screen.queryByText("scoped")).toBeNull(); // 限 platform 8 的规则在 platform 9 面板隐藏
  });
});
