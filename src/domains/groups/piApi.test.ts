import { describe, it, expect } from "vitest";
import { parseGroupPiApi, PI_API_DEFAULT } from "./piApi";

describe("parseGroupPiApi", () => {
  it("reads the stored protocol", () => {
    expect(parseGroupPiApi('{"pi_api":"openai-responses"}')).toBe("openai-responses");
  });

  it("falls back for old groups with no value, junk, or an unknown protocol", () => {
    expect(parseGroupPiApi("")).toBe(PI_API_DEFAULT);
    expect(parseGroupPiApi('{"_ui_collapsed":true}')).toBe(PI_API_DEFAULT);
    expect(parseGroupPiApi("not json")).toBe(PI_API_DEFAULT);
    expect(parseGroupPiApi('{"pi_api":"nonsense"}')).toBe(PI_API_DEFAULT);
  });
});
