import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  appReady,
  deleteGlobalTag,
  deleteLocation,
  deleteMedia,
  deleteMediaBatch,
  deleteTagCategory,
  ensureThumbnail,
  executeOrganize,
  executeSync,
  getConfig,
  listLocations,
  listTagCategories,
  listTags,
  pickDirectory,
  preloadThumbnails,
  previewOrganize,
  previewSync,
  previewUrl,
  saveConfig,
  saveGlobalTag,
  saveLocation,
  saveTagCategory,
  scanLibrary,
  searchMedia,
  setMediaLocation,
  setMediaTags,
} from "./api";
import { DateRangePicker } from "./DateRangePicker";
import { clampToDays, uniqueCaptureDays, wrappedIndex } from "./lib/dates";
import { hasActiveFilters, matchesLibraryFilters, type KindVisibility } from "./lib/filter";
import { formatBytes, formatCaptureDate, groupByDate } from "./lib/format";
import type {
  AppConfig,
  GlobalTag,
  JobProgress,
  MediaItem,
  MediaTag,
  SavedLocation,
  SearchQuery,
  Storage,
  StorageKind,
  TagCategory,
  ThumbnailReady,
  TransferOp,
  ViewId,
} from "./types";
import { mediaKey } from "./types";

const NAV: { id: ViewId; label: string; icon: NavIconId }[] = [
  { id: "library", label: "Library", icon: "library" },
  { id: "organize", label: "Organize", icon: "organize" },
  { id: "sync", label: "Import", icon: "import" },
  { id: "tags", label: "Tags", icon: "tags" },
  { id: "settings", label: "Settings", icon: "settings" },
];

const CLASS_COLORS = ["#e59a2a", "#7ea36a", "#6a93c4", "#c45c3a", "#9b7ed9", "#d4c06a"];

type NavIconId = "library" | "organize" | "import" | "tags" | "settings";

const SCAN_PROGRESS: JobProgress = {
  job: "scan",
  current: 0,
  total: 0,
  filename: "",
  message: "Scanning library…",
};

type LibraryLayout = "thumbs" | "list";

const emptyConfig = (): AppConfig => ({ storageMode: "single", storages: [] });

export default function App() {
  const [view, setView] = useState<ViewId>("library");
  const [config, setConfig] = useState<AppConfig>(emptyConfig);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [selected, setSelected] = useState<MediaItem | null>(null);
  const [status, setStatus] = useState("Ready.");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<JobProgress | null>(null);
  const [preload, setPreload] = useState<JobProgress | null>(null);
  const [unorganizedOnly, setUnorganizedOnly] = useState(false);
  const [kinds, setKinds] = useState<KindVisibility>({ photo: true, video: true });
  const [search, setSearch] = useState<SearchQuery>({});
  const [tags, setTags] = useState<GlobalTag[]>([]);
  const [categories, setCategories] = useState<TagCategory[]>([]);
  const [locations, setLocations] = useState<SavedLocation[]>([]);
  const [uiReady, setUiReady] = useState(false);
  const [catalogLoaded, setCatalogLoaded] = useState(false);
  const [expandedPreview, setExpandedPreview] = useState(false);
  const [markedForDelete, setMarkedForDelete] = useState<Set<string>>(new Set());
  const [navCollapsed, setNavCollapsed] = useState(false);
  const [ops, setOps] = useState<TransferOp[]>([]);
  const [selectedOps, setSelectedOps] = useState<Set<string>>(new Set());
  const [syncSource, setSyncSource] = useState("");
  const [syncStorageId, setSyncStorageId] = useState("");

  useEffect(() => {
    const webview = getCurrentWebviewWindow();
    const unlisten: Array<() => void> = [];
    const onProgress = (payload: JobProgress) => {
      if (payload.job === "preload") {
        setPreload(payload);
        return;
      }
      setPreload(null);
      setProgress(payload);
      const count =
        payload.total > 0
          ? ` ${payload.current}/${payload.total}`
          : payload.current > 0
            ? ` ${payload.current}`
            : "";
      setStatus(
        payload.filename ? `${payload.message}${count} — ${payload.filename}` : `${payload.message}${count}`,
      );
    };
    const onThumb = (payload: ThumbnailReady) => {
      setItems((current) =>
        current.map((item) =>
          item.storageId === payload.storageId && item.id === payload.mediaId
            ? { ...item, thumbnailPath: payload.path }
            : item,
        ),
      );
    };
    const bind = async (
      listenFn: typeof webview.listen,
    ) => {
      unlisten.push(await listenFn<JobProgress>("job-progress", (event) => onProgress(event.payload)));
      unlisten.push(await listenFn<ThumbnailReady>("thumbnail-ready", (event) => onThumb(event.payload)));
    };
    void bind(webview.listen.bind(webview)).catch(() => {
      void bind(listen);
    });
    return () => unlisten.forEach((fn) => fn());
  }, []);

  async function loadCatalog() {
    setError(null);
    try {
      const cfg = await getConfig();
      setConfig(cfg);
      if (!cfg.storages.length) {
        setItems([]);
        setStatus("Add a media storage in Settings.");
        return;
      }
      setStatus("Loading catalog…");
      const [found, nextTags, nextCategories, nextLocations] = await Promise.all([
        searchMedia({}),
        listTags(),
        listTagCategories(),
        listLocations(),
      ]);
      setItems(found);
      setTags(nextTags);
      setCategories(nextCategories);
      setLocations(nextLocations);
      setStatus(
        found.length
          ? `${found.length} files in catalog.`
          : "Catalog is empty. Scan the library to index files.",
      );
      if (!syncStorageId && cfg.storages[0]) {
        setSyncStorageId(cfg.storages[0].id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCatalogLoaded(true);
    }
  }

  async function refresh() {
    setBusy(true);
    setScanning(true);
    setError(null);
    setProgress(SCAN_PROGRESS);
    setStatus("Scanning library…");
    try {
      const cfg = await getConfig();
      setConfig(cfg);
      if (!cfg.storages.length) {
        setItems([]);
        setStatus("Add a media storage in Settings.");
        return;
      }
      const scanned = await scanLibrary();
      setItems(scanned);
      await reloadTagStore();
      setStatus(`${scanned.length} files in catalog.`);
      if (!syncStorageId && cfg.storages[0]) {
        setSyncStorageId(cfg.storages[0].id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
      setScanning(false);
      setProgress(null);
    }
  }

  async function reloadTagStore() {
    const [nextTags, nextCategories, nextLocations] = await Promise.all([
      listTags(),
      listTagCategories(),
      listLocations(),
    ]);
    setTags(nextTags);
    setCategories(nextCategories);
    setLocations(nextLocations);
  }

  useEffect(() => {
    void loadCatalog();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!catalogLoaded || uiReady || busy || scanning) return;
    let cancelled = false;
    const markReady = () => {
      void (async () => {
        try {
          await appReady();
        } catch {
          // Web preview without Tauri still marks the UI ready.
        }
        if (!cancelled) setUiReady(true);
      })();
    };
    let timer = window.setTimeout(markReady, 4000);
    const bump = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(markReady, 4000);
    };
    window.addEventListener("pointerdown", bump, { passive: true });
    window.addEventListener("keydown", bump);
    window.addEventListener("wheel", bump, { passive: true });
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      window.removeEventListener("pointerdown", bump);
      window.removeEventListener("keydown", bump);
      window.removeEventListener("wheel", bump);
    };
  }, [catalogLoaded, busy, scanning, uiReady]);

  useEffect(() => {
    if (!uiReady || busy || scanning) return;
    void preloadThumbnails();
  }, [uiReady, busy, scanning]);

  const visible = useMemo(
    () => items.filter((item) => matchesLibraryFilters(item, search, unorganizedOnly, kinds)),
    [items, search, unorganizedOnly, kinds],
  );
  const groups = useMemo(() => groupByDate(visible), [visible]);
  const ordered = useMemo(() => [...groups.values()].flat(), [groups]);
  const captureDays = useMemo(
    () => uniqueCaptureDays(items.filter((item) => item.kind !== "sidecar").map((item) => item.capturedAt)),
    [items],
  );

  useEffect(() => {
    if (selected && !visible.some((item) => item.path === selected.path)) {
      setSelected(null);
    }
  }, [visible, selected]);

  useEffect(() => {
    if (!search.dateFrom && !search.dateTo) return;
    const dateFrom = clampToDays(search.dateFrom, captureDays);
    const dateTo = clampToDays(search.dateTo, captureDays);
    if (dateFrom !== (search.dateFrom ?? null) || dateTo !== (search.dateTo ?? null)) {
      setSearch((current) => ({ ...current, dateFrom, dateTo }));
    }
  }, [captureDays, search.dateFrom, search.dateTo]);

  useEffect(() => {
    if (!selected) setExpandedPreview(false);
  }, [selected]);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape" && expandedPreview) {
        event.preventDefault();
        setExpandedPreview(false);
        return;
      }
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA" || tag === "VIDEO") return;
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault();
      stepPreview(event.key === "ArrowRight" ? 1 : -1);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selected, ordered, expandedPreview]);

  function stepPreview(delta: number) {
    if (ordered.length === 0) return;
    const index = selected
      ? ordered.findIndex((item) => mediaKey(item) === mediaKey(selected))
      : -1;
    setSelected(ordered[wrappedIndex(index, delta, ordered.length)]);
  }

  return (
    <div className={navCollapsed ? "app-shell nav-collapsed" : "app-shell"}>
      <nav className="nav" aria-label="Main">
        <div className="nav-brand">
          <h1 className="wordmark">
            {navCollapsed ? (
              <>
                C<span>O</span>
              </>
            ) : (
              <>
                CAMERA
                <br />
                <span>ORGANIZER</span>
              </>
            )}
          </h1>
          <button
            className="nav-collapse"
            onClick={() => setNavCollapsed((value) => !value)}
            title={navCollapsed ? "Expand sidebar" : "Minimize sidebar"}
            aria-label={navCollapsed ? "Expand sidebar" : "Minimize sidebar"}
          >
            <SidebarIcon />
          </button>
        </div>
        <div className="nav-links">
          {NAV.map((item) => (
            <button
              key={item.id}
              className={view === item.id ? "active" : ""}
              onClick={() => setView(item.id)}
              title={item.label}
            >
              <NavIcon id={item.icon} />
              <span className="nav-label">{item.label}</span>
            </button>
          ))}
        </div>
        {navCollapsed ? null : <p className={error ? "status error" : "status"}>{error ?? status}</p>}
        {navCollapsed ? null : <ProgressMeter progress={progress} />}
        <PreloadStatus progress={preload} collapsed={navCollapsed} />
      </nav>
      <main className="stage">
        {view === "library" && (
          <LibraryView
            groups={groups}
            selected={selected}
            unorganizedOnly={unorganizedOnly}
            kinds={kinds}
            search={search}
            captureDays={captureDays}
            locations={locations}
            tags={tags}
            categories={categories}
            matchCount={visible.length}
            busy={busy || !catalogLoaded}
            scanning={scanning}
            progress={progress}
            onToggleUnorganized={() => setUnorganizedOnly((v) => !v)}
            onKindsChange={setKinds}
            onSearchChange={setSearch}
            onClearFilters={() => {
              setSearch({});
              setUnorganizedOnly(false);
              setKinds({ photo: true, video: true });
            }}
            onRefresh={() => void refresh()}
            onSelect={setSelected}
            markedForDelete={markedForDelete}
            onClearMarked={() => setMarkedForDelete(new Set())}
            onDeleteMarked={async () => {
              if (markedForDelete.size === 0) return;
              setBusy(true);
              try {
                const refs = items
                  .filter((item) => markedForDelete.has(mediaKey(item)))
                  .map((item) => ({ storageId: item.storageId, mediaId: item.id }));
                await deleteMediaBatch(refs);
                const removed = new Set(refs.map((ref) => `${ref.storageId}:${ref.mediaId}`));
                setItems((current) =>
                  current.filter((item) => !removed.has(mediaKey(item))),
                );
                setMarkedForDelete(new Set());
                if (selected && removed.has(mediaKey(selected))) setSelected(null);
                setStatus(`Deleted ${refs.length} file${refs.length === 1 ? "" : "s"}.`);
              } catch (err) {
                setError(err instanceof Error ? err.message : String(err));
              } finally {
                setBusy(false);
              }
            }}
          />
        )}
        {view === "organize" && (
          <OrganizeView
            config={config}
            ops={ops}
            selectedOps={selectedOps}
            busy={busy}
            progress={progress}
            onPreview={async (storageId?: string) => {
              setBusy(true);
              try {
                const next = await previewOrganize(storageId);
                setOps(next);
                setSelectedOps(new Set(next.map((op) => op.source)));
                setStatus(`${next.length} files ready to move.`);
              } catch (err) {
                setError(String(err));
              } finally {
                setBusy(false);
              }
            }}
            onToggle={(source) => {
              const next = new Set(selectedOps);
              if (next.has(source)) next.delete(source);
              else next.add(source);
              setSelectedOps(next);
            }}
            onExecute={async (storageId) => {
              const chosen = ops.filter((op) => selectedOps.has(op.source));
              setBusy(true);
              try {
                await executeOrganize(storageId, chosen);
                await refresh();
                setOps([]);
                setStatus("Move complete.");
              } catch (err) {
                setError(String(err));
              } finally {
                setBusy(false);
                setProgress(null);
              }
            }}
          />
        )}
        {view === "sync" && (
          <SyncView
            config={config}
            source={syncSource}
            storageId={syncStorageId}
            ops={ops}
            selectedOps={selectedOps}
            busy={busy}
            progress={progress}
            onSource={setSyncSource}
            onStorage={setSyncStorageId}
            onPick={async () => {
              const dir = await pickDirectory();
              if (dir) setSyncSource(dir);
            }}
            onPreview={async () => {
              setBusy(true);
              try {
                const next = await previewSync(syncSource, syncStorageId);
                setOps(next);
                setSelectedOps(new Set(next.map((op) => op.source)));
                setStatus(`${next.length} files missing from the library.`);
              } catch (err) {
                setError(String(err));
              } finally {
                setBusy(false);
              }
            }}
            onToggle={(source) => {
              const next = new Set(selectedOps);
              if (next.has(source)) next.delete(source);
              else next.add(source);
              setSelectedOps(next);
            }}
            onExecute={async () => {
              const chosen = ops.filter((op) => selectedOps.has(op.source));
              setBusy(true);
              try {
                await executeSync(syncStorageId, chosen);
                await refresh();
                setOps([]);
                setStatus("Copy complete.");
              } catch (err) {
                setError(String(err));
              } finally {
                setBusy(false);
                setProgress(null);
              }
            }}
          />
        )}
        {view === "tags" && (
          <TagsView
            tags={tags}
            categories={categories}
            locations={locations}
            onReload={reloadTagStore}
            onError={setError}
          />
        )}
        {view === "settings" && (
          <SettingsView
            config={config}
            onChange={setConfig}
            onSave={async () => {
              setBusy(true);
              try {
                const saved = await saveConfig(config);
                setConfig(saved);
                await refresh();
                setStatus("Configuration saved.");
              } catch (err) {
                setError(String(err));
              } finally {
                setBusy(false);
                setProgress(null);
              }
            }}
          />
        )}
      </main>
      <PreviewPane
        item={selected}
        index={
          selected ? ordered.findIndex((item) => mediaKey(item) === mediaKey(selected)) : -1
        }
        total={ordered.length}
        onPrev={() => stepPreview(-1)}
        onNext={() => stepPreview(1)}
        globalTags={tags}
        categories={categories}
        marked={selected ? markedForDelete.has(mediaKey(selected)) : false}
        onMarkedChange={(checked) => {
          if (!selected) return;
          const key = mediaKey(selected);
          setMarkedForDelete((current) => {
            const next = new Set(current);
            if (checked) next.add(key);
            else next.delete(key);
            return next;
          });
        }}
        onDeleteCurrent={async () => {
          if (!selected) return;
          setBusy(true);
          try {
            await deleteMedia(selected.storageId, selected.id);
            const key = mediaKey(selected);
            const remaining = ordered.filter((item) => mediaKey(item) !== key);
            const index = ordered.findIndex((item) => mediaKey(item) === key);
            setItems((current) => current.filter((item) => mediaKey(item) !== key));
            setMarkedForDelete((current) => {
              const next = new Set(current);
              next.delete(key);
              return next;
            });
            setSelected(
              remaining.length ? remaining[Math.min(index, remaining.length - 1)] : null,
            );
            setStatus("File deleted.");
          } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
          } finally {
            setBusy(false);
          }
        }}
        onTags={async (nextTags) => {
          if (!selected) return;
          const saved = await setMediaTags(
            selected.storageId,
            selected.id,
            nextTags.map((tag) => tag.name),
          );
          setSelected({ ...selected, tags: saved });
          setItems((current) =>
            current.map((item) =>
              item.id === selected.id && item.storageId === selected.storageId
                ? { ...item, tags: saved }
                : item,
            ),
          );
        }}
        onCreateTag={async (name, categoryId) => {
          const created = await saveGlobalTag({ name, categoryId });
          await reloadTagStore();
          return created;
        }}
        locations={locations}
        onLocation={async (label) => {
          if (!selected) return;
          const saved = await setMediaLocation(selected.storageId, selected.id, label);
          setSelected({ ...selected, locationLabel: saved });
          setItems((current) =>
            current.map((item) =>
              item.id === selected.id && item.storageId === selected.storageId
                ? { ...item, locationLabel: saved }
                : item,
            ),
          );
          setLocations(await listLocations());
        }}
        onExpand={() => setExpandedPreview(true)}
      />
      {expandedPreview && selected ? (
        <MediaLightbox
          item={selected}
          index={ordered.findIndex((item) => mediaKey(item) === mediaKey(selected))}
          total={ordered.length}
          onPrev={() => stepPreview(-1)}
          onNext={() => stepPreview(1)}
          onClose={() => setExpandedPreview(false)}
        />
      ) : null}
    </div>
  );
}

function PreloadStatus({
  progress,
  collapsed,
}: {
  progress: JobProgress | null;
  collapsed: boolean;
}) {
  if (!progress) return null;
  const total = progress.total;
  const current = progress.current;
  const done = total === 0 || (total > 0 && current >= total);
  const pct = total > 0 ? Math.min(100, (current / total) * 100) : done ? 100 : 0;
  const label = done
    ? "Thumbnails ready"
    : total > 0
      ? `Preloading ${current} / ${total}`
      : "Preloading thumbnails";
  return (
    <div
      className={collapsed ? "nav-preload collapsed" : "nav-preload"}
      role="status"
      title={label}
    >
      {collapsed ? null : <div className="preload-label">{label}</div>}
      <div className="progress-track">
        <div
          className={total > 0 || done ? "progress-fill" : "progress-fill indeterminate"}
          style={total > 0 || done ? { width: `${pct}%` } : undefined}
        />
      </div>
    </div>
  );
}

function ProgressMeter({ progress }: { progress: JobProgress | null }) {
  if (!progress) return null;
  const pct = progress.total > 0 ? Math.min(100, (progress.current / progress.total) * 100) : 0;
  return (
    <div className="progress-meter" role="status">
      <div className="progress-label">
        {progress.message}
        {progress.filename ? ` — ${progress.filename}` : ""}
      </div>
      <div className="progress-track">
        <div
          className={progress.total > 0 ? "progress-fill" : "progress-fill indeterminate"}
          style={progress.total > 0 ? { width: `${pct}%` } : undefined}
        />
      </div>
      {progress.total > 0 ? (
        <div className="progress-count">
          {progress.current} / {progress.total}
          {` · ${Math.round(pct)}%`}
        </div>
      ) : progress.current > 0 ? (
        <div className="progress-count">{progress.current} files</div>
      ) : (
        <div className="progress-count">Working…</div>
      )}
    </div>
  );
}

function LibraryView({
  groups,
  selected,
  unorganizedOnly,
  kinds,
  search,
  captureDays,
  locations,
  tags,
  categories,
  matchCount,
  busy,
  scanning,
  progress,
  onToggleUnorganized,
  onKindsChange,
  onSearchChange,
  onClearFilters,
  onRefresh,
  onSelect,
  markedForDelete,
  onClearMarked,
  onDeleteMarked,
}: {
  groups: Map<string, MediaItem[]>;
  selected: MediaItem | null;
  unorganizedOnly: boolean;
  kinds: KindVisibility;
  search: SearchQuery;
  captureDays: string[];
  locations: SavedLocation[];
  tags: GlobalTag[];
  categories: TagCategory[];
  matchCount: number;
  busy: boolean;
  scanning: boolean;
  progress: JobProgress | null;
  onToggleUnorganized: () => void;
  onKindsChange: (kinds: KindVisibility) => void;
  onSearchChange: (query: SearchQuery) => void;
  onClearFilters: () => void;
  onRefresh: () => void;
  onSelect: (item: MediaItem) => void;
  markedForDelete: Set<string>;
  onClearMarked: () => void;
  onDeleteMarked: () => void;
}) {
  const [layout, setLayout] = useState<LibraryLayout>("list");
  const filtering = hasActiveFilters(search, kinds) || unorganizedOnly;
  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="section-title">ROLL</h2>
        <p className="lede">
          Camera originals grouped by day. Tick photos or videos, then switch between thumbnails and
          a compact list.
        </p>
        <div className="toolbar">
          <button className="primary" onClick={onRefresh} disabled={busy}>
            {scanning ? "Scanning…" : "Scan library"}
          </button>
          <button
            className="danger"
            disabled={busy || markedForDelete.size === 0}
            onClick={onDeleteMarked}
          >
            Delete marked{markedForDelete.size ? ` (${markedForDelete.size})` : ""}
          </button>
          <button
            className="primary"
            disabled={busy || markedForDelete.size === 0}
            onClick={onClearMarked}
          >
            Clear selected{markedForDelete.size ? ` (${markedForDelete.size})` : ""}
          </button>
        </div>
        <div className="toolbar library-toggles">
          <div className="tick-group">
            <label className="tick">
              <input
                type="checkbox"
                checked={kinds.photo}
                onChange={(e) => onKindsChange({ ...kinds, photo: e.target.checked })}
              />
              Photos
            </label>
            <label className="tick">
              <input
                type="checkbox"
                checked={kinds.video}
                onChange={(e) => onKindsChange({ ...kinds, video: e.target.checked })}
              />
              Videos
            </label>
            <label className="tick">
              <input
                type="checkbox"
                checked={unorganizedOnly}
                onChange={onToggleUnorganized}
              />
              Show unorganized
            </label>
          </div>
          <div className="view-toggle" role="group" aria-label="Library layout">
            <button
              className={layout === "thumbs" ? "active" : ""}
              onClick={() => setLayout("thumbs")}
            >
              Thumbnails
            </button>
            <button className={layout === "list" ? "active" : ""} onClick={() => setLayout("list")}>
              List
            </button>
          </div>
        </div>
      <div className="toolbar">
        <input
          type="search"
          placeholder="Filename, tag, camera…"
          value={search.text ?? ""}
          onChange={(e) => onSearchChange({ ...search, text: e.target.value })}
        />
        <input
          type="text"
          placeholder="Location"
          list="library-locations"
          value={search.location ?? ""}
          onChange={(e) => onSearchChange({ ...search, location: e.target.value || null })}
        />
        <datalist id="library-locations">
          {locations.map((location) => (
            <option key={location.id} value={location.name} />
          ))}
        </datalist>
        <DateRangePicker
          days={captureDays}
          dateFrom={search.dateFrom}
          dateTo={search.dateTo}
          onChange={({ dateFrom, dateTo }) => onSearchChange({ ...search, dateFrom, dateTo })}
        />
        <select
          value={search.camera ?? ""}
          onChange={(e) => onSearchChange({ ...search, camera: e.target.value || null })}
        >
          <option value="">All cameras</option>
          <option>DJI</option>
          <option>GoPro</option>
          <option>Insta360</option>
          <option>Unknown</option>
        </select>
        <select
          value={search.tagCategory ?? ""}
          onChange={(e) => {
            const tagCategory = e.target.value || null;
            const next: SearchQuery = { ...search, tagCategory };
            if (
              search.tag &&
              tagCategory &&
              !tags.some(
                (tag) =>
                  tag.name === search.tag &&
                  tag.categoryName.toLowerCase() === tagCategory.toLowerCase(),
              )
            ) {
              next.tag = null;
            }
            onSearchChange(next);
          }}
        >
          <option value="">All categories</option>
          {categories.map((category) => (
            <option key={category.id} value={category.name}>
              {category.name}
            </option>
          ))}
        </select>
        <select
          value={search.tag ?? ""}
          onChange={(e) => onSearchChange({ ...search, tag: e.target.value || null })}
        >
          <option value="">All tags</option>
          {tags
            .filter(
              (tag) =>
                !search.tagCategory ||
                tag.categoryName.toLowerCase() === search.tagCategory.toLowerCase(),
            )
            .map((tag) => (
              <option key={tag.id} value={tag.name}>
                {tag.name}
              </option>
            ))}
        </select>
        {filtering ? (
          <button className="ghost" onClick={onClearFilters}>
            Clear filters
          </button>
        ) : null}
        {filtering ? <span className="status">{matchCount} matching</span> : null}
      </div>
      {scanning ? <ProgressMeter progress={progress ?? SCAN_PROGRESS} /> : null}
      </div>
      <div className="panel-scroll">
      {groups.size === 0 ? (
        <div className="empty">
          {filtering ? "No files match these filters." : "No media yet. Add a storage path, then scan."}
        </div>
      ) : (
        <>
          {layout === "list" ? (
            <div className="file-list-head">
              <span></span>
              <span>File name</span>
              <span>Type</span>
              <span>Tags</span>
            </div>
          ) : null}
          {[...groups.entries()].map(([day, files]) => (
            <div className={layout === "list" ? "day-block list-block" : "day-block"} key={day}>
              <div className="day-label">{day}</div>
              {layout === "thumbs" ? (
                <div className="grid">
                  {files.map((item) => (
                    <button
                      key={item.path}
                      className={selected?.path === item.path ? "card selected" : "card"}
                      onClick={() => onSelect(item)}
                    >
                      <div className="card-thumb">
                        <MediaThumb item={item} />
                      </div>
                      <div className="card-meta">
                        <span className="card-kind">{item.kind}</span>
                        <span className="card-name">{item.filename}</span>
                      </div>
                      <span className="badge">{item.camera}</span>
                      {!item.organized && <span className="badge unorganized">loose</span>}
                      {markedForDelete.has(mediaKey(item)) ? (
                        <span className="trash-mark" title="Marked for deletion">
                          ⌫
                        </span>
                      ) : null}
                    </button>
                  ))}
                </div>
              ) : (
                files.map((item) => (
                  <button
                    key={item.path}
                    className={selected?.path === item.path ? "file-row selected" : "file-row"}
                    onClick={() => onSelect(item)}
                  >
                    <span className="file-mark">
                      {markedForDelete.has(mediaKey(item)) ? (
                        <span className="trash-mark" title="Marked for deletion">
                          ⌫
                        </span>
                      ) : null}
                    </span>
                    <span className="file-name">{item.filename}</span>
                    <span className="file-type">{item.kind}</span>
                    <span className="file-tags">
                      {item.tags.length ? (
                        <span className="tag-row compact">
                          {item.tags.map((tag) => (
                            <span
                              key={tag.name}
                              className="tag"
                              style={{ borderColor: tag.color, color: tag.color }}
                            >
                              {tag.name}
                            </span>
                          ))}
                        </span>
                      ) : (
                        "—"
                      )}
                    </span>
                  </button>
                ))
              )}
            </div>
          ))}
        </>
      )}
      </div>
    </section>
  );
}

function MediaThumb({ item }: { item: MediaItem }) {
  const slot = useRef<HTMLDivElement>(null);
  const tried = useRef(false);
  const [active, setActive] = useState(false);
  const [thumb, setThumb] = useState(item.thumbnailPath);
  const [broken, setBroken] = useState(false);

  useEffect(() => {
    setThumb(item.thumbnailPath);
    setBroken(false);
    tried.current = false;
  }, [item.thumbnailPath, item.path]);

  useEffect(() => {
    const node = slot.current;
    if (!node) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setActive(true);
          observer.disconnect();
        }
      },
      { rootMargin: "240px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!active || (thumb && !broken) || tried.current) return;
    tried.current = true;
    let cancelled = false;
    void ensureThumbnail(item.storageId, item.id).then((path) => {
      if (cancelled) return;
      if (path) {
        setBroken(false);
        setThumb(path);
      } else {
        setThumb(null);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [active, thumb, broken, item.storageId, item.id]);

  const src = active && thumb && !broken ? previewUrl(thumb) : "";
  return (
    <div ref={slot} className={src ? "thumb-slot" : "thumb-slot missing"}>
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          onError={() => setBroken(true)}
        />
      ) : null}
    </div>
  );
}

function OrganizeView({
  config,
  ops,
  selectedOps,
  busy,
  progress,
  onPreview,
  onToggle,
  onExecute,
}: {
  config: AppConfig;
  ops: TransferOp[];
  selectedOps: Set<string>;
  busy: boolean;
  progress: JobProgress | null;
  onPreview: (storageId?: string) => void;
  onToggle: (source: string) => void;
  onExecute: (storageId: string) => void;
}) {
  const [storageId, setStorageId] = useState(config.storages[0]?.id ?? "");
  useEffect(() => {
    if (!storageId && config.storages[0]) {
      setStorageId(config.storages[0].id);
    }
  }, [config, storageId]);
  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="section-title">SHELVE</h2>
        <p className="lede">
          Existing library files are moved into year / day / Video or Photo. Review the plan, then move
          everything or a selection.
        </p>
        <div className="toolbar">
          <select value={storageId} onChange={(e) => setStorageId(e.target.value)}>
            {config.storages.map((storage) => (
              <option key={storage.id} value={storage.id}>
                {storage.name}
              </option>
            ))}
          </select>
          <button className="ghost" disabled={busy || !storageId} onClick={() => onPreview(storageId)}>
            Find unorganized
          </button>
          <button
            className="primary"
            disabled={busy || selectedOps.size === 0 || !storageId}
            onClick={() => onExecute(storageId)}
          >
            Move selected
          </button>
        </div>
        {busy && progress ? <ProgressMeter progress={progress} /> : null}
      </div>
      <div className="panel-scroll">
        <OpTable ops={ops} selectedOps={selectedOps} onToggle={onToggle} />
      </div>
    </section>
  );
}

function SyncView({
  config,
  source,
  storageId,
  ops,
  selectedOps,
  busy,
  progress,
  onSource,
  onStorage,
  onPick,
  onPreview,
  onToggle,
  onExecute,
}: {
  config: AppConfig;
  source: string;
  storageId: string;
  ops: TransferOp[];
  selectedOps: Set<string>;
  busy: boolean;
  progress: JobProgress | null;
  onSource: (value: string) => void;
  onStorage: (value: string) => void;
  onPick: () => void;
  onPreview: () => void;
  onToggle: (source: string) => void;
  onExecute: () => void;
}) {
  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="section-title">INTAKE</h2>
        <p className="lede">
          Compare an SD card or other volume to the library. Missing files are copied into the correct
          date folder — originals stay on the card.
        </p>
        <div className="toolbar">
          <input
            type="text"
            style={{ minWidth: 280 }}
            placeholder="External volume path"
            value={source}
            onChange={(e) => onSource(e.target.value)}
          />
          <button className="ghost" onClick={onPick}>
            Browse
          </button>
          <select value={storageId} onChange={(e) => onStorage(e.target.value)}>
            {config.storages.map((storage) => (
              <option key={storage.id} value={storage.id}>
                {storage.name}
              </option>
            ))}
          </select>
          <button className="ghost" disabled={busy || !source || !storageId} onClick={onPreview}>
            Find missing
          </button>
          <button className="primary" disabled={busy || selectedOps.size === 0} onClick={onExecute}>
            Copy selected
          </button>
        </div>
        {busy && progress ? <ProgressMeter progress={progress} /> : null}
      </div>
      <div className="panel-scroll">
        <OpTable ops={ops} selectedOps={selectedOps} onToggle={onToggle} />
      </div>
    </section>
  );
}

function OpTable({
  ops,
  selectedOps,
  onToggle,
}: {
  ops: TransferOp[];
  selectedOps: Set<string>;
  onToggle: (source: string) => void;
}) {
  if (!ops.length) {
    return <div className="empty">No planned transfers yet.</div>;
  }
  return (
    <table className="table">
      <thead>
        <tr>
          <th></th>
          <th>Action</th>
          <th>File</th>
          <th>Destination</th>
        </tr>
      </thead>
      <tbody>
        {ops.map((op) => (
          <tr key={op.source}>
            <td>
              <input
                type="checkbox"
                checked={selectedOps.has(op.source)}
                onChange={() => onToggle(op.source)}
              />
            </td>
            <td>{op.action}</td>
            <td>
              {op.filename}
              {op.warning ? <div className="status error">{op.warning}</div> : null}
            </td>
            <td>{op.destination}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SettingsView({
  config,
  onChange,
  onSave,
}: {
  config: AppConfig;
  onChange: (config: AppConfig) => void;
  onSave: () => void;
}) {
  async function addStorage() {
    const path = await pickDirectory();
    if (!path) return;
    if (config.storageMode === "single" && config.storages.length >= 1) {
      return;
    }
    const next: Storage = {
      id: crypto.randomUUID(),
      name: path.split(/[\\/]/).filter(Boolean).pop() ?? "Storage",
      path,
      kind: "local",
    };
    onChange({ ...config, storages: [...config.storages, next] });
  }

  function update(id: string, patch: Partial<Storage>) {
    onChange({
      ...config,
      storages: config.storages.map((s) => (s.id === id ? { ...s, ...patch } : s)),
    });
  }

  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="section-title">VAULT</h2>
        <p className="lede">
          One library disk, or several — local, mounted network, or a plugged-in drive. Each storage
          keeps its own CamOrg folder for the catalog and thumbnails.
        </p>
        <div className="toolbar">
          <select
            value={config.storageMode}
            onChange={(e) =>
              onChange({
                ...config,
                storageMode: e.target.value as AppConfig["storageMode"],
              })
            }
          >
            <option value="single">Single location</option>
            <option value="multiple">Multiple locations</option>
          </select>
          <button className="ghost" onClick={() => void addStorage()}>
            Add storage
          </button>
          <button className="primary" onClick={onSave}>
            Save configuration
          </button>
        </div>
      </div>
      <div className="panel-scroll">
        {config.storages.map((storage) => (
          <div className="storage-card" key={storage.id}>
            <div className="toolbar">
              <input
                type="text"
                value={storage.name}
                onChange={(e) => update(storage.id, { name: e.target.value })}
              />
              <input
                type="text"
                style={{ minWidth: 280 }}
                value={storage.path}
                onChange={(e) => update(storage.id, { path: e.target.value })}
              />
              <select
                value={storage.kind}
                onChange={(e) => update(storage.id, { kind: e.target.value as StorageKind })}
              >
                <option value="local">Local disk</option>
                <option value="network">Network storage</option>
                <option value="external">External drive</option>
              </select>
              <button
                className="danger"
                onClick={() =>
                  onChange({
                    ...config,
                    storages: config.storages.filter((s) => s.id !== storage.id),
                  })
                }
              >
                Remove
              </button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function TagsView({
  tags,
  categories,
  locations,
  onReload,
  onError,
}: {
  tags: GlobalTag[];
  categories: TagCategory[];
  locations: SavedLocation[];
  onReload: () => Promise<void>;
  onError: (message: string | null) => void;
}) {
  const [categoryName, setCategoryName] = useState("");
  const [categoryColor, setCategoryColor] = useState(CLASS_COLORS[0]);
  const [tagName, setTagName] = useState("");
  const [tagCategoryId, setTagCategoryId] = useState(categories[0]?.id ?? "");
  const [locationName, setLocationName] = useState("");

  useEffect(() => {
    if (tagCategoryId && categories.some((category) => category.id === tagCategoryId)) return;
    if (categories[0]) setTagCategoryId(categories[0].id);
  }, [categories, tagCategoryId]);

  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="section-title">TAGS</h2>
        <p className="lede">
          Categories hold a color. Tags belong to a category and inherit that color. Storages only
          remember which tag names are applied to a file.
        </p>
        <h3 className="subsection">Categories</h3>
        <div className="toolbar">
          <input
            type="text"
            placeholder="Category name"
            value={categoryName}
            onChange={(e) => setCategoryName(e.target.value)}
          />
          <input
            type="color"
            value={categoryColor}
            onChange={(e) => setCategoryColor(e.target.value)}
          />
          <button
            className="primary"
            onClick={async () => {
              const trimmed = categoryName.trim();
              if (!trimmed) return;
              try {
                await saveTagCategory({ name: trimmed, color: categoryColor });
                setCategoryName("");
                setCategoryColor(CLASS_COLORS[categories.length % CLASS_COLORS.length]);
                await onReload();
                onError(null);
              } catch (err) {
                onError(err instanceof Error ? err.message : String(err));
              }
            }}
          >
            Add category
          </button>
        </div>
        <h3 className="subsection">Tags</h3>
        <div className="toolbar">
          <input
            type="text"
            placeholder="Tag name"
            value={tagName}
            onChange={(e) => setTagName(e.target.value)}
          />
          <select
            value={tagCategoryId}
            onChange={(e) => setTagCategoryId(e.target.value)}
            disabled={categories.length === 0}
          >
            {categories.length === 0 ? <option value="">No categories</option> : null}
            {categories.map((category) => (
              <option key={category.id} value={category.id}>
                {category.name}
              </option>
            ))}
          </select>
          <button
            className="primary"
            disabled={!tagCategoryId}
            onClick={async () => {
              const trimmed = tagName.trim();
              if (!trimmed || !tagCategoryId) return;
              try {
                await saveGlobalTag({ name: trimmed, categoryId: tagCategoryId });
                setTagName("");
                await onReload();
                onError(null);
              } catch (err) {
                onError(err instanceof Error ? err.message : String(err));
              }
            }}
          >
            Add tag
          </button>
        </div>
        <h3 className="subsection">Locations</h3>
        <p className="lede">
          Named places for files that have no GPS. Assign them in the preview, or add them here
          first.
        </p>
        <div className="toolbar">
          <input
            type="text"
            placeholder="Location name"
            value={locationName}
            onChange={(e) => setLocationName(e.target.value)}
          />
          <button
            className="primary"
            onClick={async () => {
              const trimmed = locationName.trim();
              if (!trimmed) return;
              try {
                await saveLocation({ name: trimmed });
                setLocationName("");
                await onReload();
                onError(null);
              } catch (err) {
                onError(err instanceof Error ? err.message : String(err));
              }
            }}
          >
            Add location
          </button>
        </div>
      </div>
      <div className="panel-scroll">
        {categories.map((category) => {
          const inCategory = tags.filter((tag) => tag.categoryId === category.id);
          return (
            <div className="storage-card" key={category.id}>
              <div className="toolbar class-row">
                <span className="class-swatch" style={{ background: category.color }} />
                <input
                  key={`${category.id}:${category.name}`}
                  type="text"
                  defaultValue={category.name}
                  onBlur={async (e) => {
                    const next = e.target.value.trim();
                    if (!next || next === category.name) return;
                    try {
                      await saveTagCategory({
                        id: category.id,
                        name: next,
                        color: category.color,
                      });
                      await onReload();
                    } catch (err) {
                      onError(err instanceof Error ? err.message : String(err));
                    }
                  }}
                />
                <input
                  type="color"
                  value={category.color}
                  onChange={async (e) => {
                    try {
                      await saveTagCategory({
                        id: category.id,
                        name: category.name,
                        color: e.target.value,
                      });
                      await onReload();
                    } catch (err) {
                      onError(err instanceof Error ? err.message : String(err));
                    }
                  }}
                />
                <button
                  className="danger"
                  disabled={categories.length <= 1}
                  onClick={async () => {
                    try {
                      await deleteTagCategory(category.id);
                      await onReload();
                    } catch (err) {
                      onError(err instanceof Error ? err.message : String(err));
                    }
                  }}
                >
                  Delete category
                </button>
              </div>
              {inCategory.length === 0 ? (
                <div className="status">No tags in this category.</div>
              ) : (
                inCategory.map((tag) => (
                  <div className="toolbar class-row" key={tag.id}>
                    <input
                      key={`${tag.id}:${tag.name}`}
                      type="text"
                      defaultValue={tag.name}
                      onBlur={async (e) => {
                        const next = e.target.value.trim();
                        if (!next || next === tag.name) return;
                        try {
                          await saveGlobalTag({
                            id: tag.id,
                            name: next,
                            categoryId: tag.categoryId,
                          });
                          await onReload();
                        } catch (err) {
                          onError(err instanceof Error ? err.message : String(err));
                        }
                      }}
                    />
                    <select
                      value={tag.categoryId}
                      onChange={async (e) => {
                        try {
                          await saveGlobalTag({
                            id: tag.id,
                            name: tag.name,
                            categoryId: e.target.value,
                          });
                          await onReload();
                        } catch (err) {
                          onError(err instanceof Error ? err.message : String(err));
                        }
                      }}
                    >
                      {categories.map((option) => (
                        <option key={option.id} value={option.id}>
                          {option.name}
                        </option>
                      ))}
                    </select>
                    <button
                      className="danger"
                      onClick={async () => {
                        try {
                          await deleteGlobalTag(tag.id);
                          await onReload();
                        } catch (err) {
                          onError(err instanceof Error ? err.message : String(err));
                        }
                      }}
                    >
                      Delete
                    </button>
                  </div>
                ))
              )}
            </div>
          );
        })}
        <h3 className="subsection">Saved locations</h3>
        {locations.length === 0 ? (
          <div className="empty">No locations yet. Add a name, or type one in the preview.</div>
        ) : (
          locations.map((location) => (
            <div className="toolbar class-row" key={location.id}>
              <input
                key={`${location.id}:${location.name}`}
                type="text"
                defaultValue={location.name}
                onBlur={async (e) => {
                  const next = e.target.value.trim();
                  if (!next || next === location.name) return;
                  try {
                    await saveLocation({ id: location.id, name: next });
                    await onReload();
                  } catch (err) {
                    onError(err instanceof Error ? err.message : String(err));
                  }
                }}
              />
              <button
                className="danger"
                onClick={async () => {
                  try {
                    await deleteLocation(location.id);
                    await onReload();
                  } catch (err) {
                    onError(err instanceof Error ? err.message : String(err));
                  }
                }}
              >
                Delete
              </button>
            </div>
          ))
        )}
      </div>
    </section>
  );
}

function asMediaTag(tag: Pick<GlobalTag, "name" | "color" | "categoryId" | "categoryName">): MediaTag {
  return {
    name: tag.name,
    color: tag.color,
    categoryId: tag.categoryId,
    categoryName: tag.categoryName,
  };
}

function PreviewPane({
  item,
  index,
  total,
  onPrev,
  onNext,
  globalTags,
  categories,
  locations,
  marked,
  onMarkedChange,
  onDeleteCurrent,
  onTags,
  onCreateTag,
  onLocation,
  onExpand,
}: {
  item: MediaItem | null;
  index: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
  globalTags: GlobalTag[];
  categories: TagCategory[];
  locations: SavedLocation[];
  marked: boolean;
  onMarkedChange: (checked: boolean) => void;
  onDeleteCurrent: () => void;
  onTags: (tags: MediaTag[]) => void;
  onCreateTag: (name: string, categoryId: string) => Promise<GlobalTag>;
  onLocation: (label: string | null) => Promise<void>;
  onExpand: () => void;
}) {
  const [draft, setDraft] = useState("");
  const [draftCategoryId, setDraftCategoryId] = useState(categories[0]?.id ?? "");
  const [locationDraft, setLocationDraft] = useState(item?.locationLabel ?? "");
  const unused = globalTags.filter(
    (tag) => !item?.tags.some((assigned) => assigned.name.toLowerCase() === tag.name.toLowerCase()),
  );

  useEffect(() => {
    setLocationDraft(item?.locationLabel ?? "");
  }, [item?.id, item?.locationLabel]);

  useEffect(() => {
    if (draftCategoryId && categories.some((category) => category.id === draftCategoryId)) return;
    if (categories[0]) setDraftCategoryId(categories[0].id);
  }, [categories, draftCategoryId]);

  if (!item) {
    return (
      <aside className="preview-pane">
        <p className="lede">Select a frame to preview.</p>
        {total > 0 ? (
          <div className="toolbar">
            <button type="button" className="ghost" onClick={onPrev} aria-label="Previous file">
              Previous
            </button>
            <button type="button" className="ghost" onClick={onNext} aria-label="Next file">
              Next
            </button>
          </div>
        ) : null}
      </aside>
    );
  }
  const src = previewUrl(item.path);
  return (
    <aside className="preview-pane">
      <div className="preview-frame">
        {item.kind === "video" ? (
          <video src={src} controls />
        ) : item.kind === "photo" ? (
          <img src={src} alt={item.filename} />
        ) : (
          <span className="status">Sidecar — no preview</span>
        )}
        {total > 0 ? (
          <>
            <button type="button" className="preview-nav prev" onClick={onPrev} aria-label="Previous file">
              ‹
            </button>
            <button type="button" className="preview-nav next" onClick={onNext} aria-label="Next file">
              ›
            </button>
          </>
        ) : null}
        {item.kind === "sidecar" ? null : (
          <button
            type="button"
            className="preview-expand"
            onClick={onExpand}
            aria-label="Open larger preview"
            title="Larger preview"
          >
            <ExpandIcon />
          </button>
        )}
      </div>
      {total > 0 ? (
        <div className="preview-count">
          {index >= 0 ? index + 1 : 0} / {total}
        </div>
      ) : null}
      <div className="meta-list">
        <div>
          <strong>{item.filename}</strong>
        </div>
        <div>Camera {item.camera}</div>
        <div>Date {formatCaptureDate(item.capturedAt)} ({item.dateSource})</div>
        <form
          className="location-edit"
          onSubmit={async (event) => {
            event.preventDefault();
            const next = locationDraft.trim();
            await onLocation(next || null);
          }}
        >
          <label>
            Location
            <input
              type="text"
              list="preview-locations"
              placeholder="Add a place if GPS is missing"
              value={locationDraft}
              onChange={(e) => setLocationDraft(e.target.value)}
              onBlur={async () => {
                const next = locationDraft.trim();
                const current = item.locationLabel ?? "";
                if (next === current) return;
                await onLocation(next || null);
              }}
            />
          </label>
          <datalist id="preview-locations">
            {locations.map((location) => (
              <option key={location.id} value={location.name} />
            ))}
          </datalist>
        </form>
        <div>{formatBytes(item.size)}</div>
        <div>{item.organized ? "Shelved" : "Loose in the vault"}</div>
      </div>
      <div className="tag-row">
        {item.tags.map((tag) => (
          <button
            key={tag.name}
            className="tag"
            style={{ borderColor: tag.color, color: tag.color }}
            onClick={() => onTags(item.tags.filter((t) => t.name !== tag.name))}
          >
            {tag.name} ×
          </button>
        ))}
      </div>
      <form
        className="toolbar"
        onSubmit={async (event) => {
          event.preventDefault();
          const typed = draft.trim();
          const picked = unused.find((tag) => tag.name === typed) ?? (typed ? undefined : unused[0]);
          if (picked) {
            if (item.tags.some((t) => t.name.toLowerCase() === picked.name.toLowerCase())) {
              setDraft("");
              return;
            }
            onTags([...item.tags, asMediaTag(picked)]);
            setDraft("");
            return;
          }
          if (!typed || !draftCategoryId) return;
          if (item.tags.some((t) => t.name.toLowerCase() === typed.toLowerCase())) {
            setDraft("");
            return;
          }
          const known = globalTags.find((tag) => tag.name.toLowerCase() === typed.toLowerCase());
          const created = known ?? (await onCreateTag(typed, draftCategoryId));
          onTags([...item.tags, asMediaTag(created)]);
          setDraft("");
        }}
      >
        <select
          value={unused.some((tag) => tag.name === draft) ? draft : ""}
          onChange={(e) => setDraft(e.target.value)}
        >
          <option value="">Tag</option>
          {unused.map((tag) => (
            <option key={tag.id} value={tag.name}>
              {tag.name}
            </option>
          ))}
        </select>
        <input
          type="text"
          placeholder="Or new tag"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
        <select
          value={draftCategoryId}
          onChange={(e) => setDraftCategoryId(e.target.value)}
          title="Category for new tags"
          disabled={categories.length === 0}
        >
          {categories.map((category) => (
            <option key={category.id} value={category.id}>
              {category.name}
            </option>
          ))}
        </select>
        <button className="ghost" type="submit">
          Add
        </button>
      </form>
      <div className="delete-actions">
        <label className="tick delete-tick">
          <input
            type="checkbox"
            checked={marked}
            onChange={(e) => onMarkedChange(e.target.checked)}
          />
          To be deleted
        </label>
        <button className="danger" onClick={() => void onDeleteCurrent()}>
          Delete file
        </button>
      </div>
    </aside>
  );
}

function MediaStage({ item }: { item: MediaItem }) {
  const src = previewUrl(item.path);
  if (item.kind === "video") return <video src={src} controls />;
  if (item.kind === "photo") return <img src={src} alt={item.filename} />;
  return <span className="status">Sidecar — no preview</span>;
}

function MediaLightbox({
  item,
  index,
  total,
  onPrev,
  onNext,
  onClose,
}: {
  item: MediaItem;
  index: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
  onClose: () => void;
}) {
  return (
    <div className="lightbox" role="dialog" aria-modal="true" aria-label="Large preview">
      <div className="lightbox-frame">
        <MediaStage item={item} />
        {total > 0 ? (
          <>
            <button type="button" className="preview-nav prev" onClick={onPrev} aria-label="Previous file">
              ‹
            </button>
            <button type="button" className="preview-nav next" onClick={onNext} aria-label="Next file">
              ›
            </button>
          </>
        ) : null}
        <button type="button" className="lightbox-close" onClick={onClose} aria-label="Close large preview">
          ×
        </button>
        {total > 0 ? (
          <div className="lightbox-count">
            {index >= 0 ? index + 1 : 0} / {total}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function ExpandIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M9 3H3v6M15 3h6v6M9 21H3v-6M21 15v6h-6" />
    </svg>
  );
}

function SidebarIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="nav-icon"
      aria-hidden
    >
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <path d="M9 3v18" />
    </svg>
  );
}

function NavIcon({ id }: { id: NavIconId }) {
  const common = {
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.7,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    className: "nav-icon",
    "aria-hidden": true,
  };
  switch (id) {
    case "library":
      return (
        <svg {...common}>
          <rect x="3" y="4" width="7" height="7" />
          <rect x="14" y="4" width="7" height="7" />
          <rect x="3" y="13" width="7" height="7" />
          <rect x="14" y="13" width="7" height="7" />
        </svg>
      );
    case "organize":
      return (
        <svg {...common}>
          <path d="M4 6h16M4 12h10M4 18h13" />
          <path d="M16 10l4 2-4 2" />
        </svg>
      );
    case "import":
      return (
        <svg {...common}>
          <path d="M12 3v12" />
          <path d="M8 11l4 4 4-4" />
          <path d="M5 21h14" />
        </svg>
      );
    case "tags":
      return (
        <svg {...common}>
          <path d="M3 12l9-9h7v7l-9 9z" />
          <circle cx="16" cy="8" r="1.2" fill="currentColor" />
        </svg>
      );
    case "settings":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="3" />
          <path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1" />
        </svg>
      );
  }
}
