import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { PlatformsState } from "./usePlatformsState";
import { PlatformEditForm } from "./PlatformEditForm";
import { emptyDraft } from "../../domains/platforms/platformDraft";

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
  getDefaultHolidayCalendar: async () => undefined,
}));
vi.mock("./formSections", () => ({
  FormSection: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ApiKeyField: () => null,
  DevinConfigSection: () => null,
  PassthroughConfigSection: () => null,
  PassThroughSection: () => null,
  EndpointsSection: () => null,
  BreakerSection: () => <div data-testid="breaker-section" />,
  PeakSection: () => null,
  RateLimitsSection: () => null,
  GroupAssignSection: () => null,
  ExpirySection: () => null,
  QuotaSection: () => null,
  ClientWhitelistSection: () => null,
  ProxyEnabledSection: () => null,
}));
vi.mock("../../services/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/api")>()),
  getClientVocab: async () => [],
}));
vi.mock("./ModelsMatrixSection", () => ({ ModelsMatrixSection: () => null }));
vi.mock("./CcMitmAccessSection", () => ({ CcMitmAccessSection: () => null }));

function state(original: "unknown" | "openai", protocol: string = original): PlatformsState {
  return {
    form: {
      editing: { name: "Existing", platform_type: original, endpoints: [] },
      // 票 14：控件读 draft、写 patch（stub 用 emptyDraft 真形状 + patch spy）。
      draft: {
        ...emptyDraft(),
        name: "Existing", protocol,
        apiKey: "key",
        endpoints: [{ protocol: "openai", base_url: "https://example.test" }],
      },
      patch: vi.fn(),
      showPaste: false, pasteInitialText: undefined, setShowPaste: vi.fn(), setPasteInitialText: vi.fn(),
      isMock: false, isPassthrough: false, isPassThrough: false, keyOptional: false, apiKeyMissing: false,
      showKey: false, setShowKey: vi.fn(),
      batchPreviewKeys: null, handleApiKeyChange: vi.fn(), previewNames: [],
      activeDropdown: null, setActiveDropdown: vi.fn(),
      quotaVariants: [], handleQuotaVariantChange: vi.fn(), setQuotaSourceExplicit: vi.fn(),
      breakerDefaults: null, windowsTz: "local", setWindowsTz: vi.fn(),
      expiryEnabled: false, setExpiryEnabled: vi.fn(),
      customSlotDefs: null,
      fetching: false, fetchError: "", fetchInfo: "", handleFetchModels: vi.fn(), handleFillAll: vi.fn(),
      saveError: "",
      handleSave: vi.fn(), resetForm: vi.fn(), applyPaste: vi.fn(),
      handleModelsChange: vi.fn(), handleProtocolChange: vi.fn(),
    },
    list: { groupDetails: [] },
    getPrimaryBaseUrl: () => "",
  } as unknown as PlatformsState;
}

describe("subscription date", () => {
  it("blocks an invalid imported date and allows clearing it", () => {
    const s = state("openai", "qianfan_coding");
    const patch = vi.fn();
    Object.assign(s.form, {
      draft: { ...emptyDraft(), protocol: "qianfan_coding", extra: '{"other":1,"subscription_start_date":"9999-01-01"}' },
      patch,
    });
    render(<PlatformEditForm s={s} />);
    expect(screen.getByRole("button", { name: "action.save" })).toBeDisabled();
    expect(screen.getByRole("alert")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("subscription-date-clear"));
    // patch({ extra: serializeSubscriptionStartDate(draft.extra, "") }) —— 值已按 draft 解析
    expect(JSON.parse(patch.mock.calls[0][0].extra)).toEqual({ other: 1 });
  });

  it("shows only for qianfan coding and writes a clearable date", () => {
    const s = state("openai", "qianfan_coding");
    const patch = vi.fn();
    Object.assign(s.form, { draft: { ...emptyDraft(), protocol: "qianfan_coding", extra: '{"other":1}' }, patch });
    const view = render(<PlatformEditForm s={s} />);
    const input = screen.getByTestId("subscription-start-date");
    fireEvent.change(input, { target: { value: "2026-01-31" } });
    expect(JSON.parse(patch.mock.calls[0][0].extra)).toEqual({ other: 1, subscription_start_date: "2026-01-31" });
    Object.assign(s.form, { draft: { ...emptyDraft(), protocol: "qianfan_coding", extra: '{"other":1,"subscription_start_date":"2026-01-31"}' } });
    view.rerender(<PlatformEditForm s={s} />);
    fireEvent.change(input, { target: { value: "" } });
    expect(JSON.parse(patch.mock.calls[1][0].extra)).toEqual({ other: 1 });
    view.rerender(<PlatformEditForm s={state("openai")} />);
    expect(screen.queryByTestId("subscription-start-date")).not.toBeInTheDocument();
  });
});

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
      "Quota Query", "Time-based models", "Peak hours multiplier", "Direct models", "Rate Limits", "Circuit breaker", "Client allowlist", "Expires at", "Group Assignment",
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
    expect(screen.getAllByRole("tab").map(b => b.textContent)).toEqual(["Client allowlist", "Expires at"]);
  });
});

describe("PlatformEditForm advanced tabs visibility matrix (spec)", () => {
  const labels = () => screen.getAllByRole("tab").map(b => b.textContent);

  it("create state matches edit tabs and can open their sections", () => {
    const s = state("openai");
    (s.form as { editing: unknown }).editing = null;
    render(<PlatformEditForm s={s} />);
    expect(labels()).toEqual([
      "Quota Query", "Time-based models", "Peak hours multiplier", "Direct models", "Rate Limits", "Circuit breaker", "Client allowlist", "Expires at", "Group Assignment",
    ]);
    fireEvent.click(screen.getByRole("tab", { name: "Circuit breaker" }));
    expect(screen.getByTestId("breaker-section")).toBeInTheDocument();
  });

  it("mock platform hides quota/timemodels/peak, keeps ratelimits/breaker/expiry/group", () => {
    const s = state("openai");
    (s.form as { isMock: boolean }).isMock = true;
    render(<PlatformEditForm s={s} />);
    expect(labels()).toEqual(["Rate Limits", "Circuit breaker", "Expires at", "Group Assignment"]);
  });

  it("passThrough platform shows only clients + expiry + group tabs", () => {
    const s = state("openai");
    (s.form as { isPassThrough: boolean }).isPassThrough = true;
    render(<PlatformEditForm s={s} />);
    expect(labels()).toEqual(["Client allowlist", "Expires at", "Group Assignment"]);
  });
});
