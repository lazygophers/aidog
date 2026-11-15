import { invoke } from "../transport";
import { setHolidayCalendarData, type HolidayCalendarYears } from "../../utils/timeWindow";

export type HolidayLocale = "en-US" | "zh-Hans" | "ar-SA" | "fr-FR" | "de-DE" | "ru-RU" | "ja-JP" | "es-ES";
export type HolidayCalendarSummary = { code: string; name: Record<HolidayLocale, string> };
export type HolidayCalendar = HolidayCalendarSummary & {
  extends?: string;
  source_urls?: string[];
  years: HolidayCalendarYears;
};

let listPromise: Promise<HolidayCalendarSummary[]> | null = null;
const calendars = new Map<string, Promise<HolidayCalendar>>();

export function holidayCalendarList(): Promise<HolidayCalendarSummary[]> {
  return listPromise ??= invoke<HolidayCalendarSummary[]>("holiday_calendar_list");
}

export function holidayCalendarGet(code: string): Promise<HolidayCalendar> {
  let promise = calendars.get(code);
  if (!promise) {
    promise = invoke<HolidayCalendar>("holiday_calendar_get", { code }).then(calendar => {
      setHolidayCalendarData(code, calendar.years);
      return calendar;
    }).catch(error => {
      calendars.delete(code);
      throw error;
    });
    calendars.set(code, promise);
  }
  return promise;
}
