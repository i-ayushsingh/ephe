<p align="center"><img src="Logo.png" alt="ephe" width="180"></p>

# ephe

**Temporary text pad between your computer and your phone. Local network only. Nothing saved. Nothing left behind.**

Run `ephe.exe`, scan the QR with your phone, and type or drop files together. Close the window — everything is gone.

Can't scan the QR? Click **copy link** on the desktop to send the mobile URL.

- **Single portable binary** — no install, no admin, no runtime
- **Local network only** — no cloud, no relay, no telemetry, no account
- **In-memory only** — text and shared files (up to 25 MB) live strictly in RAM; wiped on session end
- **One binary, one process** — opens your default browser, serves the phone page, runs the WebSocket
- **MIT licensed**

## Quick start

1. Download **`ephe-portable.zip`** from the latest release (just `ephe.exe` + `Logo.png`).
2. Extract anywhere. Double-click `ephe.exe`.
3. Your default browser opens with a QR code.
4. Scan it with your phone (must be on the same Wi-Fi).
5. Type. Both sides see the same text in real time.
6. Close the browser tab, or `Ctrl+C` in the terminal.

That's it.

## Or: install with the NSIS installer

1. Download **`ephe-setup.exe`**.
2. Run it. It installs to `%LOCALAPPDATA%\ephe`, adds a Start menu shortcut, and registers an uninstaller.
3. Same usage.

## Requirements

- Windows 10 / 11 (x64)
- A phone on the same Wi-Fi as your computer
- On first run, Windows Firewall will ask to allow incoming connections — say **Yes** (private networks)

## Environment overrides

| Variable | Effect |
|---|---|
| `EPHE_PORT=8080` | Bind to a specific port instead of the default `42380` (with `40000–50000` fallback) |
| `EPHE_IP=192.168.x.x` | Force a specific LAN IP (if auto-detect picks a VPN/tunnel) |
| `EPHE_NO_BROWSER=1` | Run as a server only — no browser opens |

## What it does

- Starts a tiny HTTP + WebSocket server on port `42380` (or random `40000–50000` fallback)
- Detects the Wi-Fi IP (skipping virtual/VPN adapters), generates a QR code pointing at `http://<lan-ip>:<port>/?token=...`
- Opens your default browser at `http://localhost:<port>/?role=desktop&token=...`
- Phone scans QR → connects → both sides share a synced pad and in-memory file drop (up to 25 MB total; executables blocked)
- Up to two phones can connect to the same desktop session
- Concurrent edits merge cleanly over WebSocket with live latency indicator
- Session ends → text and files wiped from RAM

## What it does not do

- ❌ No persistence — no disk writes, no history, no autosave
- ❌ No network outside your LAN — no cloud, no telemetry, no analytics
- ❌ No third-party API calls — phone + desktop only
- ❌ No rich text, no formatting, no markdown
- ❌ No account, no login, no sign-up
- ✅ Up to **two phones** at a time (one desktop + two mobiles; a third is rejected)
- ❌ No mobile app — phone is just a browser

## Reset during a session

On the desktop, when the session ends (peer left, disconnected), a **"start new session"** button appears. Clicking it rotates the token and refreshes the QR. The phone has to re-scan — old URL is invalidated.

## File sharing (in-memory)

Drag & drop a file onto the editor or click the paperclip button (📎) to share files between connected devices.

- **Size limit**: **25 MB** total across all active files in a session (stored strictly in RAM, never written to disk, and wiped on delete or session end).
- **Allowed**: Any standard non-executable file — images (`.png`, `.jpg`, `.jpeg`, `.webp`, `.svg`, `.gif`), documents (`.pdf`, `.txt`, `.md`, `.csv`, `.json`, `.docx`), source/config files (`.rs`, `.py`, `.js`, `.ts`, `.html`, `.css`, `.yaml`), archives (`.zip`), audio/video, etc.
- **Not allowed (blocked)**: Executable and script file types are rejected for safety:
  `.exe`, `.bat`, `.cmd`, `.ps1`, `.scr`, `.vbs`, `.msi`, `.dll`, `.com`, `.pif`, `.hta`, `.cpl`, `.jar`

## Architecture

```
ephe.exe
├── src/main.rs    entry, picks port + IP, opens browser, runs until Ctrl+C
├── src/lib.rs     library root, test helpers
├── src/net.rs     LAN IP detection (skips VPN/tunnel interfaces, picks Wi-Fi)
├── src/server.rs  axum HTTP + WebSocket handler
├── src/session.rs in-memory session state, token, message types
├── src/index.html desktop + mobile page (served to both)
├── src/style.css  monochrome UI tokens
├── src/app.js     role detection, WebSocket, textarea sync
└── build.rs       generates Logo.ico from Logo.png, embeds as binary icon
```

10 runtime dependencies. ~900 KB release binary, opt-level z, LTO, stripped.

## License

MIT. See [LICENSE](LICENSE).

## Verifying

```
sha256sum ephe-setup.exe
sha256sum ephe-portable.zip
```

Compare against the checksums published in the release notes.