import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, testI18n, waitFor } from "../../test/render";
import { modelInfoApi } from "../../services/api/pricing";
import { ModelCombobox } from "./ModelCombobox";

vi.mock("../../services/api/pricing", () => ({
  modelInfoApi: { snapshot: vi.fn() },
  onRegistryUpdated: () => () => {},
}));
vi.mock("../../utils/displayCurrency", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../utils/displayCurrency")>(),
  useDisplayCurrency: () => {},
}));

const t = ((key: string, fallback?: string) => fallback ?? key) as never;
const price = (input: number) => JSON.stringify({ price: { input } });
const groups = [{
  canonical_model: "shared-model",
  display_name: "Shared model",
  primary_platform: "openai",
  entries: [
    { platform_code: "openai", model_id: "a-package", official: true, price_data: price(9e-6) },
    { platform_code: "openai", model_id: "shared-model", official: true, price_data: price(1e-6) },
    { platform_code: "codex", model_id: "shared-model", official: false, price_data: "{}" },
  ],
}] as unknown as import("../../services/api/types/generated/ModelEntryGroup").ModelEntryGroup[];

async function showPrice(protocol: "openai" | "codex") {
  const { container } = render(
    <ModelCombobox value="" onChange={() => {}} onSelect={() => {}} source={["shared-model"]}
      protocol={protocol} t={t} inputProps={{ "data-testid": `input-${protocol}` }} />,
  );
  fireEvent.focus(container.querySelector(`[data-testid="input-${protocol}"]`)!);
  const option = await screen.findByTestId("model-option-shared-model");
  await waitFor(() => expect(option.textContent).toContain("in $"));
  return option.textContent;
}

afterEach(() => {
  testI18n.removeResourceBundle("zh-Hans", "translation");
  vi.clearAllMocks();
});

describe("ModelCombobox canonical price rendering", () => {
  it("uses canonical representative within primary platform and marks cross-platform fallback", async () => {
    testI18n.addResource("zh-Hans", "translation", "common.priceFromPlatform", "price from {{platform}}");
    vi.mocked(modelInfoApi.snapshot).mockResolvedValue({
      groups, platforms: [], pricing_only: [], bundled: false,
    });

    const ownPlatformPrice = await showPrice("openai");
    expect(ownPlatformPrice).toContain("in $1.00 /M");
    expect(ownPlatformPrice).not.toContain("price from");
    expect(await showPrice("codex")).toContain("in $1.00 /M");
    expect(await showPrice("codex")).toContain("from openai");
  });
});
