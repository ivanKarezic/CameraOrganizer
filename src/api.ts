import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppConfig,
  GlobalTag,
  JobProgress,
  MediaItem,
  MediaRef,
  MediaTag,
  SavedLocation,
  SearchQuery,
  TagCategory,
  TransferOp,
} from "./types";

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function saveConfig(config: AppConfig): Promise<AppConfig> {
  return invoke("save_config", { config });
}

export async function scanLibrary(): Promise<MediaItem[]> {
  return invoke("scan_library");
}

export async function searchMedia(query: SearchQuery): Promise<MediaItem[]> {
  return invoke("search_media", { query });
}

export async function listTags(): Promise<GlobalTag[]> {
  return invoke("list_tags");
}

export async function listTagCategories(): Promise<TagCategory[]> {
  return invoke("list_tag_categories");
}

export async function saveTagCategory(category: {
  id?: string | null;
  name: string;
  color: string;
}): Promise<TagCategory> {
  return invoke("save_tag_category", { category });
}

export async function deleteTagCategory(categoryId: string): Promise<void> {
  return invoke("delete_tag_category", { categoryId });
}

export async function saveGlobalTag(tag: {
  id?: string | null;
  name: string;
  categoryId: string;
}): Promise<GlobalTag> {
  return invoke("save_global_tag", { tag });
}

export async function deleteGlobalTag(tagId: string): Promise<void> {
  return invoke("delete_global_tag", { tagId });
}

export async function setMediaTags(
  storageId: string,
  mediaId: number,
  tags: string[],
): Promise<MediaTag[]> {
  return invoke("set_media_tags", { storageId, mediaId, tags });
}

export async function listLocations(): Promise<SavedLocation[]> {
  return invoke("list_locations");
}

export async function saveLocation(location: {
  id?: string | null;
  name: string;
}): Promise<SavedLocation> {
  return invoke("save_location", { location });
}

export async function deleteLocation(locationId: string): Promise<void> {
  return invoke("delete_location", { locationId });
}

export async function setMediaLocation(
  storageId: string,
  mediaId: number,
  location: string | null,
): Promise<string | null> {
  return invoke("set_media_location", { storageId, mediaId, location });
}

export async function appReady(): Promise<boolean> {
  return invoke("app_ready");
}

export async function deleteMedia(storageId: string, mediaId: number): Promise<void> {
  return invoke("delete_media", { storageId, mediaId });
}

export async function deleteMediaBatch(items: MediaRef[]): Promise<void> {
  return invoke("delete_media_batch", { items });
}

export async function ensureThumbnail(
  storageId: string,
  mediaId: number,
): Promise<string | null> {
  return invoke("ensure_thumbnail", { storageId, mediaId });
}

export async function preloadThumbnails(): Promise<number> {
  return invoke("preload_thumbnails");
}

export async function previewOrganize(storageId?: string): Promise<TransferOp[]> {
  return invoke("preview_organize", { storageId: storageId ?? null });
}

export async function executeOrganize(
  storageId: string,
  operations: TransferOp[],
): Promise<string[]> {
  return invoke("execute_organize", { storageId, operations });
}

export async function previewSync(sourcePath: string, storageId: string): Promise<TransferOp[]> {
  return invoke("preview_sync", { request: { sourcePath, storageId } });
}

export async function executeSync(storageId: string, operations: TransferOp[]): Promise<string[]> {
  return invoke("execute_sync", { storageId, operations });
}

export async function pickDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") return selected;
  return null;
}

export function previewUrl(path: string): string {
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

export type { JobProgress };
