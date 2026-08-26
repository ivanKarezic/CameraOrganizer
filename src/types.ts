export type CameraBrand = "DJI" | "GoPro" | "Insta360" | "Unknown";
export type MediaKind = "photo" | "video" | "sidecar";
export type DateSource = "exif" | "filename" | "mtime" | "unknown";
export type StorageMode = "single" | "multiple";
export type StorageKind = "local" | "network" | "external";
export type TransferAction = "move" | "copy";
export type ViewId = "library" | "organize" | "sync" | "settings";

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
  tags: string[];
}

export interface SearchQuery {
  text?: string | null;
  tag?: string | null;
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
