// 日志页：点行 → 整页详情替换列表；返回 → 列表回来且筛选 / 分页 state 保持（state 在页面级 hook，不随列表卸载）。
import { describe, it, expect, vi } from "vitest";
import { useState } from "react";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";

const filters = { platformMap: new Map(), groupName: (k: string) => k, activeFilter: {}, hasFilter: false, marker: "kept" };
vi.mock("./Logs/useLogsFilters", () => ({ useLogsFilters: () => filters }));
vi.mock("./Logs/useLogsList", () => ({ useLogsList: () => ({}) }));
vi.mock("./Logs/useLogsDetail", () => ({
  useLogsDetail: () => {
    const [detail, setDetail] = useState<{ id: string } | null>(null);
    return { detail, setDetail, openDetail: (id: string) => setDetail({ id }), copyRow: () => {} };
  },
}));
vi.mock("./Logs/ListView", () => ({
  ListView: ({ filters: f, openDetail }: { filters: { marker: string }; openDetail: (id: string) => void }) => (
    <button data-testid="list-view" data-marker={f.marker} onClick={() => openDetail("r1")} />
  ),
}));
vi.mock("./Logs/DetailPanel", () => ({
  DetailPanel: ({ d }: { d: { setDetail: (v: null) => void } }) => <button data-testid="detail-page" onClick={() => d.setDetail(null)} />,
}));

import { Logs } from "./Logs";

describe("Logs 列表 ⇄ 独立详情页", () => {
  it("点行进详情页（列表卸载），返回回列表且筛选状态保持", () => {
    render(<Logs />);
    expect(screen.queryByTestId("detail-page")).toBeNull();
    fireEvent.click(screen.getByTestId("list-view"));
    expect(screen.getByTestId("detail-page")).not.toBeNull();
    expect(screen.queryByTestId("list-view")).toBeNull();
    fireEvent.click(screen.getByTestId("detail-page"));
    expect(screen.getByTestId("list-view").getAttribute("data-marker")).toBe("kept");
    expect(screen.queryByTestId("detail-page")).toBeNull();
  });
});
