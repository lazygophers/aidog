import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import type { PlatformsState } from "./usePlatformsState";
import { PlatformEditForm } from "./PlatformEditForm";

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

  it("keeps known platform editing locked", async () => {
    render(<PlatformEditForm s={state("openai")} />);
    expect(screen.queryByRole("combobox", { name: "protocol" })).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("Protocol cannot be changed after creation")).toBeInTheDocument());
  });
});
