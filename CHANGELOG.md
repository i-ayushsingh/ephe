# ephe changelog

Running log of every change made to ephe, categorized for release notes and changelog generation. No decisions live here — see `other-docs/user-decisions.md` for the "why".

Categories:

- **Features added** — new capabilities
- **UI/UX changes or overhaul** — visual or interaction changes
- **Error and bug fixes** — things that were broken and got fixed
- **Other** — performance, build, packaging, docs, etc.

---

## v0.3.0 (unreleased)

### Features added

- Up to two phones per session (one desktop + two mobiles; a third is rejected with "session full"). Text from any peer reaches all others; any disconnect still wipes the session.
- Mobile link shown as plain text under the QR with a one-click "copy link" button — QR fallback when the camera can't scan.
- Toast notifications replace button-text flashes ("copied!", "pasted!", "cleared!", "link copied!").
- Text size +/− controls (desktop action row + mobile topbar, per-device via localStorage, 12–24px).
- Native share button (Web Share API) next to "copy link" — opens the system share sheet on the desktop, falls back to copying the link.
- Desktop caption shows how many phones are connected ("1 phone connected" / "2 phones connected"); both sides get a toast when a phone joins.

### UI/UX changes or overhaul

- Mobile topbar shows status dot only (label hidden) to fit the new buttons.
- Compact button sizing on narrow screens; phones drop the status dot entirely so copy / paste / clear / A− / A+ / theme always fit.
- "start new session" now asks for confirmation — an accidental click can no longer wipe a session and kick both phones.

### Error and bug fixes

- Second phone's status dot stayed "waiting" forever — peer_joined is now also sent to the joining phone.
- Rejection reasons ("session full: max 2 mobiles", "invalid token", "desktop already connected") were clobbered instantly by the socket close and hidden on phones — they now show as toasts (phones) and captions (desktop).
- `/reset` accepted requests from any device on the LAN — it now requires the current session token.
- Reset token rotation raced under concurrent requests — token is now generated inside the session lock.
- QR card never dimmed on session error — the CSS selector referenced elements that are not siblings.
- WebSocket `error` events left the editor editable — they now lock it and show reset, like `close` events.

### Other

-

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