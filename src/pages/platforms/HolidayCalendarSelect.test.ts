import { describe, expect, it } from "vitest";
import type { HolidayCalendarSummary } from "../../services/api/holidays";
import { holidayCalendarTreeLabels } from "./HolidayCalendarSelect";

const names = (en: string, zh: string) => ({
  "en-US": en, "zh-Hans": zh, "ar-SA": en, "fr-FR": en,
  "de-DE": en, "ru-RU": en, "ja-JP": en, "es-ES": en,
});

const calendars: HolidayCalendarSummary[] = [
  { code: "US", name: names("United States", "美国") },
  { code: "US-CA", name: names("California", "加利福尼亚") },
  { code: "US-CA-SF", name: names("San Francisco", "旧金山") },
  { code: "DE", name: names("Germany", "德国") },
];

describe("holidayCalendarTreeLabels", () => {
  it("builds localized full paths from the available ISO code tree", () => {
    const labels = holidayCalendarTreeLabels(calendars, "en-US");
    expect(labels.get("US-CA-SF")).toBe("United States › California › San Francisco (US-CA-SF)");
    expect(labels.get("DE")).toBe("Germany (DE)");
  });

  it("uses the selected locale for every tree level", () => {
    const labels = holidayCalendarTreeLabels(calendars, "zh-Hans");
    expect(labels.get("US-CA")).toBe("美国 › 加利福尼亚 (US-CA)");
  });
});
