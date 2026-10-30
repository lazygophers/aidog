import { describe, it, expect } from "vitest";
import { render } from "../../../test/render";
import { FieldRenderer } from "./FieldRenderer";

describe("FieldRenderer string+options", () => {
  it("输入框挂 datalist，选项齐全", () => {
    const { container } = render(
      <FieldRenderer
        field={{ key: "outputStyle", label: "Output Style", type: "string", options: ["default", "Mine"] }}
        value="x"
        onChange={() => {}}
        t={((k: string) => k) as never}
      />,
    );
    const dl = container.querySelector("#dl-outputStyle")!;
    expect([...dl.querySelectorAll("option")].map((o) => o.getAttribute("value"))).toEqual(["default", "Mine"]);
    expect(container.querySelector('input[list="dl-outputStyle"]')).not.toBeNull();
  });
});
