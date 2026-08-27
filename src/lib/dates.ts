import { formatCaptureDate } from "./format";

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
];

const MONTHS_SHORT = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

export const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

export function uniqueCaptureDays(capturedAt: Iterable<string>): string[] {
  const days = new Set<string>();
  for (const iso of capturedAt) {
    const day = formatCaptureDate(iso);
    if (/^\d{4}-\d{2}-\d{2}$/.test(day)) days.add(day);
  }
  return [...days].sort();
}

export function monthKey(day: string): string {
  return day.slice(0, 7);
}

export function uniqueMonths(days: string[]): string[] {
  return [...new Set(days.map(monthKey))].sort();
}

export function uniqueYears(days: string[]): string[] {
  return [...new Set(days.map((day) => day.slice(0, 4)))].sort();
}

export function formatDayLabel(day: string): string {
  const [year, month, date] = day.split("-").map(Number);
  return `${date} ${MONTHS_SHORT[(month ?? 1) - 1]} ${year}`;
}

export function formatMonthLabel(month: string): string {
  const [year, monthNum] = month.split("-").map(Number);
  return `${MONTHS[(monthNum ?? 1) - 1]} ${year}`;
}

export function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

export function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

export function mondayIndex(year: number, month: number, day: number): number {
  const weekday = new Date(Date.UTC(year, month - 1, day)).getUTCDay();
  return (weekday + 6) % 7;
}

export function calendarDays(month: string): Array<string | null> {
  const [year, monthNum] = month.split("-").map(Number);
  if (!year || !monthNum) return [];
  const lead = mondayIndex(year, monthNum, 1);
  const count = daysInMonth(year, monthNum);
  const cells: Array<string | null> = Array.from({ length: lead }, () => null);
  for (let day = 1; day <= count; day += 1) {
    cells.push(`${year}-${pad2(monthNum)}-${pad2(day)}`);
  }
  while (cells.length % 7 !== 0) cells.push(null);
  return cells;
}

export function stepMonth(months: string[], current: string, delta: number): string {
  if (months.length === 0) return current;
  const index = months.indexOf(current);
  if (index < 0) return delta >= 0 ? months[0] : months[months.length - 1];
  return months[Math.min(months.length - 1, Math.max(0, index + delta))];
}

export function monthForSelection(
  days: string[],
  dateFrom?: string | null,
  dateTo?: string | null,
): string {
  const months = uniqueMonths(days);
  if (dateFrom && months.includes(monthKey(dateFrom))) return monthKey(dateFrom);
  if (dateTo && months.includes(monthKey(dateTo))) return monthKey(dateTo);
  return months[months.length - 1] ?? "";
}

export function clampToDays(day: string | null | undefined, days: string[]): string | null {
  if (!day || days.length === 0) return null;
  if (days.includes(day)) return day;
  if (day < days[0]) return days[0];
  if (day > days[days.length - 1]) return days[days.length - 1];
  return days.find((candidate) => candidate >= day) ?? days[days.length - 1];
}

export function pickRangeDay(
  day: string,
  dateFrom: string | null | undefined,
  dateTo: string | null | undefined,
): { dateFrom: string; dateTo: string } {
  const from = dateFrom || null;
  const to = dateTo || null;
  if (!from || !to || from !== to) {
    return { dateFrom: day, dateTo: day };
  }
  if (day < from) return { dateFrom: day, dateTo: from };
  return { dateFrom: from, dateTo: day };
}

export function dayInRange(
  day: string,
  dateFrom: string | null | undefined,
  dateTo: string | null | undefined,
): boolean {
  if (!dateFrom && !dateTo) return false;
  const start = dateFrom ?? dateTo ?? day;
  const end = dateTo ?? dateFrom ?? day;
  return day >= start && day <= end;
}

export function wrappedIndex(index: number, delta: number, length: number): number {
  if (length <= 0) return -1;
  if (index < 0) return delta >= 0 ? 0 : length - 1;
  return (index + delta + length) % length;
}
