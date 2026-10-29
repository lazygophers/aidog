import { describe, it, expect } from "vitest";
import { render, screen } from "../../test/render";
import { TestResultBody } from "./TestResultBody";

describe("TestResultBody 渲染", () => {
  it("空正文不渲染", () => {
    const { container } = render(<TestResultBody body="" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("非 JSON 回退原文", () => {
    render(<TestResultBody body="HTTP 500 oops" />);
    expect(screen.getByText("HTTP 500 oops")).toBeInTheDocument();
  });

  it("已知结构渲染 label/value 行", () => {
    render(<TestResultBody body={JSON.stringify({ error: { message: "bad key", type: "auth" } })} />);
    expect(screen.getByText("testBody.errorMessage")).toBeInTheDocument();
    expect(screen.getByText("bad key")).toBeInTheDocument();
    expect(screen.getByText("auth")).toBeInTheDocument();
  });
});
