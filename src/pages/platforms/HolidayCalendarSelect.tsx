import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { holidayCalendarGet, holidayCalendarList, type HolidayCalendarSummary } from "../../services/api/holidays";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";

export function holidayCalendarTreeLabels(calendars: HolidayCalendarSummary[], locale: keyof HolidayCalendarSummary["name"]) {
  const byCode = new Map(calendars.map(calendar => [calendar.code, calendar]));
  return new Map(calendars.map(calendar => {
    const names = [calendar.name[locale] ?? calendar.name["en-US"] ?? calendar.code];
    const seen = new Set([calendar.code]);
    let parentCode = calendar.code.slice(0, calendar.code.lastIndexOf("-"));
    while (parentCode && byCode.has(parentCode) && !seen.has(parentCode)) {
      seen.add(parentCode);
      const parent = byCode.get(parentCode)!;
      names.unshift(parent.name[locale] ?? parent.name["en-US"] ?? parent.code);
      parentCode = parent.code.slice(0, parent.code.lastIndexOf("-"));
    }
    return [calendar.code, `${names.join(" › ")} (${calendar.code})`];
  }));
}

export function HolidayCalendarSelect({ value, inheritedValue, timezone, onChange, t, allowInherited = true }: {
  value?: string;
  inheritedValue?: string;
  timezone?: string;
  onChange: (value: string | undefined) => void;
  t: TFunction;
  allowInherited?: boolean;
}) {
  const { i18n } = useTranslation();
  const [calendars, setCalendars] = useState<HolidayCalendarSummary[]>([]);
  const [failed, setFailed] = useState(false);
  const [missingYear, setMissingYear] = useState(false);
  const locale = (i18n.language === "zh" ? "zh-Hans" : i18n.language) as keyof HolidayCalendarSummary["name"];

  useEffect(() => {
    let cancelled = false;
    holidayCalendarList().then(items => { if (!cancelled) setCalendars(items); })
      .catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const code = value ?? inheritedValue;
    if (!code) return;
    setMissingYear(false);
    holidayCalendarGet(code).then(calendar => {
      let year: string;
      try { year = new Intl.DateTimeFormat("en", { timeZone: timezone || "UTC", year: "numeric" }).format(new Date()); }
      catch { year = new Intl.DateTimeFormat("en", { timeZone: "UTC", year: "numeric" }).format(new Date()); }
      setMissingYear(!calendar.years[year]);
    }).catch(() => setFailed(true));
  }, [value, inheritedValue, timezone]);

  const calendarLabels = holidayCalendarTreeLabels(calendars, locale);
  const inherited = "__inherited__";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
      <Select value={value ?? (allowInherited ? inherited : "")} onValueChange={v => onChange(v === inherited ? undefined : v)}>
        <SelectTrigger className="input" style={{ width: 220, height: 30 }} aria-label={t("platform.holidayCalendar", "节假日日历")}>
          <SelectValue placeholder={t("platform.holidayCalendar", "节假日日历")} />
        </SelectTrigger>
        <SelectContent>
          {allowInherited && <SelectItem value={inherited}>{t("platform.holidayCalendar.inherit", "继承上级日历")}{inheritedValue ? ` (${calendars.find(c => c.code === inheritedValue)?.name[locale] ?? inheritedValue})` : ""}</SelectItem>}
          {calendars.map(calendar => (
            <SelectItem key={calendar.code} value={calendar.code}>
              {calendarLabels.get(calendar.code)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {failed && <span role="status" style={{ fontSize: 14, color: "var(--color-warning)" }}>{t("platform.holidayCalendar.loadFailed", "日历数据加载失败，将按星期判断")}</span>}
      {!failed && missingYear && <span role="status" style={{ fontSize: 14, color: "var(--color-warning)" }}>{t("platform.holidayCalendar.yearUnavailable", "日历未覆盖当前年份，将按星期判断")}</span>}
    </div>
  );
}
