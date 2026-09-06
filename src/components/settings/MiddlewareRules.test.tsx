// MiddlewareRulesPanel 列表测试（票 02）：渲染、builtin toggle-only、Failed 徽标、
// 「导入默认」入口不存在（统一引擎后内置规则不可删，无导入场景）。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { MiddlewareRulesPanel } from "./MiddlewareRules";
import type { MiddlewareRule } from "../../services/api";

const listRules = vi.hoisted(() => vi.fn());
const updateRule = vi.hoisted(() => vi.fn());
const budgetStatus = vi.hoisted(() => vi.fn());
vi.mock("../../services/api/scheduling", () => ({
  middlewareApi: {
    listRules: (...a: unknown[]) => listRules(...a),
    updateRule: (...a: unknown[]) => updateRule(...a),
    budgetStatus: () => budgetStatus(),
  },
}));
const platformList = vi.hoisted(() => vi.fn());
const groupList = vi.hoisted(() => vi.fn());
vi.mock("../../services/api/platforms", () => ({ platformApi: { list: () => platformList() } }));
vi.mock("../../services/api/groups", () => ({ groupApi: { list: () => groupList() } }));

const leaf = (target: string, pattern: string) => ({
  kind: "leaf" as const,
  target,
  field: "",
  match_type: "contains" as const,
  pattern,
  validator: "",
});

const mk = (over: Partial<MiddlewareRule>): MiddlewareRule => ({
  id: 1,
  name: "r",
  description: "",
  conditions: leaf("request_body", "x"),
  actions: [{ kind: "mask", params: { replacement: "****", fields: [] } }],
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
  platformList.mockReset().mockResolvedValue([]);
  groupList.mockReset().mockResolvedValue([]);
  budgetStatus.mockReset().mockResolvedValue([]);
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

  it("内置规则：可启停 + 查看详情（只读表单），无删除按钮", async () => {
    listRules.mockResolvedValue([mk({ id: 2, name: "内置·密钥脱敏", is_builtin: true })]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("内置·密钥脱敏")).toBeTruthy());
    expect(screen.getByText("middleware.builtin")).toBeTruthy();
    // toggle 存在（role switch）
    expect(screen.getByRole("switch")).toBeTruthy();
    // 删除按钮不存在（内置内容归 seed 管）
    expect(screen.queryByTitle("action.delete")).toBeNull();
    // 点编辑 → 只读查看表单：无保存按钮，有只读提示
    await userEvent.click(screen.getByTitle("action.edit"));
    await waitFor(() => expect(screen.getByText("middleware.builtinReadonlyHint")).toBeTruthy());
    expect(screen.queryByText("action.save")).toBeNull();
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

  it("观察模式徽标（票 04）：block 带 observe 才出，普通 block 不出", async () => {
    const blockStep = (observe: boolean) => ({
      kind: "block" as const,
      params: { replacement: "****", fields: [], inject_mode: "", target: "", value: "", category: "", retryable: true, override_status: null, override_body: null, observe },
    });
    listRules.mockResolvedValue([mk({ id: 1, name: "watching", actions: [blockStep(true)] })]);
    const { unmount } = render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("watching")).toBeTruthy());
    expect(screen.getByText("middleware.observe")).toBeTruthy();
    unmount();

    listRules.mockResolvedValue([mk({ id: 2, name: "hard-block", actions: [blockStep(false)] })]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("hard-block")).toBeTruthy());
    expect(screen.queryByText("middleware.observe")).toBeNull();
  });

  it("全局面板不按范围过滤：限定平台的规则也列出（唯一配置入口）", async () => {
    listRules.mockResolvedValue([
      mk({ id: 5, name: "scoped", applies_to: { platforms: [8], groups: [], models: [] } }),
      mk({ id: 6, name: "wild" }),
    ]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("wild")).toBeTruthy());
    expect(screen.getByText("scoped")).toBeTruthy();
  });

  // 票 06：预算闸门规则行展示本月已用 / 剩余额度；超限时提示已超预算。
  it("预算闸门规则：展示本月已用与剩余额度", async () => {
    listRules.mockResolvedValue([
      mk({ id: 7, name: "budget-rule", actions: [{ kind: "budget_gate", params: { budget_usd: 10 } }] }),
    ]);
    budgetStatus.mockResolvedValue([
      { rule_id: 7, rule_name: "budget-rule", budget_usd: 10, spent_usd: 3, remaining_usd: 7 },
    ]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("budget-rule")).toBeTruthy());
    expect(screen.getByText(/middleware\.budgetUsed \$3\.00 \/ \$10\.00/)).toBeTruthy();
    expect(screen.getByText(/middleware\.budgetRemaining \$7\.00/)).toBeTruthy();
  });

  it("预算闸门规则：已超限显示超预算提示", async () => {
    listRules.mockResolvedValue([
      mk({ id: 7, name: "budget-rule", actions: [{ kind: "budget_gate", params: { budget_usd: 10 } }] }),
    ]);
    budgetStatus.mockResolvedValue([
      { rule_id: 7, rule_name: "budget-rule", budget_usd: 10, spent_usd: 12.5, remaining_usd: -2.5 },
    ]);
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("middleware.budgetExceeded")).toBeTruthy());
  });

  it("新增 / 编辑走弹窗：默认无 dialog，点开后 role=dialog 存在，关闭后消失", async () => {
    listRules.mockResolvedValue([mk({ id: 7, name: "dlg" })]);
    const user = userEvent.setup();
    render(<MiddlewareRulesPanel />);
    await waitFor(() => expect(screen.getByText("dlg")).toBeTruthy());
    // 表单不再内嵌在页面底部
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByPlaceholderText("middleware.name")).toBeNull();

    await user.click(screen.getByText("+ middleware.addRule"));
    const dialog = await screen.findByRole("dialog");
    expect(dialog.querySelector("input[placeholder='middleware.name']")).toBeTruthy();

    await user.click(screen.getByText("action.cancel"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});

import { RuleForm } from "./MiddlewareRules";

describe("RuleForm（票 04/05：卡片编辑器 + DSL）", () => {
  beforeEach(() => {
    platformList.mockReset().mockResolvedValue([]);
    groupList.mockReset().mockResolvedValue([]);
  });

  it("提交 payload 与引擎模型一致（票 04）", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<RuleForm onSave={onSave} onCancel={() => {}} />);
    await user.type(screen.getByPlaceholderText("middleware.name"), "my-rule");
    await user.clear(screen.getByPlaceholderText("middleware.pattern"));
    await user.type(screen.getByPlaceholderText("middleware.pattern"), "sk-abc");
    await user.click(screen.getByRole("button", { name: "action.save" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "my-rule",
        conditions: {
          kind: "leaf",
          target: "request_body",
          field: "",
          match_type: "contains",
          pattern: "sk-abc",
          validator: "",
        },
        actions: [{ kind: "mask", params: expect.objectContaining({ replacement: "****", fields: [] }) }],
        applies_to: { platforms: [], groups: [], models: [] },
        is_builtin: false,
      }),
    );
  });

  it("DSL 模式：非法 DSL 禁保存 + 禁切回卡片并提示位置（票 05）", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<RuleForm onSave={onSave} onCancel={() => {}} />);
    await user.click(screen.getByRole("button", { name: "middleware.toDsl" }));
    // 定位 DSL textarea（最后一个 textarea）
    const dsl = screen.getByTestId("mw-dsl") as HTMLTextAreaElement;
    await user.clear(dsl);
    await user.type(dsl, 'foo contains "x"');
    await waitFor(() => expect(screen.getByText(/未知 target/)).toBeTruthy());
    // 保存与切回按钮均禁用
    expect(screen.getByRole("button", { name: "action.save" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "middleware.toCards" })).toBeDisabled();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("DSL 双向同步：合法 DSL 切回卡片后提交解析结果", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<RuleForm onSave={onSave} onCancel={() => {}} />);
    await user.type(screen.getByPlaceholderText("middleware.name"), "dsl-rule");
    await user.click(screen.getByRole("button", { name: "middleware.toDsl" }));
    const dsl = screen.getByTestId("mw-dsl") as HTMLTextAreaElement;
    await user.clear(dsl);
    await user.type(dsl, 'ALL(request_body regex "a+" model exact "m")')
    await user.click(screen.getByRole("button", { name: "middleware.toCards" }));
    await user.click(screen.getByRole("button", { name: "action.save" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        conditions: {
          kind: "all",
          children: [
            { kind: "leaf", target: "request_body", field: "", match_type: "regex", pattern: "a+", validator: "" },
            { kind: "leaf", target: "model", field: "", match_type: "exact", pattern: "m", validator: "" },
          ],
        },
      }),
    );
  });
});
