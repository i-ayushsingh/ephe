# ephe changelog

Running log of every change made to ephe, categorized for release notes and changelog generation. No decisions live here — see `other-docs/user-decisions.md` for the "why".

Categories:

- **Features added** — new capabilities
- **UI/UX changes or overhaul** — visual or interaction changes
- **Error and bug fixes** — things that were broken and got fixed
- **Other** — performance, build, packaging, docs, etc.

---

## v0.12.0

### Features added

- **Non-blocking in-app paste sheet** — replaced the disruptive native browser `window.prompt()` fallback with a sleek bottom-sheet modal for clipboard paste on mobile/HTTP networks. Supports <kbd>Ctrl</kbd>/<kbd>Cmd</kbd>+<kbd>Enter</kbd> quick insert.
- **Enhanced dialog accessibility & focus trapping** — added `aria-labelledby` linkages on all modal dialogs, automatic active element focus restore on close, loop keyboard focus trapping (<kbd>Tab</kbd> / <kbd>Shift</kbd>+<kbd>Tab</kbd>), and accurate shortcut documentation (removed non-working `Ctrl+F` row).

### UI/UX changes or overhaul

- **Mobile action bar hierarchy** — promoted Copy and Paste buttons to primary prominent controls with accent borders; transformed secondary tools into a cleaner, minimal presentation.
- **Unified 4-tier typography scale** — consolidated arbitrary subpixel font sizes into standard tokens (`--text-xs`, `--text-sm`, `--text-base`, `--text-lg`) for crisp rendering across all displays.
- **Polished dark mode QR presentation** — updated the QR container card to blend with theme tokens while keeping a clean white inset for the QR code to ensure 100% camera scannability.
- **Consolidated action DOM** — eliminated redundant duplicated button definitions between the header and footer, streamlining the DOM and reducing layout overhead.
- **Enhanced toast feedback system** — categorized toasts with semantic types (success, info, error) with tailored display durations (1.5s, 2.5s, 3.5s) and smooth stacking animations.

### Error and bug fixes

- **WebSocket payload size limits** — enforced 1MB text limit on incoming messages and WebSocket upgrade frame/message limits (`1MB + 4KB`) to prevent memory exhaustion.
- **Cross-origin /reset protection (CSRF)** — enforced `X-Ephe-Client` header verification on `/reset`, preventing unauthorized reset triggers from cross-origin pages.
- **Constant-time token validation** — implemented timing-safe constant-time comparison for session tokens.
- **CI pipeline formatting & clippy lints** — resolved rustfmt formatting and clippy lints across all test suites and modules for reliable GitHub Actions verification.

### Other

- Version 0.12.0.

---

## v0.11.0 (released)

### Features added

- **Modal Escape key support** — pressing <kbd>Escape</kbd> now closes any open overlay dialog (keyboard shortcuts reference, text transform sheet, and time-travel history).
- **Dialog & screen reader accessibility** — added proper `aria-modal="true"` dialog semantics and descriptive alt text for the connection QR code.

### UI/UX changes or overhaul

- **Mobile connection status dot restored** — mobile devices now show a live connection status dot in the header, giving immediate visual feedback whether connected, reconnecting, or disconnected.
- **Fixed mobile scroll behavior** — eliminated jarring viewport jumps to the bottom of the editor when tapping to edit text on mobile devices.

### Error and bug fixes

- **Server-side pad lock enforcement** — pad lock mode is now strictly verified on the backend; when locked by desktop, text edits from mobile peers are rejected server-side.
- **Truly self-contained binary** — embedded all visual assets (`Logo.png`, `text-logo.png`, `PressStart2P-Regular.ttf`) directly into `ephe.exe` via compile-time binary embedding, ensuring execution from any folder.
- **Fixed portable release packaging** — added missing retro font (`PressStart2P-Regular.ttf`) to GitHub release portable archives.

### Other

- Added pull request and push CI verification workflow (`.github/workflows/ci.yml`) with formatting, clippy lints, and test execution.
- Version 0.11.0.

---

## v0.10.0 (released)

### Features added

- **PortPeek integration** — ephe now pairs with [PortPeek](https://github.com/i-ayushsingh/PortPeek), a small utility for seeing and opening the apps running on your desktop. While an ephe session is running, you can see it as an active session in PortPeek and open it straight from there — no scanning, no copying links.
- **Start a new session from PortPeek** — with ephe installed, PortPeek can start a brand-new ephe session on your desktop on demand.
- **Open an existing session as the host** — clicking a running ephe session in PortPeek opens it as the desktop host (QR code and all host controls), not as a secondary device. The host URL stays safe by design: no session token ever leaves your machine, and only devices on your network with the token can join. (Warning: this may sometimes not work — the developer is still fixing it.)
- **Better connection status** — the status dot now tells you what is actually happening: "connected · 2 phones" when everything is fine (with the phone count), "reconnecting…" and "connection lost" when the connection drops, and "session ended" when the session is over. If the connection drops briefly, ephe tries to get you back in automatically — you land in a fresh pad instead of a dead screen.
- **Char/word/line statistics** — the counter now also shows the line count: "1,248 chars · 214 words · 38 lines".

### Other

- Version 0.10.0.

---

## v0.9.0 (released)

### Features added

- **Transform menu** — the single case button becomes a small Transform menu: UPPERCASE, lowercase, Title Case, Sentence case, Trim whitespace, Remove empty lines. Every transformation syncs to all devices and is undoable.
- **Mobile bottom action bar** — undo (↶) and redo (↷) move from the topbar to the bottom, with arrow symbols instead of text; the Transform menu opens from a ⋮ button at the bottom centre, all on one row. Mobile topbar is decluttered.
- **Keyboard shortcut reference** — desktop only: a "keys" panel listing the expected shortcuts (Ctrl+A select all, Ctrl+C copy, Ctrl+V paste, Ctrl+Z undo, Ctrl+Y redo, Ctrl+F find), each of which works natively in the browser.

### Other

- Version 0.9.0.

---

## v0.8.0 (released)

### Features added

- **Font toggle** — cycle between 5 editor fonts: system sans, serif, mono, cursive, and a pixel/retro font (Press Start 2P, OFL-licensed, bundled with the app) that echoes the ephe logo look. Per-device, remembered between sessions.
- **Word wrap toggle** — switch the editor between wrapped lines and hard line breaks with horizontal scroll. Per-device, remembered.
- **Case conversion** — one click turns the whole text UPPERCASE, the next lowercase, the next back to the original case; each change syncs to every device and is undoable.

### Other

- Version 0.8.0.

---

## v0.7.0 (released)

### Features added

- **Quick undo / redo** — dedicated undo and redo buttons (topbar on mobile, footer on desktop) step back and forward through your last 5 edits instantly, synced to every device.
- **Time-travel restore fixed & one-click** — restoring an old history snapshot no longer needs a confirm dialog and always applies to the whole session (the previous confirm flow could misbehave in some browsers and leave peers out of sync); a dead session shows "session ended" instead of silently changing only one device.
- **Connected phones bar** — the topbar's center shows the connected phones (phone 1 · phone 2); each has an × the desktop can click to remove it from the session.
- **Read receipts** — when a peer's browser round-trips your text, a "seen on phone 1 & phone 2" pill appears over the editor; useful with two phones to know who actually got it.

### Other

- Version 0.7.0.
- New integration test: devices list, seen receipts, and kick flow over live WebSockets (13 tests total).

---

## v0.6.0 (released)

### Features added

- **Text-size reset** — when A−/A+ move the text size away from default, a "size reset" button appears next to them (footer on desktop, topbar on mobile) and snaps it back to 16px.
- **Third-phone inline rejection** — a third mobile is now turned away at the join screen with a persistent "session full" notice over the editor instead of a flash toast and a dead pad.
- **Time-travel undo (desktop)** — ephe keeps a rolling history of the pad (a snapshot every ~1.5s of activity, last 50 kept); the "history" button opens a timeline to restore any past state — restoring rewinds the pad for everyone.

---

## v0.5.0 (released)

### UI/UX changes or overhaul

- Desktop action buttons (copy / paste / clear / lock pad / A− / A+) moved out of the QR panel into the footer, centered between the "start typing" hint and the char counter — collapsing the QR no longer shuffles buttons to the topbar, the layout stays put. Mobile unchanged (buttons stay in the topbar).

---

## v0.4.0 (released)

### Features added

- "someone is typing…" indicator — a debounced relayed typing message shows a transient pill over the editor while another peer is typing, and auto-hides ~2s after they stop (or when their text arrives). — a debounced relayed typing message shows a transient pill over the editor while another peer is typing, and auto-hides ~2s after they stop (or when their text arrives).
- Caret is no longer yanked to the bottom on remote keystrokes — if your cursor sits in the middle or you're scrolled up reading, it stays put and a "new text below ↓" pill appears instead; the caret only jumps to the end when it was already near the end.
- Lock pad (single-writer mode, desktop only): a "lock pad" button takes exclusive write access — every other device goes read-only with a "pad locked by desktop" pill until the desktop unlocks. Lock state survives desktop reloads (sent in `init`) and resets with a new session. Phones cannot lock.

### Error and bug fixes

- With two phones connected, killing one phone's app (or its Wi-Fi dropping) left the desktop showing "2 phones connected" forever — the server never noticed the dead peer and never wiped the session. The server now pings every WebSocket peer every 10s (protocol-level ping; browsers auto-pong even in background tabs) and drops any peer silent for 30s, so a force-killed phone is detected, the session wipes, and every remaining peer gets `peer_left` like any other disconnect. Heartbeat period is tunable via `EPHE_HEARTBEAT_SECS`.
- Peer count no longer survives a disconnect — a phone rejoining after a wipe reports "1 phone connected", not a stale higher count.
- Pressing A+ at the maximum text size (or A− at the minimum) now shows a toast ("maximum size reached" / "minimum size reached") instead of silently doing nothing.

---

## v0.3.0 (released)

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