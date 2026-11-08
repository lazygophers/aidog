// ProfilesSection 行为测试：渲染真组件 + data-testid 断言（frontend-refactor-tests 规则）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({
  save: vi.fn(),
  apply: vi.fn(),
  list: vi.fn(),
  remove: vi.fn(),
}));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api/profile", () => ({ profileApi: h }));

import { ProfilesSection } from "./ProfilesSection";

const initial = [
  { name: "work", created_at: 1700000000 },
  { name: "home", created_at: 1699990000 },
];

beforeEach(() => {
  vi.clearAllMocks();
  h.list.mockResolvedValue(initial);
  h.save.mockImplementation((_n: string, _o: boolean) => Promise.resolve({ name: "x", created_at: 1 }));
  h.apply.mockResolvedValue([]);
  h.remove.mockResolvedValue(undefined);
});

describe("ProfilesSection", () => {
  it("加载已有方案列表", async () => {
    render(<ProfilesSection />);
    expect(await screen.findByTestId("profiles-row-work")).toHaveAttribute("data-name", "work");
    expect(screen.getByTestId("profiles-row-home")).toBeInTheDocument();
  });

  it("保存：save 带 name + overwrite=false，成功后清空输入并刷新列表", async () => {
    render(<ProfilesSection />);
    await screen.findByTestId("profiles-row-work");
    fireEvent.change(screen.getByTestId("profiles-name-input"), { target: { value: "demo" } });
    fireEvent.click(screen.getByTestId("profiles-save-btn"));
    await waitFor(() => expect(h.save).toHaveBeenCalledWith("demo", false));
    await waitFor(() => expect(screen.getByTestId("profiles-message")).toBeInTheDocument());
    await waitFor(() => expect(h.list).toHaveBeenCalledTimes(2));
  });

  it("同名保存被拒（后端 exists）→ 弹覆盖确认，确认后 overwrite=true", async () => {
    h.save.mockRejectedValueOnce("profile already exists: work");
    render(<ProfilesSection />);
    await screen.findByTestId("profiles-row-work");
    fireEvent.change(screen.getByTestId("profiles-name-input"), { target: { value: "work" } });
    fireEvent.click(screen.getByTestId("profiles-save-btn"));
    await screen.findByTestId("profiles-overwrite-dialog");
    fireEvent.click(screen.getByTestId("profiles-overwrite-confirm"));
    await waitFor(() => expect(h.save).toHaveBeenLastCalledWith("work", true));
  });

  it("应用：先 dry_run=true 取差异，确认对话框后 dry_run=false 执行", async () => {
    const items = [
      { target: "claude_dot_json", op: "update", detail: "还原" },
      { target: "db:groups", op: "update", detail: "分组对齐" },
    ];
    h.apply.mockImplementation((_n: string, dry: boolean) =>
      dry ? Promise.resolve(items) : Promise.resolve(items.map((i) => ({ ...i }))),
    );
    render(<ProfilesSection />);
    await screen.findByTestId("profiles-row-work");
    fireEvent.click(screen.getByTestId("profiles-apply-work"));
    await waitFor(() => expect(h.apply).toHaveBeenCalledWith("work", true));
    const dialog = await screen.findByTestId("profiles-diff-dialog");
    expect(dialog).toBeInTheDocument();
    expect(screen.getByTestId("profiles-diff-item-0").getAttribute("data-op")).toBe("update");
    expect(screen.getByTestId("profiles-diff-item-1")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("profiles-diff-confirm"));
    await waitFor(() => expect(h.apply).toHaveBeenLastCalledWith("work", false));
  });

  it("应用执行有失败面 → 错误态可见，不报成功", async () => {
    h.apply.mockImplementation((_n: string, dry: boolean) =>
      dry
        ? Promise.resolve([{ target: "claude_dot_json", op: "update", detail: "d" }])
        : Promise.resolve([{ target: "claude_dot_json", op: "update", detail: "d", error: "permission denied" }]),
    );
    render(<ProfilesSection />);
    await screen.findByTestId("profiles-row-work");
    fireEvent.click(screen.getByTestId("profiles-apply-work"));
    fireEvent.click(await screen.findByTestId("profiles-diff-confirm"));
    const err = await screen.findByTestId("profiles-error");
    expect(err.textContent).toContain("permission denied");
  });

  it("删除：两步确认后 remove(name) 并刷新", async () => {
    render(<ProfilesSection />);
    await screen.findByTestId("profiles-row-home");
    fireEvent.click(screen.getByTestId("profiles-delete-home"));
    await screen.findByTestId("profiles-delete-dialog");
    fireEvent.click(screen.getByTestId("profiles-delete-confirm"));
    await waitFor(() => expect(h.remove).toHaveBeenCalledWith("home"));
    await waitFor(() => expect(h.list).toHaveBeenCalledTimes(2));
  });

  it("空列表显示占位", async () => {
    h.list.mockResolvedValue([]);
    render(<ProfilesSection />);
    expect(await screen.findByTestId("profiles-empty")).toBeInTheDocument();
  });
});
