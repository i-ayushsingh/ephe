# PortPeek integration

Optional discovery integration between ephe and the PortPeek Windows utility.
ephe publishes a small metadata file for every running session so PortPeek can
list and open them. Without PortPeek (or without `%LOCALAPPDATA%`, e.g.
Linux/macOS) nothing changes — ephe runs exactly as before.

## Manifest location

```
%LOCALAPPDATA%\Ephe\PortPeek\<session-id>.json
```

One file per running ephe session. `<session-id>` is the session's stable id
(`Session.id`, random hex, generated at startup).

## Schema

```json
{
  "schemaVersion": 1,
  "application": "Ephe",
  "applicationId": "ephe",
  "sessionId": "3fa85f64",
  "sessionName": "Ephe Session",
  "version": "0.10.0",
  "port": 42317,
  "localUrl": "http://localhost:42317",
  "hostUrl": "http://localhost:42317/?role=desktop",
  "status": "active",
  "connectedDevices": 2
}
```

| Field             | Meaning                                                        |
|-------------------|----------------------------------------------------------------|
| `schemaVersion`   | Manifest format version (currently 1).                         |
| `application`     | Human name: `Ephe`.                                            |
| `applicationId`   | Stable machine id: `ephe`.                                     |
| `sessionId`       | Stable id of this session (the manifest filename stem).        |
| `sessionName`     | Session display name. ephe has no naming concept, so always `Ephe Session`. |
| `version`         | ephe version (`CARGO_PKG_VERSION`).                            |
| `port`            | The actual listening TCP port.                                 |
| `localUrl`        | `http://localhost:<port>` — plain entry point.                |
| `hostUrl`         | `http://localhost:<port>/?role=desktop` — opens the desktop host UI (QR, host controls). |
| `status`          | Always `active`.                                               |
| `connectedDevices`| Desktop + connected phones currently in the session.           |

PortPeek should open `hostUrl` to open an existing session as the desktop
host. `hostUrl` **intentionally contains no session token**: loopback
(localhost) is trusted by the server exactly like the desktop WebSocket join,
so the host UI — including its reset/start-new-session operation — works
tokenless; LAN secondary devices still require the token, which never appears
in the manifest.

PortPeek should ignore unknown fields (additive evolution only).

## Lifecycle

- **Startup** — after the TCP listener binds, ephe sweeps stale manifests
  (see below) and writes its own manifest with `connectedDevices: 0`.
- **Device joins/leaves** — the manifest is rewritten whenever a desktop or
  phone connects or disconnects (`connectedDevices` changes). Session resets
  funnel through the same path.
- **Clean shutdown** (Ctrl+C) — the manifest is deleted.
- **Crash** — the manifest lingers. Two layers handle it:
  1. On the next ephe startup, `sweep_stale()` deletes any manifest whose
     reported port no longer accepts TCP connections.
  2. PortPeek independently validates reachability and ignores dead entries.

## Security and privacy

The manifest intentionally contains **only** discovery data:
no tokens, passwords, session text, or device ids. `hostUrl` opens the
desktop host UI without a token because loopback is trusted — the same rule
that lets the user's own browser join as desktop; the reset/start-new-session
operation is loopback-only, so a LAN client without the token cannot use it.
The manifest exposes the port (already visible via `netstat`) and the
session's existence. The directory is under the current user's
`%LOCALAPPDATA%`, so only that user can read it.

## Versioning

`schemaVersion` is bumped only on breaking format changes. PortPeek must
ignore unknown fields, so ordinary additions do not bump it.

## Launcher Contract

PortPeek can ask ephe to create a new session:

```
ephe.exe --new-session
```

Exactly what happens:

1. A new normal ephe session is created via the existing session-creation path.
2. The local server starts on a fresh random port.
3. The PortPeek manifest is published through the existing integration.
4. The session opens in the user's default browser (unless `EPHE_NO_BROWSER` is set), exactly like a plain `ephe.exe` launch.

`--new-session` is not a special session type — it is the same code path a
plain launch takes. Running it while another ephe instance is active starts
an independent second session (separate process, separate port, separate
manifest); ephe has no process-reuse mechanism and intentionally adds none.

Plain `ephe.exe` with no arguments behaves exactly as before.

Unknown or extra arguments print a usage error and exit non-zero, before any
session or manifest is created — no malformed manifests, no stale state.

Only `--new-session` exists today; future commands (e.g.
`--open-session <session-id>`) would slot into the same CLI parser.

## Testing

Covered by `tests/manifest.rs`: creation, field correctness (port, url, id,
name, device count), update, removal, stale sweep (dead vs live ports), and
three simultaneous sessions co-existing in the same directory.

The launcher contract is covered by `tests/cli.rs`: `--new-session` spawns a
real session with a listening port and a correct manifest, plain launches are
unchanged, two invocations yield two independent sessions, and invalid
arguments fail cleanly without leaving state.