import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import type { PlatformsState } from "./usePlatformsState";
import { PlatformEditForm } from "./PlatformEditForm";

vi.mock("../../domains/platforms", () => ({
  SearchableProtocolSelect: ({ value }: { value: string }) => <select aria-label="protocol" value={value} onChange={() => {}}><option value={value}>{value}</option></select>,
  MockConfigEditor: () => null,
  allConfiguredModelValues: () => [],
}));
vi.mock("../../domains/platforms/defaults", () => ({
  getProtocolLabel: async (p: string) => p,
  getProtocolLabelMap: async () => ({}),
  getProtocolColorMap: async () => ({}),
  buildProtocolsFromPresets: async () => [],
  getProtocolUsageWarning: async (p: string) => (p === "risky" ? "RISK-TEXT" : ""),
}));
vi.mock("./formSections", () => ({
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
  ClientWhitelistSection: () => null,
}));
vi.mock("../../services/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/api")>()),
  getClientVocab: async () => [],
}));
vi.mock("./ModelsMatrixSection", () => ({ ModelsMatrixSection: () => null }));
vi.mock("./CcMitmAccessSection", () => ({ CcMitmAccessSection: () => null }));

function state(original: "unknown" | "openai", protocol = original): PlatformsState {
  return {
    form: {
      editing: { name: "Existing", platform_type: original, endpoints: [] },
      name: "Existing", protocol, endpoints: [{ protocol: "openai", base_url: "https://example.test" }],
      apiKey: "key", quotaVariants: [], models: {}, availableModels: [],
      manualBudgets: [], timemodels: {
        peak: [], setPeak: vi.fn(), windowsTz: "local", setWindowsTz: vi.fn(),
        disableDuringPeak: false, setDisableDuringPeak: vi.fn(), timeModels: [], setTimeModels: vi.fn(),
      }, joinGroupIds: [], rateLimits: {},
    },
    list: { groupDetails: [] },
    getPrimaryBaseUrl: () => "",
  } as unknown as PlatformsState;
}

describe("PlatformEditForm unknown protocol", () => {
  it("keeps the selector available after repairing the original unknown protocol", async () => {
    const { rerender } = render(<PlatformEditForm s={state("unknown")} />);
    expect(screen.getByRole("combobox", { name: "protocol" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "action.save" })).toBeDisabled();
    expect(screen.getByRole("alert")).toBeInTheDocument();
    rerender(<PlatformEditForm s={state("unknown", "openai")} />);
    await waitFor(() => expect(screen.getByRole("combobox", { name: "protocol" })).toHaveValue("openai"));
    expect(screen.getByRole("button", { name: "action.save" })).toBeEnabled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the protocol selector when editing a known platform", async () => {
    render(<PlatformEditForm s={state("openai")} />);
    await waitFor(() => expect(screen.getByRole("combobox", { name: "protocol" })).toHaveValue("openai"));
  });
});

describe("PlatformEditForm usage warning", () => {
  it("renders the preset usage warning in an alert when the protocol has one", async () => {
    render(<PlatformEditForm s={state("openai", "risky" as "openai")} />);
    const el = await screen.findByTestId("platform-usage-warning");
    expect(el).toHaveTextContent("RISK-TEXT");
    expect(el).toHaveAttribute("role", "alert");
  });
  it("renders no warning for a protocol without one", async () => {
    render(<PlatformEditForm s={state("openai")} />);
    await waitFor(() => expect(screen.getByRole("combobox", { name: "protocol" })).toHaveValue("openai"));
    expect(screen.queryByTestId("platform-usage-warning")).not.toBeInTheDocument();
  });
});

describe("PlatformEditForm advanced tabs (8-tab split)", () => {
  it("shows 8 tabs in order with quota first selected, old labels gone", () => {
    render(<PlatformEditForm s={state("openai")} />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map(b => b.textContent)).toEqual([
      "配额查询", "时段档", "高峰时段倍率", "直通模型", "限频", "熔断", "客户端白名单", "过期时间", "分组归属",
    ]);
    expect(tabs[0]).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByText("计费")).not.toBeInTheDocument();
    expect(screen.queryByText("稳定性")).not.toBeInTheDocument();
    expect(screen.queryByText("生命周期")).not.toBeInTheDocument();
  });

  it("passthrough shows only clients + expiry tabs (透传同受白名单约束)", () => {
    const s = state("openai");
    (s.form as { isPassthrough: boolean }).isPassthrough = true;
    render(<PlatformEditForm s={s} />);
    expect(screen.getAllByRole("tab").map(b => b.textContent)).toEqual(["客户端白名单", "过期时间"]);
  });
});

describe("PlatformEditForm advanced tabs visibility matrix (spec)", () => {
  const labels = () => screen.getAllByRole("tab").map(b => b.textContent);

  it("create state (editing=null) shows only the 5 non-editing tabs", () => {
    const s = state("openai");
    (s.form as { editing: unknown }).editing = null;
    render(<PlatformEditForm s={s} />);
    expect(labels()).toEqual(["配额查询", "时段档", "客户端白名单", "过期时间", "分组归属"]);
  });

  it("mock platform hides quota/timemodels/peak, keeps ratelimits/breaker/expiry/group", () => {
    const s = state("openai");
    (s.form as { isMock: boolean }).isMock = true;
    render(<PlatformEditForm s={s} />);
    expect(labels()).toEqual(["限频", "熔断", "过期时间", "分组归属"]);
  });

  it("passThrough platform shows only clients + expiry + group tabs", () => {
    const s = state("openai");
    (s.form as { isPassThrough: boolean }).isPassThrough = true;
    render(<PlatformEditForm s={s} />);
    expect(labels()).toEqual(["客户端白名单", "过期时间", "分组归属"]);
  });
});
