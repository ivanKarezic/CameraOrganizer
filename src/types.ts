export type CameraBrand = "DJI" | "GoPro" | "Insta360" | "Unknown";
export type MediaKind = "photo" | "video" | "sidecar";
export type DateSource = "exif" | "filename" | "mtime" | "unknown";
export type StorageMode = "single" | "multiple";
export type StorageKind = "local" | "network" | "external";
export type TransferAction = "move" | "copy";
export type ViewId = "library" | "organize" | "sync" | "tags" | "settings";

export interface Storage {
  id: string;
  name: string;
  path: string;
  kind: StorageKind;
}

export interface AppConfig {
  storageMode: StorageMode;
  storages: Storage[];
}

export interface TagCategory {
  id: string;
  name: string;
  color: string;
}

export interface GlobalTag {
  id: string;
  name: string;
  categoryId: string;
  categoryName: string;
  color: string;
}

export interface MediaTag {
  name: string;
  color: string;
  categoryId: string;
  categoryName: string;
}

export interface MediaItem {
  id: number;
  storageId: string;
  path: string;
  filename: string;
  size: number;
  hash: string | null;
  camera: CameraBrand;
  kind: MediaKind;
  capturedAt: string;
  dateSource: DateSource;
  latitude: number | null;
  longitude: number | null;
  locationLabel: string | null;
  organized: boolean;
  thumbnailPath: string | null;
  tags: MediaTag[];
}

export interface SearchQuery {
  text?: string | null;
  tag?: string | null;
  tagCategory?: string | null;
  dateFrom?: string | null;
  dateTo?: string | null;
  camera?: string | null;
  location?: string | null;
  unorganizedOnly?: boolean;
}

export interface TransferOp {
  source: string;
  destination: string;
  filename: string;
  action: TransferAction;
  warning: string | null;
  groupKey: string;
}

export interface JobProgress {
  job: string;
  current: number;
  total: number;
  filename: string;
  message: string;
}

export interface MediaRef {
  storageId: string;
  mediaId: number;
}

export function mediaKey(item: Pick<MediaItem, "storageId" | "id">): string {
  return `${item.storageId}:${item.id}`;
}
