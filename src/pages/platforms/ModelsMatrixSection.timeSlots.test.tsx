// 时段档 embedding / rerank 行（票 时段档-模型配置对齐/02）：渲染真组件，选择器锚 data-testid / id + data-state，不用文案。
import { render } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const base = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };

function mountTime(ruleModels: Record<string, string[]>, endpoints: { protocol: string }[], protocol = "anthropic") {
  return render(
    <ModelsMatrixSection
      columns="time"
      models={base as never} onModelsChange={() => {}}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={[]}
      protocol={protocol as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={endpoints.length}
      endpoints={endpoints}
      rules={[{ windows: [], models: ruleModels as never }]} setRules={() => {}} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
    />,
  );
}

describe("时段档 embedding / rerank 行", () => {
  it("出现 embedding / rerank 行，带专用徽标与计数", () => {
    const { container } = mountTime({ embedding: ["e1", "e2"] }, [{ protocol: "openai" }]);
    expect(container.querySelector('[data-testid="time-slot-embedding"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="time-slot-rerank"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="time-slot-badge-embedding"]')).not.toBeNull();
    expect(container.querySelectorAll(".model-matrix-row").length).toBe(8);
  });
  it("档内 embedding / rerank 为空 → 显示「时段内不接」提示（data-state=empty）", () => {
    const { container } = mountTime({}, [{ protocol: "openai" }]);
    expect(container.querySelector("#time-slot-hint-embedding")?.getAttribute("data-state")).toBe("empty");
    expect(container.querySelector("#time-slot-hint-rerank")?.getAttribute("data-state")).toBe("empty");
  });
  it("档内有值且无 OpenAI 系端点 → 与「模型」tab 相同的警告提示（data-state=warn）", () => {
    const { container } = mountTime({ embedding: ["e1"] }, [{ protocol: "anthropic" }]);
    expect(container.querySelector("#time-slot-hint-embedding")?.getAttribute("data-state")).toBe("warn");
  });
  it("档内有值且有 OpenAI 系端点 → 无提示；聊天空槽不出此提示", () => {
    const { container } = mountTime({ embedding: ["e1"] }, [{ protocol: "openai_responses" }]);
    expect(container.querySelector("#time-slot-hint-embedding")).toBeNull();
    expect(container.querySelector("#time-slot-hint-default")).toBeNull();
  });
});
