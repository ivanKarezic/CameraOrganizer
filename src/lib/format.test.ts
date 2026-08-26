import { describe, expect, it } from "vitest";
import { formatBytes, formatCaptureDate, groupByDate } from "./format";

describe("formatBytes", () => {
  it("uses compact units", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
  });
});

describe("formatCaptureDate", () => {
  it("keeps ISO calendar dates", () => {
    expect(formatCaptureDate("2024-08-26T14:30:22")).toBe("2024-08-26");
  });
});

describe("groupByDate", () => {
  it("buckets items by day", () => {
    const groups = groupByDate([
      { capturedAt: "2024-08-26T10:00:00" },
      { capturedAt: "2024-08-26T18:00:00" },
      { capturedAt: "2024-08-27T09:00:00" },
    ]);
    expect(groups.get("2024-08-26")).toHaveLength(2);
    expect(groups.get("2024-08-27")).toHaveLength(1);
  });
});
