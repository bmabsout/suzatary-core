/**
 * Day separators for a transcript that spans days, on the owner's days. A day is printed from its own `YYYY-MM-DD`,
 * never through a zoned `Date` that could move it; `Date.UTC` only names the weekday.
 */

import time from "../app/time";

const WEEKDAYS = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
] as const;

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
] as const;

/** `2026-09-14T09:00:00` → `Monday 14 September`; "" when the leading `YYYY-MM-DD` does not parse. */
export function dayLabel(at: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(at);
  if (match === null) return "";
  const [, yearText, monthText, dayText] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const weekday = WEEKDAYS[new Date(Date.UTC(year, month - 1, day)).getUTCDay()];
  const name = MONTHS[month - 1];
  if (weekday === undefined || name === undefined) return "";
  return `${weekday} ${day} ${name}`;
}

export interface Dated {
  readonly key: number;
  readonly at: string;
}

/** A separator above each item whose owner's day differs from the previous dated item's; an undated item neither gets one nor resets the previous day. */
export function daySeparators(items: readonly Dated[], zone: time.Zone): ReadonlyMap<number, string> {
  const marks = new Map<number, string>();
  let lastDay = "";
  for (const item of items) {
    const at = time.ms(item.at);
    const today = at === null ? "" : time.date(zone, at);
    if (today === "" || today === lastDay) continue;
    lastDay = today;
    marks.set(item.key, dayLabel(today));
  }
  return marks;
}
