import { FormEvent, useEffect, useMemo, useState } from "react";
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
  { id: "search", label: "Search" },
  { id: "organize", label: "Organize" },
  { id: "sync", label: "Import" },
  { id: "settings", label: "Settings" },
];

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
  const [search, setSearch] = useState<SearchQuery>({});
  const [tags, setTags] = useState<string[]>([]);
  const [ops, setOps] = useState<TransferOp[]>([]);
  const [selectedOps, setSelectedOps] = useState<Set<string>>(new Set());
  const [syncSource, setSyncSource] = useState("");
  const [syncStorageId, setSyncStorageId] = useState("");

  async function refresh(nextQuery?: SearchQuery) {
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
      const query = nextQuery ?? (unorganizedOnly ? { unorganizedOnly: true } : {});
      const found = Object.values(query).some((v) => v)
        ? await searchMedia(query)
        : scanned;
      setItems(found);
      setTags(await listTags());
      setStatus(`${found.length} files in catalog.`);
      if (selected) {
        setSelected(found.find((item) => item.id === selected.id) ?? found[0] ?? null);
      }
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
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const visible = useMemo(
    () => (unorganizedOnly && view === "library" ? items.filter((i) => !i.organized) : items),
    [items, unorganizedOnly, view],
  );
  const groups = useMemo(() => groupByDate(visible.filter((i) => i.kind !== "sidecar")), [visible]);

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
            busy={busy}
            onToggleUnorganized={() => setUnorganizedOnly((v) => !v)}
            onRefresh={() => void refresh()}
            onSelect={setSelected}
          />
        )}
        {view === "search" && (
          <SearchView
            search={search}
            tags={tags}
            items={items}
            selected={selected}
            onChange={setSearch}
            onSearch={() => void refresh(search)}
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
  busy,
  onToggleUnorganized,
  onRefresh,
  onSelect,
}: {
  groups: Map<string, MediaItem[]>;
  selected: MediaItem | null;
  unorganizedOnly: boolean;
  busy: boolean;
  onToggleUnorganized: () => void;
  onRefresh: () => void;
  onSelect: (item: MediaItem) => void;
}) {
  return (
    <section>
      <h2 className="section-title">CONTACT SHEET</h2>
      <p className="lede">
        A vault of camera originals. Dates come from metadata or the filename; tags stay searchable
        even when files live across disks.
      </p>
      <div className="toolbar">
        <button className="primary" onClick={onRefresh} disabled={busy}>
          Scan library
        </button>
        <button className={unorganizedOnly ? "primary" : "ghost"} onClick={onToggleUnorganized}>
          {unorganizedOnly ? "Showing unorganized" : "Show unorganized"}
        </button>
      </div>
      {groups.size === 0 ? (
        <div className="empty">No media yet. Add a storage path, then scan.</div>
      ) : (
        [...groups.entries()].map(([day, files]) => (
          <div className="day-block" key={day}>
            <div className="day-label">{day}</div>
            <div className="grid">
              {files.map((item) => (
                <button
                  key={item.path}
                  className={selected?.path === item.path ? "card selected" : "card"}
                  onClick={() => onSelect(item)}
                >
                  <span className="card-kind">{item.kind}</span>
                  <span className="badge">{item.camera}</span>
                  {!item.organized && <span className="badge unorganized">loose</span>}
                  <span className="card-name">{item.filename}</span>
                </button>
              ))}
            </div>
          </div>
        ))
      )}
    </section>
  );
}

function SearchView({
  search,
  tags,
  items,
  selected,
  onChange,
  onSearch,
  onSelect,
}: {
  search: SearchQuery;
  tags: string[];
  items: MediaItem[];
  selected: MediaItem | null;
  onChange: (query: SearchQuery) => void;
  onSearch: () => void;
  onSelect: (item: MediaItem) => void;
}) {
  function submit(event: FormEvent) {
    event.preventDefault();
    onSearch();
  }
  return (
    <section>
      <h2 className="section-title">FIND</h2>
      <p className="lede">Search the catalog by tag, camera, date, or the GPS label pulled from the file.</p>
      <form className="toolbar" onSubmit={submit}>
        <input
          type="search"
          placeholder="Filename, tag, camera…"
          value={search.text ?? ""}
          onChange={(e) => onChange({ ...search, text: e.target.value })}
        />
        <input
          type="text"
          placeholder="Location"
          value={search.location ?? ""}
          onChange={(e) => onChange({ ...search, location: e.target.value })}
        />
        <input
          type="date"
          value={search.dateFrom ?? ""}
          onChange={(e) => onChange({ ...search, dateFrom: e.target.value })}
        />
        <input
          type="date"
          value={search.dateTo ?? ""}
          onChange={(e) => onChange({ ...search, dateTo: e.target.value })}
        />
        <select
          value={search.camera ?? ""}
          onChange={(e) => onChange({ ...search, camera: e.target.value || null })}
        >
          <option value="">All cameras</option>
          <option>DJI</option>
          <option>GoPro</option>
          <option>Insta360</option>
          <option>Unknown</option>
        </select>
        <select
          value={search.tag ?? ""}
          onChange={(e) => onChange({ ...search, tag: e.target.value || null })}
        >
          <option value="">All tags</option>
          {tags.map((tag) => (
            <option key={tag}>{tag}</option>
          ))}
        </select>
        <button className="primary" type="submit">
          Search
        </button>
      </form>
      <div className="grid">
        {items.map((item) => (
          <button
            key={item.path}
            className={selected?.path === item.path ? "card selected" : "card"}
            onClick={() => onSelect(item)}
          >
            <span className="card-kind">{formatCaptureDate(item.capturedAt)}</span>
            <span className="badge">{item.camera}</span>
            <span className="card-name">{item.filename}</span>
          </button>
        ))}
      </div>
    </section>
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
    <section>
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
      <OpTable ops={ops} selectedOps={selectedOps} onToggle={onToggle} />
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
    <section>
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
      <OpTable ops={ops} selectedOps={selectedOps} onToggle={onToggle} />
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
    <section>
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
