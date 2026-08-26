import { useEffect, useMemo, useRef, useState } from "react";
import {
  executeOrganize,
  executeSync,
  getConfig,
  listTags,
  pickDirectory,
  previewOrganize,
  previewSync,
  previewUrl,
  saveConfig,
  scanLibrary,
  searchMedia,
  setMediaTags,
} from "./api";
import { hasActiveFilters, matchesLibraryFilters, type KindVisibility } from "./lib/filter";
import { formatBytes, formatCaptureDate, groupByDate } from "./lib/format";
import type {
  AppConfig,
  MediaItem,
  SearchQuery,
  Storage,
  StorageKind,
  TransferOp,
  ViewId,
} from "./types";

const NAV: { id: ViewId; label: string }[] = [
  { id: "library", label: "Library" },
  { id: "organize", label: "Organize" },
  { id: "sync", label: "Import" },
  { id: "settings", label: "Settings" },
];

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
  const [unorganizedOnly, setUnorganizedOnly] = useState(false);
  const [kinds, setKinds] = useState<KindVisibility>({ photo: true, video: true });
  const [search, setSearch] = useState<SearchQuery>({});
  const [tags, setTags] = useState<string[]>([]);
  const [ops, setOps] = useState<TransferOp[]>([]);
  const [selectedOps, setSelectedOps] = useState<Set<string>>(new Set());
  const [syncSource, setSyncSource] = useState("");
  const [syncStorageId, setSyncStorageId] = useState("");

  async function loadCatalog() {
    setBusy(true);
    setError(null);
    try {
      const cfg = await getConfig();
      setConfig(cfg);
      if (!cfg.storages.length) {
        setItems([]);
        setStatus("Add a media storage in Settings.");
        return;
      }
      const found = await searchMedia({});
      setItems(found);
      setTags(await listTags());
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
      setBusy(false);
    }
  }

  async function refresh() {
    setBusy(true);
    setError(null);
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
      setTags(await listTags());
      setStatus(`${scanned.length} files in catalog.`);
      if (!syncStorageId && cfg.storages[0]) {
        setSyncStorageId(cfg.storages[0].id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void loadCatalog();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const visible = useMemo(
    () => items.filter((item) => matchesLibraryFilters(item, search, unorganizedOnly, kinds)),
    [items, search, unorganizedOnly, kinds],
  );
  const groups = useMemo(() => groupByDate(visible), [visible]);

  useEffect(() => {
    if (selected && !visible.some((item) => item.path === selected.path)) {
      setSelected(null);
    }
  }, [visible, selected]);

  return (
    <div className="app-shell">
      <aside className="sprocket" aria-hidden="true">
        <div className="sprocket-holes" />
      </aside>
      <nav className="nav">
        <h1 className="wordmark">
          CAMERA
          <br />
          <span>ORGANIZER</span>
        </h1>
        <div className="nav-links">
          {NAV.map((item) => (
            <button
              key={item.id}
              className={view === item.id ? "active" : ""}
              onClick={() => setView(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
        <p className={error ? "status error" : "status"}>{error ?? status}</p>
      </nav>
      <main className="stage">
        {view === "library" && (
          <LibraryView
            groups={groups}
            selected={selected}
            unorganizedOnly={unorganizedOnly}
            kinds={kinds}
            search={search}
            tags={tags}
            matchCount={visible.length}
            busy={busy}
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
          />
        )}
        {view === "organize" && (
          <OrganizeView
            config={config}
            ops={ops}
            selectedOps={selectedOps}
            busy={busy}
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
              }
            }}
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
              }
            }}
          />
        )}
      </main>
      <PreviewPane
        item={selected}
        onTags={async (nextTags) => {
          if (!selected) return;
          const saved = await setMediaTags(selected.id, nextTags);
          setSelected({ ...selected, tags: saved });
          setItems((current) =>
            current.map((item) => (item.id === selected.id ? { ...item, tags: saved } : item)),
          );
        }}
      />
    </div>
  );
}

function LibraryView({
  groups,
  selected,
  unorganizedOnly,
  kinds,
  search,
  tags,
  matchCount,
  busy,
  onToggleUnorganized,
  onKindsChange,
  onSearchChange,
  onClearFilters,
  onRefresh,
  onSelect,
}: {
  groups: Map<string, MediaItem[]>;
  selected: MediaItem | null;
  unorganizedOnly: boolean;
  kinds: KindVisibility;
  search: SearchQuery;
  tags: string[];
  matchCount: number;
  busy: boolean;
  onToggleUnorganized: () => void;
  onKindsChange: (kinds: KindVisibility) => void;
  onSearchChange: (query: SearchQuery) => void;
  onClearFilters: () => void;
  onRefresh: () => void;
  onSelect: (item: MediaItem) => void;
}) {
  const [layout, setLayout] = useState<LibraryLayout>("thumbs");
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
            Scan library
          </button>
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
          <button className={unorganizedOnly ? "primary" : "ghost"} onClick={onToggleUnorganized}>
            {unorganizedOnly ? "Showing unorganized" : "Show unorganized"}
          </button>
          {filtering ? (
            <button className="ghost" onClick={onClearFilters}>
              Clear filters
            </button>
          ) : null}
          {filtering ? <span className="status">{matchCount} matching</span> : null}
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
          value={search.location ?? ""}
          onChange={(e) => onSearchChange({ ...search, location: e.target.value })}
        />
        <input
          type="date"
          value={search.dateFrom ?? ""}
          onChange={(e) => onSearchChange({ ...search, dateFrom: e.target.value })}
        />
        <input
          type="date"
          value={search.dateTo ?? ""}
          onChange={(e) => onSearchChange({ ...search, dateTo: e.target.value })}
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
          value={search.tag ?? ""}
          onChange={(e) => onSearchChange({ ...search, tag: e.target.value || null })}
        >
          <option value="">All tags</option>
          {tags.map((tag) => (
            <option key={tag}>{tag}</option>
          ))}
        </select>
      </div>
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
                    <span className="file-name">{item.filename}</span>
                    <span className="file-type">{item.kind}</span>
                    <span className="file-tags">
                      {item.tags.length ? item.tags.join(", ") : "—"}
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
  const [active, setActive] = useState(false);

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

  const src = active ? previewUrl(item.path) : "";
  return (
    <div ref={slot} className="thumb-slot">
      {active && src && item.kind === "video" ? (
        <video src={src} muted playsInline preload="metadata" />
      ) : null}
      {active && src && item.kind !== "video" ? <img src={src} alt="" loading="lazy" /> : null}
    </div>
  );
}

function OrganizeView({
  config,
  ops,
  selectedOps,
  busy,
  onPreview,
  onToggle,
  onExecute,
}: {
  config: AppConfig;
  ops: TransferOp[];
  selectedOps: Set<string>;
  busy: boolean;
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
          One library disk, or several — local, mounted network, or a plugged-in drive. Paths are saved
          in the app config file.
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

function PreviewPane({
  item,
  onTags,
}: {
  item: MediaItem | null;
  onTags: (tags: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  if (!item) {
    return (
      <aside className="preview-pane">
        <p className="lede">Select a frame to preview.</p>
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
      </div>
      <div className="meta-list">
        <div>
          <strong>{item.filename}</strong>
        </div>
        <div>Camera {item.camera}</div>
        <div>Date {formatCaptureDate(item.capturedAt)} ({item.dateSource})</div>
        <div>Location {item.locationLabel ?? "—"}</div>
        <div>{formatBytes(item.size)}</div>
        <div>{item.organized ? "Shelved" : "Loose in the vault"}</div>
      </div>
      <div className="tag-row">
        {item.tags.map((tag) => (
          <button
            key={tag}
            className="tag"
            onClick={() => onTags(item.tags.filter((t) => t !== tag))}
          >
            {tag} ×
          </button>
        ))}
      </div>
      <form
        className="toolbar"
        onSubmit={(event) => {
          event.preventDefault();
          const tag = draft.trim();
          if (!tag) return;
          onTags([...item.tags, tag]);
          setDraft("");
        }}
      >
        <input
          type="text"
          placeholder="Add tag"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
        <button className="ghost" type="submit">
          Tag
        </button>
      </form>
    </aside>
  );
}
