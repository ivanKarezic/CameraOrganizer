# Camera Organizer

Desktop app for a DJI / GoPro / Insta360 library: scan disks, search by metadata, shelve files into date folders, and copy missing clips off an SD card.

## Stack

- Tauri 2 + React + TypeScript
- Rust core (scan, organize, sync, SQLite catalog)

## Layout on disk

Existing library files are **moved**. Files coming from a card or other external volume are **copied**.

```text
{storage}/2024/2024-08-26/Video/
{storage}/2024/2024-08-26/Photo/
```

## Develop

Rust (stable) and Node 20+ are required.

```bash
npm install
npm run test:all
npm run tauri dev
```

Config is saved as TOML in the OS app-config directory. The catalog is a local SQLite file in the app data directory.
