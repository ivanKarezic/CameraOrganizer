import { formatCaptureDate } from "./format";
import type { MediaItem, SearchQuery } from "../types";

export interface KindVisibility {
  photo: boolean;
  video: boolean;
}

export function hasActiveFilters(search: SearchQuery, kinds?: KindVisibility): boolean {
  const kindsNarrowed = kinds ? !kinds.photo || !kinds.video : false;
  return Boolean(
    search.text?.trim() ||
      search.location?.trim() ||
      search.dateFrom ||
      search.dateTo ||
      search.camera ||
      search.tag ||
      search.tagCategory ||
      kindsNarrowed,
  );
}

export function matchesLibraryFilters(
  item: MediaItem,
  search: SearchQuery,
  unorganizedOnly: boolean,
  kinds: KindVisibility = { photo: true, video: true },
): boolean {
  if (item.kind === "sidecar") return false;
  if (item.kind === "photo" && !kinds.photo) return false;
  if (item.kind === "video" && !kinds.video) return false;
  if (unorganizedOnly && item.organized) return false;

  const text = search.text?.trim().toLowerCase();
  if (text) {
    const hay = [
      item.filename,
      item.camera,
      item.locationLabel ?? "",
      ...item.tags.map((tag) => tag.name),
    ]
      .join(" ")
      .toLowerCase();
    if (!hay.includes(text)) return false;
  }

  const location = search.location?.trim().toLowerCase();
  if (location && !(item.locationLabel ?? "").toLowerCase().includes(location)) {
    return false;
  }

  if (search.camera && item.camera !== search.camera) return false;
  if (search.tag && !item.tags.some((tag) => tag.name.toLowerCase() === search.tag!.toLowerCase())) {
    return false;
  }
  if (
    search.tagCategory &&
    !item.tags.some((tag) => tag.categoryName.toLowerCase() === search.tagCategory!.toLowerCase())
  ) {
    return false;
  }

  const day = formatCaptureDate(item.capturedAt);
  if (search.dateFrom && day < search.dateFrom) return false;
  if (search.dateTo && day > search.dateTo) return false;

  return true;
}
