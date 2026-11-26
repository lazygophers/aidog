import { expect, it, vi } from "vitest";
import { fireEvent, render, screen, testI18n, waitFor } from "../../test/render";
import { modelInfoApi } from "../../services/api/pricing";
import { ModelCombobox } from "./ModelCombobox";

vi.mock("../../services/api/pricing", () => ({
  modelInfoApi: { resolvePrices: vi.fn() },
  onRegistryUpdated: () => () => {},
}));
vi.mock("../../utils/displayCurrency", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../utils/displayCurrency")>(),
  useDisplayCurrency: () => {},
}));

it("requests the complete dynamic model ID and displays the returned price", async () => {
  testI18n.addResource("zh-Hans", "translation", "common.priceFromPlatform", "from {{platform}}");
  vi.mocked(modelInfoApi.resolvePrices).mockResolvedValue([{
    platform_code: "openai",
    price_data: JSON.stringify({ price: { input: 1e-6 } }),
  } as never]);
  const id = "codex/dynamic-model";
  render(<ModelCombobox value="" onChange={() => {}} onSelect={() => {}} source={[id]}
    protocol={"magpie" as never} t={((key: string) => key) as never} inputProps={{ "data-testid": "model-input" }} />);
  fireEvent.focus(screen.getByTestId("model-input"));
  const option = await screen.findByTestId(`model-option-${id}`);
  await waitFor(() => expect(option.textContent).toContain("in $1.00 /M"));
  expect(modelInfoApi.resolvePrices).toHaveBeenCalledWith("magpie", [id]);
  expect(option.textContent).toContain("openai");
});
