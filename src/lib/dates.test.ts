import { describe, expect, it } from "vitest";
import {
  calendarDays,
  clampToDays,
  mondayIndex,
  monthForSelection,
  pickRangeDay,
  stepMonth,
  uniqueCaptureDays,
  uniqueMonths,
  wrappedIndex,
} from "./dates";

describe("uniqueCaptureDays", () => {
  it("keeps sorted unique calendar days", () => {
    expect(
      uniqueCaptureDays(["2024-08-27T10:00:00", "2024-08-26T09:00:00", "2024-08-26T18:00:00"]),
    ).toEqual(["2024-08-26", "2024-08-27"]);
  });
});

describe("calendarDays", () => {
  it("starts weeks on Monday and pads the month", () => {
    expect(mondayIndex(2024, 8, 1)).toBe(3);
    const cells = calendarDays("2024-08");
    expect(cells.slice(0, 4)).toEqual([null, null, null, "2024-08-01"]);
    expect(cells).toContain("2024-08-31");
    expect(cells.length % 7).toBe(0);
  });
});

describe("pickRangeDay", () => {
  it("starts with a single day, then expands, then resets", () => {
    expect(pickRangeDay("2024-08-26", null, null)).toEqual({
      dateFrom: "2024-08-26",
      dateTo: "2024-08-26",
    });
    expect(pickRangeDay("2024-08-28", "2024-08-26", "2024-08-26")).toEqual({
      dateFrom: "2024-08-26",
      dateTo: "2024-08-28",
    });
    expect(pickRangeDay("2024-08-20", "2024-08-26", "2024-08-26")).toEqual({
      dateFrom: "2024-08-20",
      dateTo: "2024-08-26",
    });
    expect(pickRangeDay("2024-08-01", "2024-08-26", "2024-08-28")).toEqual({
      dateFrom: "2024-08-01",
      dateTo: "2024-08-01",
    });
  });
});

describe("clampToDays", () => {
  const days = ["2024-08-26", "2024-08-28", "2025-06-12"];
  it("rejects dates outside the library and snaps to the nearest available day", () => {
    expect(clampToDays("2024-01-01", days)).toBe("2024-08-26");
    expect(clampToDays("2026-01-01", days)).toBe("2025-06-12");
    expect(clampToDays("2024-08-27", days)).toBe("2024-08-28");
    expect(clampToDays("2024-08-26", days)).toBe("2024-08-26");
    expect(clampToDays(null, days)).toBeNull();
  });
});

describe("stepMonth", () => {
  it("stays within months that have library files", () => {
    const months = uniqueMonths(["2024-08-26", "2025-06-12"]);
    expect(stepMonth(months, "2024-08", 1)).toBe("2025-06");
    expect(stepMonth(months, "2025-06", 1)).toBe("2025-06");
    expect(stepMonth(months, "2024-08", -1)).toBe("2024-08");
    expect(monthForSelection(["2024-08-26", "2025-06-12"], null, null)).toBe("2025-06");
  });
});

describe("wrappedIndex", () => {
  it("loops at both ends", () => {
    expect(wrappedIndex(0, -1, 4)).toBe(3);
    expect(wrappedIndex(3, 1, 4)).toBe(0);
    expect(wrappedIndex(1, 1, 4)).toBe(2);
    expect(wrappedIndex(-1, 1, 4)).toBe(0);
  });
});
