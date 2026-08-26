import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppConfig, MediaItem, SearchQuery, TransferOp } from "./types";

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

export async function listTags(): Promise<string[]> {
  return invoke("list_tags");
}

export async function setMediaTags(mediaId: number, tags: string[]): Promise<string[]> {
  return invoke("set_media_tags", { mediaId, tags });
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
  return convertFileSrc(path);
}
