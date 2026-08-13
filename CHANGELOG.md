# ephe changelog

Running log of every change made to ephe, categorized for release notes and changelog generation. No decisions live here — see `other-docs/user-decisions.md` for the "why".

Categories:

- **Features added** — new capabilities
- **UI/UX changes or overhaul** — visual or interaction changes
- **Error and bug fixes** — things that were broken and got fixed
- **Other** — performance, build, packaging, docs, etc.

---

## v0.2.0 (released)

### Features added

- Desktop: collapsible QR panel ("hide qr" / "show qr" in the topbar). When collapsed, copy / paste / clear / start new session move to the topbar.
- One-click copy and paste buttons: desktop gets them in a row under the QR (next to "start new session"), mobile gets them in the topbar. Copy uses the Clipboard API with an `execCommand` fallback (non-secure `http://lan-ip` contexts); paste falls back to a prompt. Buttons flash "copied!/pasted!" on success.
- One-click clear button (desktop row + mobile topbar): wipes the shared text on both devices, flashes "cleared!".
- Quick theme toggle in the topbar on both desktop and mobile. Per-device: choice stored in that browser's `localStorage`, follows system until first toggle.
- Char/word counter in the desktop footer, updates live.
- "Start new session" is desktop-only (QR and new sessions live on the desktop; removed from mobile).

### UI/UX changes or overhaul

- Header brand is now the `text-logo.png` image instead of system-font "ephe" text. Inverted in light mode, as-is in dark (logo is white). Tab favicon stays `Logo.png`. Text-logo ships next to the exe (portable zip + installer).

### Error and bug fixes

- Start menu showed white generic file icon — installer never shipped `Logo.ico`; the shortcut pointed at a file that didn't exist. Added `File "Logo.ico"` to the install section.
- Installer baked in a stale `ephe.exe` — `build.ps1` only copied the fresh exe if `installer\ephe.exe` was missing, so an old build (without the `/text-logo.png` route) went into the setup. Now always copies the current build.

### Other

- Version 0.2.0.
- GitHub Actions CI: builds, runs tests, packages installer + portable zip, publishes GitHub Release with assets on `v*` tag push.

## v0.1.0 (released)

### Features added

- First release: LAN-only text pad between desktop and phone
- HTTP + WebSocket server on a random local port
- LAN IP auto-detection (Wi-Fi preferred over VPN/tunnel), `EPHE_IP` override
- QR code with session token for the phone
- Opens default browser as the desktop UI
- Shared textarea, ~16ms debounce keystroke sync
- Session wipe on disconnect, reset button for a new session
- NSIS installer + portable zip, embedded logo icon, favicon

### UI/UX changes or overhaul

- Monochrome, minimal, professional design (light/dark auto, reduced-motion support)
- Status indicator dot, QR panel with subtle fade-in
- QR code sized down to 180px per feedback
- Cursor drops to end of text on remote update

### Error and bug fixes

- LAN IP picker scoring (VPN-bound IP made phone unreachable)
- Disconnect teardown bug (first disconnect removed other peer too early)
- Mobile iOS zoom prevention (`font-size: 16px` on editor)

### Other

- 10 runtime dependencies, ~900 KB release binary (opt-level z, LTO, stripped)
- MIT license, README, SHA256SUMS