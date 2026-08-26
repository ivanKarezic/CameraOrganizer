import { describe, expect, it } from "vitest";
import { hasActiveFilters, matchesLibraryFilters } from "./filter";
import type { MediaItem, SearchQuery } from "../types";

function item(overrides: Partial<MediaItem> = {}): MediaItem {
  return {
    id: 1,
    storageId: "s",
    path: "/lib/DJI_0001.JPG",
    filename: "DJI_0001.JPG",
    size: 12,
    hash: null,
    camera: "DJI",
    kind: "photo",
    capturedAt: "2024-08-26T14:30:22",
    dateSource: "filename",
    latitude: 47.1,
    longitude: 8.5,
    locationLabel: "47.1, 8.5",
    organized: true,
    tags: ["alps"],
    ...overrides,
  };
}

describe("matchesLibraryFilters", () => {
  it("hides sidecars and organized files when asked", () => {
    expect(matchesLibraryFilters(item({ kind: "sidecar" }), {}, false)).toBe(false);
    expect(matchesLibraryFilters(item({ organized: true }), {}, true)).toBe(false);
    expect(matchesLibraryFilters(item({ organized: false }), {}, true)).toBe(true);
  });

  it("filters by text, tag, camera, location, and date", () => {
    const shot = item();
    expect(matchesLibraryFilters(shot, { text: "dji_0001" }, false)).toBe(true);
    expect(matchesLibraryFilters(shot, { text: "gopro" }, false)).toBe(false);
    expect(matchesLibraryFilters(shot, { tag: "alps" }, false)).toBe(true);
    expect(matchesLibraryFilters(shot, { tag: "ocean" }, false)).toBe(false);
    expect(matchesLibraryFilters(shot, { camera: "GoPro" }, false)).toBe(false);
    expect(matchesLibraryFilters(shot, { location: "47.1" }, false)).toBe(true);
    expect(matchesLibraryFilters(shot, { dateFrom: "2024-08-27" }, false)).toBe(false);
    expect(matchesLibraryFilters(shot, { dateTo: "2024-08-26" }, false)).toBe(true);
  });

  it("shows only ticked media kinds", () => {
    const photo = item({ kind: "photo" });
    const video = item({ kind: "video", filename: "clip.MP4" });
    expect(matchesLibraryFilters(photo, {}, false, { photo: true, video: false })).toBe(true);
    expect(matchesLibraryFilters(video, {}, false, { photo: true, video: false })).toBe(false);
    expect(matchesLibraryFilters(photo, {}, false, { photo: false, video: true })).toBe(false);
    expect(matchesLibraryFilters(video, {}, false, { photo: false, video: true })).toBe(true);
    expect(matchesLibraryFilters(photo, {}, false, { photo: false, video: false })).toBe(false);
  });
});

describe("hasActiveFilters", () => {
  it("is false for an empty query", () => {
    expect(hasActiveFilters({} as SearchQuery)).toBe(false);
    expect(hasActiveFilters({ text: "  " })).toBe(false);
  });

  it("is true when any field is set", () => {
    expect(hasActiveFilters({ camera: "DJI" })).toBe(true);
    expect(hasActiveFilters({}, { photo: true, video: false })).toBe(true);
    expect(hasActiveFilters({}, { photo: true, video: true })).toBe(false);
  });
});
