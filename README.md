# Instagram for Windows

[![Release](https://img.shields.io/github/v/release/taylorivanoff/instagram-windows)](https://github.com/taylorivanoff/instagram-windows/releases)
[![Downloads](https://img.shields.io/github/downloads/taylorivanoff/instagram-windows/total)](https://github.com/taylorivanoff/instagram-windows/releases)
[![License](https://img.shields.io/github/license/taylorivanoff/instagram-windows)](LICENSE)

Instagram desktop app for Windows. Loads [instagram.com](https://www.instagram.com) in a native **Tauri / WebView2** shell.

## Features

- **System tray** — close hides to tray; left-click toggles the window
- **Chrome user-agent** — desktop web session compatibility
- **Cookie persistence** — stay signed in across restarts
- **Start with Windows** — installer registers a login item
- **Deep link** — `instagram-windows://` protocol handler

## Installation

1. Download the latest installer from [Releases](https://github.com/taylorivanoff/instagram-windows/releases)
2. Run the installer (WebView2 Runtime is used if already installed; otherwise the bootstrapper downloads it)
3. Sign in with your Instagram account

## Security & authentication

This app is **not affiliated with Meta or Instagram**. It is an unofficial desktop wrapper around Instagram's web UI.

- You sign in on Instagram's own pages inside the WebView2 window
- Your password is never collected by this app
- Session cookies for Instagram / Meta domains are stored under this app's `%APPDATA%` folder only

## Development

Requires Rust (MSVC), WebView2, and Bun. Sibling crate [`tauri-tray-base`](https://github.com/taylorivanoff/tauri-tray-base) must sit at `../tauri-tray-base` relative to this repo (i.e. `Projects/tauri-tray-base`).

```bash
bun install
bun run icon          # regenerate icons from icon.png
bun run dev
```

### Release build

```bash
bun run release
```

Installer output: `src-tauri/target/release/bundle/nsis/`

## License

[MIT](LICENSE)
