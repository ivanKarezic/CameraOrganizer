import { useEffect, useMemo, useRef, useState } from "react";
import {
  calendarDays,
  formatDayLabel,
  formatMonthLabel,
  monthForSelection,
  pickRangeDay,
  stepMonth,
  uniqueMonths,
  uniqueYears,
  WEEKDAYS,
} from "./lib/dates";

export function DateRangePicker({
  days,
  dateFrom,
  dateTo,
  onChange,
}: {
  days: string[];
  dateFrom?: string | null;
  dateTo?: string | null;
  onChange: (next: { dateFrom: string | null; dateTo: string | null }) => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const months = useMemo(() => uniqueMonths(days), [days]);
  const years = useMemo(() => uniqueYears(days), [days]);
  const available = useMemo(() => new Set(days), [days]);
  const [month, setMonth] = useState(() => monthForSelection(days, dateFrom, dateTo));

  useEffect(() => {
    if (!open) return;
    setMonth(monthForSelection(days, dateFrom, dateTo));
  }, [open, days, dateFrom, dateTo]);

  useEffect(() => {
    if (!open) return;
    function onPointer(event: MouseEvent) {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    window.addEventListener("mousedown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const cells = month ? calendarDays(month) : [];
  const year = month.slice(0, 4);
  const atStart = months[0] === month;
  const atEnd = months[months.length - 1] === month;
  const fromLabel = dateFrom ? formatDayLabel(dateFrom) : "Any";
  const toLabel = dateTo ? formatDayLabel(dateTo) : "Any";

  return (
    <div className={open ? "date-range open" : "date-range"} ref={root}>
      <button
        type="button"
        className="date-range-field"
        disabled={days.length === 0}
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-haspopup="dialog"
      >
        <span className="date-range-side">
          <span className="date-range-kicker">From</span>
          <span className="date-range-value">{days.length ? fromLabel : "—"}</span>
        </span>
        <span className="date-range-rule" aria-hidden>
          →
        </span>
        <span className="date-range-side">
          <span className="date-range-kicker">To</span>
          <span className="date-range-value">{days.length ? toLabel : "—"}</span>
        </span>
      </button>
      {dateFrom || dateTo ? (
        <button
          type="button"
          className="date-range-clear"
          aria-label="Clear dates"
          onClick={() => onChange({ dateFrom: null, dateTo: null })}
        >
          ×
        </button>
      ) : null}
      {open && month ? (
        <div className="date-cal" role="dialog" aria-label="Library dates">
          <div className="date-cal-head">
            <button
              type="button"
              className="date-cal-shift"
              disabled={atStart}
              onClick={() => setMonth(stepMonth(months, month, -1))}
              aria-label="Previous month with files"
            >
              ‹
            </button>
            <div className="date-cal-label">
              <span>{formatMonthLabel(month)}</span>
              <select
                value={year}
                onChange={(event) => {
                  const nextYear = event.target.value;
                  const sameMonth = `${nextYear}-${month.slice(5)}`;
                  setMonth(
                    months.includes(sameMonth)
                      ? sameMonth
                      : (months.find((entry) => entry.startsWith(nextYear)) ?? month),
                  );
                }}
                aria-label="Year"
              >
                {years.map((entry) => (
                  <option key={entry} value={entry}>
                    {entry}
                  </option>
                ))}
              </select>
            </div>
            <button
              type="button"
              className="date-cal-shift"
              disabled={atEnd}
              onClick={() => setMonth(stepMonth(months, month, 1))}
              aria-label="Next month with files"
            >
              ›
            </button>
          </div>
          <div className="date-cal-week">
            {WEEKDAYS.map((day) => (
              <span key={day}>{day}</span>
            ))}
          </div>
          <div className="date-cal-grid">
            {cells.map((day, index) => {
              if (!day) return <span key={`empty-${index}`} />;
              const live = available.has(day);
              const selected = day === dateFrom || day === dateTo;
              const inRange =
                Boolean(dateFrom && dateTo) && day >= dateFrom! && day <= dateTo!;
              const classes = [
                "date-cal-day",
                live ? "live" : "empty",
                selected ? "selected" : "",
                inRange ? "in-range" : "",
                day === dateFrom ? "start" : "",
                day === dateTo ? "end" : "",
              ]
                .filter(Boolean)
                .join(" ");
              return (
                <button
                  key={day}
                  type="button"
                  className={classes}
                  disabled={!live}
                  onClick={() => {
                    const next = pickRangeDay(day, dateFrom, dateTo);
                    onChange(next);
                  }}
                >
                  {Number(day.slice(8))}
                </button>
              );
            })}
          </div>
          <p className="date-cal-hint">Only days with files can be chosen. Click once for a day, twice to set a range.</p>
        </div>
      ) : null}
    </div>
  );
}
