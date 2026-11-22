import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SegmentEditModal } from "./SegmentEditModal";
import type { StatusLineSegment } from "../../statusline-gen";

const t = ((_k: string, d?: string) => d ?? _k) as never;

function save(options: Record<string, unknown>, edit?: string) {
  const onSave = vi.fn();
  const segment: StatusLineSegment = { id: "s1", type: "group-balance", enabled: true, newline: false, options };
  render(<SegmentEditModal segment={segment} isRowLeader={false} onSave={onSave} onClose={() => {}} t={t} />);
  if (edit !== undefined) fireEvent.change(screen.getByTestId("sl-seg-field-prefix"), { target: { value: edit } });
  fireEvent.click(screen.getByTestId("sl-seg-save"));
  return onSave.mock.calls[0][0].options;
}

describe("SegmentEditModal 默认前缀", () => {
  it("未改动的默认前缀不存", () => {
    expect(save({})).not.toHaveProperty("prefix");
  });
  it("用户清空成空串照存", () => {
    expect(save({}, "")).toHaveProperty("prefix", "");
  });
  it("用户改过的前缀照存", () => {
    expect(save({}, "B: ")).toHaveProperty("prefix", "B: ");
  });
  it("已存的旧前缀不动", () => {
    expect(save({ prefix: "Balance " })).toHaveProperty("prefix", "Balance ");
  });
});
