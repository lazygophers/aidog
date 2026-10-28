// Platforms 表单路由（2026-10-28 拍板）：新建走全页、编辑 / 复制进抽屉。
// usePlatformsState 整体 mock（真 hook 走 Tauri IPC）；PlatformListView mock 掉；
// PlatformEditForm 保持真实（footer / back 按钮即路由断言点）。
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import type { PlatformsState } from "./platforms/usePlatformsState";
import { Platforms } from "./Platforms";

vi.mock("../../domains/platforms", () => ({
  SearchableProtocolSelect: ({ value }: { value: string }) => <select aria-label="protocol" value={value} onChange={() => {}}><option value={value}>{value}</option></select>,
  MockConfigEditor: () => null,
}));
vi.mock("../../domains/platforms/defaults", () => ({
  getProtocolLabel: async (p: string) => p,
  getProtocolLabelMap: async () => ({}),
  getProtocolColorMap: async () => ({}),
  buildProtocolsFromPresets: async () => [],
}));
vi.mock("./platforms/formSections", () => ({
  FormSection: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ApiKeyField: () => null,
  DevinConfigSection: () => null,
  PassthroughConfigSection: () => null,
  PassThroughSection: () => null,
  EndpointsSection: () => null,
  BreakerSection: () => null,
  PeakSection: () => null,
  RateLimitsSection: () => null,
  GroupAssignSection: () => null,
  ExpirySection: () => null,
  QuotaSection: () => null,
}));
vi.mock("./platforms/ModelsMatrixSection", () => ({ ModelsMatrixSection: () => null }));
vi.mock("./platforms/CcMitmAccessSection", () => ({ CcMitmAccessSection: () => null }));
vi.mock("./platforms/PlatformListView", () => ({ PlatformListView: () => null }));

let mockS: PlatformsState;
vi.mock("./platforms/usePlatformsState", () => ({
  usePlatformsState: () => mockS,
}));

function makeState(mode: "create" | "edit" | "duplicate"): PlatformsState {
  return {
    form: {
      editing: mode === "edit" ? { name: "Existing", platform_type: "openai", endpoints: [] } : null,
      duplicating: mode === "duplicate",
      showForm: true,
      name: mode === "create" ? "" : "Existing",
      protocol: "openai",
      endpoints: [{ protocol: "openai", base_url: "https://example.test" }],
      apiKey: "key", quotaVariants: [], models: {}, availableModels: [],
      manualBudgets: [], rateLimits: {},
      timemodels: {
        peak: [], setPeak: vi.fn(), windowsTz: "local", setWindowsTz: vi.fn(),
        disableDuringPeak: false, setDisableDuringPeak: vi.fn(), timeModels: [], setTimeModels: vi.fn(),
        directModels: [], setDirectModels: vi.fn(),
      },
      joinGroupIds: [],
    },
    list: { groupDetails: [] },
    // drag / quota 仅被 actionsRef 引用属性（值可为 undefined），给空对象即可
    drag: {}, quota: {},
    getPrimaryBaseUrl: () => "",
  } as unknown as PlatformsState;
}

describe("Platforms form routing (2026-10-28 拍板)", () => {
  it("create renders full-page form: no drawer footer, has back button", () => {
    mockS = makeState("create");
    render(<Platforms />);
    expect(screen.queryByTestId("platform-edit-drawer")).not.toBeInTheDocument();
    expect(screen.queryByTestId("platform-drawer-footer")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /back/i })).toBeInTheDocument();
  });

  it("edit renders inside drawer with drawer footer", () => {
    mockS = makeState("edit");
    render(<Platforms />);
    expect(screen.getByTestId("platform-edit-drawer")).toBeInTheDocument();
    expect(screen.getByTestId("platform-drawer-footer")).toBeInTheDocument();
  });

  it("duplicate renders inside drawer even though editing is null", () => {
    mockS = makeState("duplicate");
    render(<Platforms />);
    expect(screen.getByTestId("platform-drawer-footer")).toBeInTheDocument();
  });
});
