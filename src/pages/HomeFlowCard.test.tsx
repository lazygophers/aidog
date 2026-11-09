// HomeFlowCard 测试（请求流向票 05）：选择器只锚 data-testid / data-state / data-loading，不断言文案。
import { describe, it, expect } from "vitest";
import { vi } from "vitest";
import { render, screen, fireEvent } from "../test/render";
import { HomeFlowCard } from "./HomeFlowCard";
import type { FlowRow, Platform } from "../services/api";

const row = (o: Partial<FlowRow>): FlowRow => ({
  client: "claude_code", group_key: "g1", req_model: "m", platform_id: 7, model: "am",
  requests: 2, tokens: 100, est_cost: 1, ...o,
});
const plats = [{ id: 7, platform_type: "openai" }] as unknown as Platform[];

// Radix Select 在 jsdom 无法开面板：换成原生 select 壳（同 DisplayCurrencySection.test）。
vi.mock("@/components/ui/select", () => ({
  Select: ({ value, onValueChange, children }: { value: string; onValueChange: (v: string) => void; children: React.ReactNode }) => (
    <select data-testid="request-class-filter" value={value} onChange={(e) => onValueChange(e.target.value)}>{children}</select>
  ),
  SelectTrigger: () => null,
  SelectValue: () => null,
  SelectContent: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  SelectItem: ({ value, children }: { value: string; children: React.ReactNode }) => <option value={value}>{children}</option>,
}));

describe("HomeFlowCard", () => {
  it("五列按序渲染", () => {
    render(<HomeFlowCard rows={[row({})]} platforms={plats} ungroupedLabel="u" loading={false} requestClass="" onRequestClassChange={() => {}} />);
    const cols = [...document.querySelectorAll('[data-testid^="flow-col-"]')].map(e => e.getAttribute("data-testid"));
    expect(cols).toEqual(["client", "group", "requestModel", "platform", "actualModel"].map(k => `flow-col-${k}`));
  });

  it("空数据 -> 空态，无图", () => {
    render(<HomeFlowCard rows={[]} platforms={[]} ungroupedLabel="u" loading={false} requestClass="" onRequestClassChange={() => {}} />);
    expect(screen.getByTestId("flow-empty")).toBeTruthy();
    expect(screen.queryByTestId("flow-chart")).toBeNull();
  });

  it("加载中 data-loading=yes", () => {
    render(<HomeFlowCard rows={[]} platforms={[]} ungroupedLabel="u" loading requestClass="" onRequestClassChange={() => {}} />);
    expect(screen.getByTestId("home-flow").getAttribute("data-loading")).toBe("yes");
    expect(screen.queryByTestId("flow-empty")).toBeNull();
  });

  it("group_key='' 渲染为独立节点（id 空串）", () => {
    render(<HomeFlowCard rows={[row({ group_key: "" })]} platforms={plats} ungroupedLabel="u" loading={false} requestClass="" onRequestClassChange={() => {}} />);
    expect(document.querySelector('[data-testid="flow-node-group-"]')).toBeTruthy();
  });

  it("platform_id=0 / 空 req_model 有显示名", () => {
    render(<HomeFlowCard rows={[row({ platform_id: 0, req_model: "" })]} platforms={plats} ungroupedLabel="u" loading={false} requestClass="" onRequestClassChange={() => {}} />);
    expect(document.querySelector('[data-testid="flow-node-platform-0"]')!.textContent).not.toContain("#0");
    expect(document.querySelector('[data-testid="flow-node-requestModel-"]')!.textContent!.length).toBeGreaterThan(0);
  });

  it("度量默认 tokens，可切换", () => {
    render(<HomeFlowCard rows={[row({})]} platforms={plats} ungroupedLabel="u" loading={false} requestClass="" onRequestClassChange={() => {}} />);
    expect(screen.getByTestId("flow-metric-tokens").getAttribute("data-state")).toBe("on");
    fireEvent.click(screen.getByTestId("flow-metric-cost"));
    expect(screen.getByTestId("flow-metric-cost").getAttribute("data-state")).toBe("on");
    expect(screen.getByTestId("flow-metric-tokens").getAttribute("data-state")).toBe("off");
  });

  it("请求类型过滤器：反映当前值，变更回调选中值（''=全部）", () => {
    const onChange = vi.fn();
    render(<HomeFlowCard rows={[row({})]} platforms={plats} ungroupedLabel="u" loading={false} requestClass="subagent" onRequestClassChange={onChange} />);
    const sel = screen.getByTestId("request-class-filter") as HTMLSelectElement;
    expect(sel.value).toBe("subagent");
    fireEvent.change(sel, { target: { value: "all" } });
    expect(onChange).toHaveBeenLastCalledWith("");
  });
});
