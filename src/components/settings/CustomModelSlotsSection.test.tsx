import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), usage: vi.fn(), purge: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api/customSlots", () => ({ customSlotsApi: h }));

import { CustomModelSlotsSection } from "./CustomModelSlotsSection";

const initial = [{ name: "fast", match: "contains" }, { name: "bg", match: "exact" }];
const add = (name: string) => {
  fireEvent.change(screen.getByTestId("slot-name-input"), { target: { value: name } });
  fireEvent.click(screen.getByTestId("slot-add"));
};

beforeEach(() => {
  vi.clearAllMocks();
  h.get.mockResolvedValue(initial);
  h.set.mockResolvedValue(undefined);
  h.usage.mockResolvedValue(3);
  h.purge.mockResolvedValue(3);
});

describe("CustomModelSlotsSection", () => {
  it("加载已有槽位，行上 data-match 反映匹配方式", async () => {
    render(<CustomModelSlotsSection />);
    expect((await screen.findByTestId("slot-row-fast")).getAttribute("data-match")).toBe("contains");
    expect(screen.getByTestId("slot-row-bg").getAttribute("data-match")).toBe("exact");
  });

  it("新增：默认包含匹配，set 带完整数组", async () => {
    render(<CustomModelSlotsSection />);
    await screen.findByTestId("slot-row-fast");
    add("my-slot1");
    await waitFor(() => expect(h.set).toHaveBeenCalledWith([...initial, { name: "my-slot1", match: "contains" }]));
  });

  it.each([
    ["Fast", "invalid"],
    ["a_b", "invalid"],
    ["", "invalid"],
    ["a".repeat(25), "invalid"],
    ["opus", "builtin"],
    ["embedding", "builtin"],
    ["fast", "duplicate"],
  ])("校验拒绝 %j -> %s，且不保存", async (name, kind) => {
    render(<CustomModelSlotsSection />);
    await screen.findByTestId("slot-row-fast");
    add(name);
    expect((await screen.findByTestId("slot-error")).getAttribute("data-state")).toBe(kind);
    expect(h.set).not.toHaveBeenCalled();
  });

  it("24 字符合法", async () => {
    render(<CustomModelSlotsSection />);
    await screen.findByTestId("slot-row-fast");
    add("a".repeat(24));
    await waitFor(() => expect(h.set).toHaveBeenCalledTimes(1));
  });

  it("改匹配方式：set 带新 match，名字不变", async () => {
    render(<CustomModelSlotsSection />);
    await screen.findByTestId("slot-row-fast");
    fireEvent.change(screen.getByTestId("slot-match-fast"), { target: { value: "exact" } });
    await waitFor(() => expect(h.set).toHaveBeenCalledWith([{ name: "fast", match: "exact" }, initial[1]]));
  });

  it("删除：确认框显示平台数，确认后先清平台再写定义", async () => {
    render(<CustomModelSlotsSection />);
    await screen.findByTestId("slot-row-fast");
    fireEvent.click(screen.getByTestId("slot-delete-fast"));
    const cnt = await screen.findByTestId("slot-delete-count");
    expect(cnt.getAttribute("data-count")).toBe("3");
    expect(h.usage).toHaveBeenCalledWith("fast");
    fireEvent.click(screen.getByTestId("slot-delete-confirm"));
    await waitFor(() => expect(h.set).toHaveBeenCalledWith([initial[1]]));
    expect(h.purge).toHaveBeenCalledWith("fast");
    expect(h.purge.mock.invocationCallOrder[0]).toBeLessThan(h.set.mock.invocationCallOrder[0]);
  });

  it("删除取消：不清平台、不写定义", async () => {
    render(<CustomModelSlotsSection />);
    await screen.findByTestId("slot-row-fast");
    fireEvent.click(screen.getByTestId("slot-delete-fast"));
    await screen.findByTestId("slot-delete-count");
    fireEvent.click(screen.getByTestId("slot-delete-cancel"));
    await waitFor(() => expect(screen.queryByTestId("slot-delete-count")).toBeNull());
    expect(h.purge).not.toHaveBeenCalled();
    expect(h.set).not.toHaveBeenCalled();
  });
});
