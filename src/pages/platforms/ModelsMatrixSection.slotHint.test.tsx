// embedding / rerank 槽位无 OpenAI 系端点提示（票 04）：选择器锚稳定 id + data-state，不用文案。
import { render } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const base = { embedding: "", rerank: "", default: "", sonnet: "", opus: "", haiku: "", gpt: "", jev: "" };

function mount(models: Record<string, string>, endpoints: { protocol: string }[], protocol = "anthropic") {
  return render(
    <ModelsMatrixSection
      models={{ ...base, ...models } as never} handleModelChange={() => {}} handleModelSelect={() => {}}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={[]}
      protocol={protocol as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={endpoints.length}
      endpoints={endpoints} rules={[]} setRules={() => {}} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
    />,
  );
}

describe("embedding/rerank 槽位提示", () => {
  it("无 OpenAI 系端点且槽位非空 → 显示", () => {
    const { container } = mount({ embedding: "e1", rerank: "r1" }, [{ protocol: "anthropic" }]);
    expect(container.querySelector("#slot-hint-embedding")?.getAttribute("data-state")).toBe("warn");
    expect(container.querySelector("#slot-hint-rerank")).not.toBeNull();
  });
  it("有 OpenAI 系端点（或平台协议即 OpenAI 系）→ 不显示", () => {
    expect(mount({ embedding: "e1" }, [{ protocol: "openai_responses" }]).container.querySelector("#slot-hint-embedding")).toBeNull();
    expect(mount({ embedding: "e1" }, [], "openai").container.querySelector("#slot-hint-embedding")).toBeNull();
  });
  it("槽位为空 → 不显示", () => {
    expect(mount({}, [{ protocol: "anthropic" }]).container.querySelector("[id^=slot-hint-]")).toBeNull();
  });
});
