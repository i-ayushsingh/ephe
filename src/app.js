// ponytail: vanilla, no framework. wire url -> role -> websocket -> editor.

const params = new URLSearchParams(location.search);
const isDesktop = params.get("role") === "desktop";
const token = params.get("token") || "";
const isMac = /Mac|iPhone|iPad/.test(navigator.platform);

document.body.classList.remove("role-desktop", "role-mobile");
document.body.classList.add(isDesktop ? "role-desktop" : "role-mobile");

const statusEl = document.getElementById("status");
const statusLabel = document.getElementById("status-label");
const captionEl = document.getElementById("caption");
const editor = document.getElementById("editor");
const footerHint = document.getElementById("footer-hint");
const footerCount = document.getElementById("footer-count");
const collapseBtn = document.getElementById("collapse-btn");

// ---------- transient hint pill over the editor ----------

const hintEl = document.getElementById("editor-hint");
let hintTimer = null;

function showHint(text, sticky) {
  if (!hintEl) return;
  hintEl.textContent = text;
  hintEl.hidden = false;
  clearTimeout(hintTimer);
  if (!sticky) hintTimer = setTimeout(() => { hintEl.hidden = true; }, 2000);
}

function hideHint() {
  clearTimeout(hintTimer);
  if (hintEl) hintEl.hidden = true;
}

// ---------- toasts ----------

const toastsEl = document.getElementById("toasts");

function toast(text, type = "success") {
  if (!toastsEl) return;
  const el = document.createElement("div");
  el.className = "toast";
  if (type !== "success") el.dataset.type = type;
  el.textContent = text;
  toastsEl.appendChild(el);
  requestAnimationFrame(() => el.classList.add("in"));
  const duration = type === "error" ? 3500 : (type === "info" ? 2500 : 1500);
  setTimeout(() => {
    el.classList.remove("in");
    el.classList.add("out");
    setTimeout(() => el.remove(), 200);
  }, duration);
}

// ---------- char/word counter ----------

function updateCounter() {
  if (footerCount) {
    const t = editor.value;
    const words = t.trim() ? t.trim().split(/\s+/).length : 0;
    const lines = t ? t.split("\n").length : 0;
    footerCount.textContent =
      `${t.length.toLocaleString()} chars · ${words} words · ${lines} lines`;
  }
  updateLineNumbers();
}

// ---------- collapse the QR panel (desktop) ----------

function setCollapsed(c) {
  document.body.classList.toggle("qr-collapsed", c);
  if (collapseBtn) collapseBtn.textContent = c ? "show qr" : "hide qr";
}

if (collapseBtn) {
  collapseBtn.addEventListener("click", () => {
    setCollapsed(!document.body.classList.contains("qr-collapsed"));
  });
}

// ---------- theme toggle (per-device, stored in this browser's localStorage) ----------

const themeBtn = document.getElementById("theme-btn");

function effectiveTheme() {
  if (document.documentElement.dataset.theme) return document.documentElement.dataset.theme;
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function setTheme(t, persist) {
  document.documentElement.dataset.theme = t;
  if (persist) localStorage.setItem("ephe-theme", t);
  themeBtn.textContent = t === "dark" ? "\u2600" : "\u263E";
  themeBtn.setAttribute("aria-label", t === "dark" ? "switch to light theme" : "switch to dark theme");
  document.querySelector('meta[name="theme-color"]').setAttribute("content", t === "dark" ? "#0a0a0a" : "#ffffff");
}

if (themeBtn) {
  themeBtn.addEventListener("click", () => {
    setTheme(effectiveTheme() === "dark" ? "light" : "dark", true);
  });
  const saved = localStorage.getItem("ephe-theme");
  if (saved) {
    setTheme(saved, true);
  } else {
    // no saved choice: follow the system, no attribute so media query stays live
    const t = effectiveTheme();
    themeBtn.textContent = t === "dark" ? "\u2600" : "\u263E";
    themeBtn.setAttribute("aria-label", t === "dark" ? "switch to light theme" : "switch to dark theme");
    document.querySelector('meta[name="theme-color"]').setAttribute("content", t === "dark" ? "#0a0a0a" : "#ffffff");
  }
}

// ---------- modal focus & accessibility helpers ----------

let lastFocusedElement = null;

function openModal(panelEl, focusTargetSelector) {
  if (!panelEl) return;
  lastFocusedElement = document.activeElement;
  panelEl.hidden = false;
  const target = focusTargetSelector
    ? panelEl.querySelector(focusTargetSelector)
    : panelEl.querySelector("button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])");
  if (target) setTimeout(() => target.focus(), 50);
}

function closeModal(panelEl) {
  if (!panelEl || panelEl.hidden) return;
  panelEl.hidden = true;
  if (lastFocusedElement && typeof lastFocusedElement.focus === "function") {
    lastFocusedElement.focus();
  }
}

function trapFocus(e, panelEl) {
  if (e.key !== "Tab" || !panelEl || panelEl.hidden) return;
  const focusables = panelEl.querySelectorAll("button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])");
  if (!focusables.length) return;
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

// ---------- copy / paste ----------

async function copyValue(text, okLabel) {
  if (!text) return;
  let ok = false;
  try {
    await navigator.clipboard.writeText(text);
    ok = true;
  } catch {
    // ponytail: execCommand fallback for non-secure contexts (http://lan-ip)
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    ok = document.execCommand("copy");
    ta.remove();
  }
  if (ok) toast(okLabel);
}

function insertAtCursor(text) {
  const start = editor.selectionStart, end = editor.selectionEnd;
  editor.value = editor.value.slice(0, start) + text + editor.value.slice(end);
  const pos = start + text.length;
  editor.setSelectionRange(pos, pos);
  editor.dispatchEvent(new Event("input", { bubbles: true }));
}

async function copyText() {
  await copyValue(editor.value, "copied!");
}

// ---------- in-app paste fallback (when clipboard permissions are unavailable) ----------

const pastePanel = document.getElementById("paste-panel");
const pasteInput = document.getElementById("paste-input");
const pasteSubmit = document.getElementById("paste-submit");
const pasteClose = document.getElementById("paste-close");

function showPasteModal() {
  if (pasteInput) pasteInput.value = "";
  openModal(pastePanel, "#paste-input");
}

function hidePaste() {
  closeModal(pastePanel);
}

if (pasteSubmit) {
  pasteSubmit.addEventListener("click", () => {
    const val = pasteInput ? pasteInput.value : "";
    if (val) {
      insertAtCursor(val);
      toast("pasted!");
    }
    hidePaste();
  });
}
if (pasteClose) {
  pasteClose.addEventListener("click", hidePaste);
}
if (pastePanel) {
  pastePanel.addEventListener("click", (e) => {
    if (e.target === pastePanel) hidePaste();
  });
  pastePanel.addEventListener("keydown", (e) => trapFocus(e, pastePanel));
}
if (pasteInput) {
  pasteInput.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      pasteSubmit.click();
    }
  });
}

async function pasteText() {
  if (editor.readOnly) return;
  try {
    if (navigator.clipboard && navigator.clipboard.readText) {
      const text = await navigator.clipboard.readText();
      if (text) {
        insertAtCursor(text);
        toast("pasted!");
        return;
      }
    }
  } catch {}
  showPasteModal();
}

const copyBtn = document.getElementById("copy-btn");
if (copyBtn) copyBtn.addEventListener("click", copyText);

const pasteBtn = document.getElementById("paste-btn");
if (pasteBtn) pasteBtn.addEventListener("click", pasteText);

// ---------- clear ----------

function clearText() {
  if (editor.readOnly || !editor.value) return;
  editor.value = "";
  editor.setSelectionRange(0, 0);
  editor.dispatchEvent(new Event("input", { bubbles: true }));
  toast("cleared!");
}

const clearBtn = document.getElementById("clear-btn");
if (clearBtn) clearBtn.addEventListener("click", clearText);

// ---------- download as file ----------

function downloadText() {
  if (!editor.value) {
    toast("nothing to download", "info");
    return;
  }
  const blob = new Blob([editor.value], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  const now = new Date();
  const dateStr = now.toISOString().slice(0, 10);
  a.href = url;
  a.download = `ephe-${dateStr}.txt`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  toast("downloaded!");
}

const downloadBtn = document.getElementById("download-btn");
if (downloadBtn) downloadBtn.addEventListener("click", downloadText);

// ---------- line numbers toggle ----------

const LINES_KEY = "ephe-line-numbers";
const gutter = document.getElementById("gutter");
const linesBtn = document.getElementById("lines-btn");
let showLines = localStorage.getItem(LINES_KEY) === "1";

function updateLineNumbers() {
  if (!showLines || !gutter) return;
  const lineCount = (editor.value.match(/\n/g) || []).length + 1;
  let s = "";
  for (let i = 1; i <= lineCount; i++) {
    s += i + (i === lineCount ? "" : "\n");
  }
  gutter.textContent = s;
}

function applyLineNumbers() {
  if (!gutter || !linesBtn) return;
  gutter.hidden = !showLines;
  linesBtn.textContent = showLines ? "lines on" : "lines off";
  localStorage.setItem(LINES_KEY, showLines ? "1" : "0");
  if (showLines) {
    updateLineNumbers();
    gutter.scrollTop = editor.scrollTop;
  }
}

if (linesBtn) {
  linesBtn.addEventListener("click", () => {
    showLines = !showLines;
    applyLineNumbers();
    toast(showLines ? "line numbers enabled" : "line numbers disabled", "info");
  });
}
applyLineNumbers();

editor.addEventListener("scroll", () => {
  if (showLines && gutter) {
    gutter.scrollTop = editor.scrollTop;
  }
});

// ---------- drag and drop text import ----------

editor.addEventListener("dragover", (e) => {
  e.preventDefault();
  if (editor.readOnly) return;
  editor.classList.add("drag-over");
});

editor.addEventListener("dragleave", (e) => {
  e.preventDefault();
  editor.classList.remove("drag-over");
});

editor.addEventListener("drop", async (e) => {
  e.preventDefault();
  editor.classList.remove("drag-over");
  if (editor.readOnly) return;
  const files = e.dataTransfer ? e.dataTransfer.files : null;
  if (!files || files.length === 0) return;
  const file = files[0];
  if (file.size > 1_048_576) {
    toast("file too large (max 1MB)", "error");
    return;
  }
  try {
    const text = await file.text();
    if (text !== undefined) {
      commitHistory(editor.value);
      applyTextBroadcast(text, "file imported!");
      updateUndoRedo();
      updateLineNumbers();
    }
  } catch {
    toast("could not read file", "error");
  }
});

// ---------- mobile link fallback (QR alternative) ----------

const urlRow = document.getElementById("url-row");
const urlText = document.getElementById("url-text");

if (isDesktop && urlRow) {
  fetch("/url")
    .then((r) => (r.ok ? r.json() : null))
    .then((data) => {
      if (!data || !data.url) return;
      urlText.value = data.url;
      urlRow.hidden = false;
    })
    .catch(() => {});
  urlText.addEventListener("focus", () => urlText.select());
  document.getElementById("copy-url-btn").addEventListener("click", () => {
    copyValue(urlText.value, "link copied!");
  });
  const shareBtn = document.getElementById("share-btn");
  if (shareBtn && navigator.share) {
    shareBtn.hidden = false;
    shareBtn.addEventListener("click", async () => {
      try {
        await navigator.share({ title: "ephe", text: "join my ephe session", url: urlText.value });
      } catch (e) {
        if (e.name !== "AbortError") copyValue(urlText.value, "link copied!");
      }
    });
  }
}

// ---------- text size (per-device) ----------

const SIZE_KEY = "ephe-text-size";
const MIN_SIZE = 12, MAX_SIZE = 24;
let textSize = parseInt(localStorage.getItem(SIZE_KEY), 10) || 16;

function applyTextSize() {
  textSize = Math.min(MAX_SIZE, Math.max(MIN_SIZE, textSize));
  editor.style.fontSize = textSize + "px";
  if (gutter) gutter.style.fontSize = textSize + "px";
  localStorage.setItem(SIZE_KEY, String(textSize));
  for (const b of document.querySelectorAll(".size-reset")) {
    b.hidden = textSize === 16;
  }
}

for (const btn of document.querySelectorAll(".size-reset")) {
  btn.addEventListener("click", () => {
    textSize = 16;
    applyTextSize();
  });
}

for (const btn of document.querySelectorAll("#size-up-btn, #size-down-btn")) {
  btn.addEventListener("click", () => {
    const up = btn.id.includes("up");
    const next = textSize + (up ? 2 : -2);
    if (next > MAX_SIZE || next < MIN_SIZE) {
      toast(up ? "maximum size reached" : "minimum size reached", "info");
      return;
    }
    textSize = next;
    applyTextSize();
  });
}
applyTextSize();

// ---------- font toggle (per-device, 4 common + 1 bundled pixel) ----------

const FONTS = [
  { key: "system", family: "ui-sans-serif, system-ui, -apple-system, 'Segoe UI', Roboto, Arial, sans-serif" },
  { key: "serif", family: "Georgia, 'Times New Roman', serif" },
  { key: "mono", family: "ui-monospace, 'Cascadia Mono', Consolas, monospace" },
  { key: "cursive", family: "'Comic Sans MS', 'Segoe Script', cursive" },
  { key: "pixel", family: "'Pixel Logo', 'Press Start 2P', monospace" },
];
const FONT_KEY = "ephe-font";
let fontIdx = Math.max(0, FONTS.findIndex((f) => f.key === localStorage.getItem(FONT_KEY)));

function applyFont() {
  editor.style.fontFamily = FONTS[fontIdx].family;
  if (gutter) gutter.style.fontFamily = FONTS[fontIdx].family;
  localStorage.setItem(FONT_KEY, FONTS[fontIdx].key);
  for (const b of document.querySelectorAll(".font-toggle")) {
    b.textContent = "font: " + FONTS[fontIdx].key;
  }
}

for (const btn of document.querySelectorAll(".font-toggle")) {
  btn.addEventListener("click", () => {
    fontIdx = (fontIdx + 1) % FONTS.length;
    applyFont();
  });
}
applyFont();

// ---------- word wrap toggle (per-device) ----------

const WRAP_KEY = "ephe-wrap";
let wrapOn = localStorage.getItem(WRAP_KEY) !== "0";

function applyWrap() {
  editor.classList.toggle("no-wrap", !wrapOn);
  localStorage.setItem(WRAP_KEY, wrapOn ? "1" : "0");
  for (const b of document.querySelectorAll(".wrap-toggle")) {
    b.textContent = wrapOn ? "wrap on" : "wrap off";
  }
}

for (const btn of document.querySelectorAll(".wrap-toggle")) {
  btn.addEventListener("click", () => {
    wrapOn = !wrapOn;
    applyWrap();
  });
}
applyWrap();

const COPY_KEY = isMac ? "\u2318" : "Ctrl";

if (footerHint) {
  footerHint.innerHTML = `start typing &nbsp;·&nbsp; <kbd>${COPY_KEY}</kbd>+<kbd>C</kbd> to copy`;
}

const captions = {
  waiting: "scan to connect a phone",
  connected: "",
  lost: "connection lost — check your network",
  reconnecting: "check your network",
  error: "session ended",
};

function phoneLabel(n) {
  if (!n) return "connected";
  return "connected · " + n + (n === 1 ? " phone" : " phones");
}

function setStatus(state, label) {
  statusEl.dataset.state = state;
  statusLabel.textContent = label;
  if (captionEl) captionEl.textContent = captions[state] || "";
  document.body.classList.toggle("session-ended", state === "error" || state === "lost");
}

function showReset() {
  if (!isDesktop) return; // QR (and new sessions) live on the desktop only
  for (const b of document.querySelectorAll(".reset-btn")) b.hidden = false;
}

for (const resetBtn of document.querySelectorAll(".reset-btn")) {
  resetBtn.addEventListener("click", async () => {
    if (resetBtn.disabled) return;
    if (!confirm("End this session for everyone?")) return;
    resetBtn.disabled = true;
    const original = resetBtn.textContent;
    resetBtn.textContent = "starting\u2026";
    try {
      const res = await fetch("/reset?token=" + encodeURIComponent(token), {
        method: "POST",
        headers: { "X-Ephe-Client": "true" },
      });
      if (!res.ok) throw new Error("reset failed");
      const data = await res.json();
      location.href = data.url;
    } catch (e) {
      resetBtn.disabled = false;
      resetBtn.textContent = original;
    }
  });
}

const wsProto = location.protocol === "https:" ? "wss:" : "ws:";
const wsUrl = `${wsProto}//${location.host}/ws?token=${encodeURIComponent(token)}`;
let ws = null;
let retryCount = 0;
let everJoined = false;

function connect() {
  const sock = new WebSocket(wsUrl);
  ws = sock;
  sock.addEventListener("open", () => {
    retryCount = 0;
    document.body.classList.remove("loading", "session-ended");
    setStatus("idle", "waiting");
    if (everJoined) {
      everJoined = false;
      toast("reconnected", "info");
    }
  });
  sock.addEventListener("message", onMessage);
  sock.addEventListener("close", () => onDrop(sock));
  sock.addEventListener("error", () => onDrop(sock));
}

// ponytail: on mobile or background tabs, when the browser is minimized
// or the user switches apps, mobile browsers pause or close WebSockets.
// We avoid exhausting retries to a terminal error while hidden, and
// reconnect automatically when the user returns to the page.
function onDrop(sock) {
  if (statusEl.dataset.state === "error") return;
  if (ws !== sock || sock._dropped) return; // superseded or already handled
  sock._dropped = true;

  // If the browser is currently hidden/minimized, pause reconnection retries
  // until the page becomes visible again. Mobile OS blocks/throttles background sockets.
  if (document.visibilityState === "hidden") {
    setStatus("lost", "connection paused");
    return;
  }

  if (retryCount >= 5) {
    editor.readOnly = true;
    showReset();
    setStatus("error", "session ended");
    return;
  }
  retryCount++;
  setStatus("lost", "connection lost");
  setTimeout(() => {
    if (document.visibilityState === "hidden") return;
    setStatus("reconnecting", "reconnecting\u2026");
    connect();
  }, 700 * retryCount);
}

function checkConnectionOnResume() {
  if (document.visibilityState === "visible") {
    if (!ws || ws.readyState === WebSocket.CLOSED || ws.readyState === WebSocket.CLOSING) {
      retryCount = 0;
      if (!isDesktop || statusEl.dataset.state !== "error") {
        document.body.classList.remove("session-ended");
        if (!padLocked && !isDesktop) {
          editor.readOnly = false;
        }
        setStatus("reconnecting", "reconnecting\u2026");
        connect();
      }
    }
  }
}

document.addEventListener("visibilitychange", checkConnectionOnResume);
window.addEventListener("pageshow", checkConnectionOnResume);
window.addEventListener("focus", checkConnectionOnResume);

let applying = false;
let lastSent = "";
let lastTypingSent = 0;
let peerCount = 0;
let joinedSelf = false;
let padLocked = false;
// read receipts: which peers have seen my most recent text
let seenRoundTrip = new Set();

function applyLock() {
  if (!isDesktop) editor.readOnly = padLocked;
  for (const b of document.querySelectorAll(".lock-btn")) {
    b.textContent = padLocked ? "unlock pad" : "lock pad";
    b.classList.toggle("locked", padLocked);
  }
  if (!isDesktop) {
    if (padLocked) showHint("pad locked by desktop");
    else hideHint();
  }
}

for (const btn of document.querySelectorAll(".lock-btn")) {
  btn.addEventListener("click", () => {
    padLocked = !padLocked;
    applyLock();
    if (ws.readyState === 1) {
      ws.send(JSON.stringify({ type: "lock", locked: padLocked }));
    }
  });
}

// ---------- time-travel undo (desktop) ----------
// ponytail: client-side snapshot history — every settled state (debounced
// 1.5s) is captured and can be restored, which broadcasts the old text to
// everyone. Cap 50 snapshots; the desktop reloading loses history, fine for
// an ephemeral pad.

const historyPanel = document.getElementById("history-panel");
const historyList = document.getElementById("history-list");
const historyEmpty = document.getElementById("history-empty");
const HISTORY_CAP = 50;
let history = [];
let historyTimer = null;

function captureHistory() {
  if (!isDesktop) return;
  clearTimeout(historyTimer);
  historyTimer = setTimeout(() => {
    const last = history[history.length - 1];
    if (last && last.text === editor.value) return;
    history.push({ text: editor.value, ts: Date.now() });
    if (history.length > HISTORY_CAP) history.shift();
    renderHistory();
  }, 1500);
}

function renderHistory() {
  if (!historyList) return;
  historyList.textContent = "";
  historyEmpty.hidden = history.length > 0;
  for (let i = history.length - 1; i >= 0; i--) {
    const snap = history[i];
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.type = "button";
    b.className = "history-item";
    const time = document.createElement("span");
    time.className = "history-time";
    time.textContent = new Date(snap.ts).toLocaleTimeString();
    b.appendChild(time);
    b.appendChild(document.createTextNode(` · ${snap.text.length} chars`));
    b.addEventListener("click", (e) => {
      e.preventDefault();
      restoreSnapshot(snap);
    });
    li.appendChild(b);
    historyList.appendChild(li);
  }
}

function hideHistory() {
  if (historyPanel) historyPanel.hidden = true;
}

// ---------- shared "apply a text state and broadcast it" ----------
// used by restore, undo and redo; never applies locally when the WS is dead

function applyTextBroadcast(text, okLabel) {
  if (ws.readyState !== 1) {
    toast("session ended", "error");
    return false;
  }
  applying = true;
  editor.value = text;
  editor.setSelectionRange(text.length, text.length);
  lastSent = text;
  applying = false;
  updateCounter();
  seenRoundTrip.clear();
  ws.send(JSON.stringify({ type: "text", text }));
  if (okLabel) toast(okLabel);
  captureHistory();
  return true;
}

function restoreSnapshot(snap) {
  commitHistory(editor.value);
  if (applyTextBroadcast(snap.text, "restored!")) hideHistory();
  updateUndoRedo();
}

// ---------- quick undo / redo (max 5 levels each) ----------

const MAX_UNDO = 5;
let undoStack = [];
let redoStack = [];

function commitHistory(oldText) {
  if (oldText === editor.value) return;
  if (undoStack[undoStack.length - 1] !== oldText) {
    undoStack.push(oldText);
    if (undoStack.length > MAX_UNDO) undoStack.shift();
  }
  redoStack = [];
  updateUndoRedo();
}

function updateUndoRedo() {
  for (const b of document.querySelectorAll(".undo-btn")) {
    b.disabled = !undoStack.length;
  }
  for (const b of document.querySelectorAll(".redo-btn")) {
    b.disabled = !redoStack.length;
  }
}

function doUndo() {
  const prev = undoStack.pop();
  if (prev === undefined) return;
  redoStack.push(editor.value);
  if (redoStack.length > MAX_UNDO) redoStack.shift();
  updateUndoRedo();
  applyTextBroadcast(prev);
}

function doRedo() {
  const next = redoStack.pop();
  if (next === undefined) return;
  undoStack.push(editor.value);
  if (undoStack.length > MAX_UNDO) undoStack.shift();
  updateUndoRedo();
  applyTextBroadcast(next);
}

for (const btn of document.querySelectorAll(".undo-btn")) {
  btn.addEventListener("click", doUndo);
}
for (const btn of document.querySelectorAll(".redo-btn")) {
  btn.addEventListener("click", doRedo);
}

// ---------- connected-phones bar (desktop) ----------

const devicesEl = document.getElementById("devices");

function renderDevices(list) {
  if (!devicesEl) return;
  devicesEl.textContent = "";
  for (const d of list || []) {
    const chip = document.createElement("span");
    chip.className = "device-chip";
    const name = document.createElement("span");
    name.textContent = d.label;
    const rm = document.createElement("button");
    rm.type = "button";
    rm.className = "device-remove";
    rm.setAttribute("aria-label", "remove " + d.label);
    rm.textContent = "\u00D7";
    rm.addEventListener("click", () => {
      if (!confirm(`Remove ${d.label} from this session?`)) return;
      ws.send(JSON.stringify({ type: "kick", id: d.id }));
    });
    chip.appendChild(name);
    chip.appendChild(rm);
    devicesEl.appendChild(chip);
  }
}

const historyBtn = document.getElementById("history-btn");
function showHistory() {
  renderHistory();
  openModal(historyPanel, ".history-item, #history-close");
}
function hideHistory() {
  closeModal(historyPanel);
}
if (historyBtn) {
  historyBtn.addEventListener("click", showHistory);
}
document.getElementById("history-close").addEventListener("click", hideHistory);
if (historyPanel) {
  historyPanel.addEventListener("click", (e) => {
    if (e.target === historyPanel) hideHistory();
  });
  historyPanel.addEventListener("keydown", (e) => trapFocus(e, historyPanel));
}

// ---------- transform menu (mobile: bottom-centre ⋮ button) ----------

const transformPanel = document.getElementById("transform-panel");
const transformList = document.getElementById("transform-list");

function showTransform() {
  openModal(transformPanel, ".transform-item");
}
function hideTransform() {
  closeModal(transformPanel);
}

const TRANSFORMS = [
  { label: "UPPERCASE", apply: (t) => t.toUpperCase() },
  { label: "lowercase", apply: (t) => t.toLowerCase() },
  { label: "Title Case", apply: (t) => t.replace(/\S+/g, (w) => w[0].toUpperCase() + w.slice(1).toLowerCase()) },
  { label: "Sentence case", apply: (t) => t.replace(/(^\s*\w|[.!?]\s+\w)/g, (c) => c.toUpperCase()) },
  { label: "Trim whitespace", apply: (t) => t.trim() },
  { label: "Remove empty lines", apply: (t) => t.split("\n").filter((l) => l.trim() !== "").join("\n") },
];

if (transformList) {
  for (const tr of TRANSFORMS) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "btn btn-sm transform-item";
    b.textContent = tr.label;
    b.addEventListener("click", () => {
      if (!editor.value) { toast("nothing to transform", "info"); return; }
      const out = tr.apply(editor.value);
      if (out === editor.value) { toast("nothing to change", "info"); return; }
      commitHistory(editor.value);
      if (applyTextBroadcast(out, "transformed")) hideTransform();
      updateUndoRedo();
    });
    transformList.appendChild(b);
  }
}

const transformBtn = document.getElementById("transform-btn");
if (transformBtn) {
  transformBtn.addEventListener("click", () => {
    if (transformPanel.hidden) showTransform();
    else hideTransform();
  });
}
if (transformPanel) {
  transformPanel.addEventListener("click", (e) => {
    if (e.target === transformPanel) hideTransform();
  });
  transformPanel.addEventListener("keydown", (e) => trapFocus(e, transformPanel));
}
document.getElementById("transform-close").addEventListener("click", hideTransform);

// ---------- keyboard shortcuts reference (desktop only, native in browser) ----------

const shortcutsPanel = document.getElementById("shortcuts-panel");
function showShortcuts() {
  openModal(shortcutsPanel, "#shortcuts-close");
}
function hideShortcuts() {
  closeModal(shortcutsPanel);
}
const shortcutsBtn = document.getElementById("shortcuts-btn");
if (shortcutsBtn) {
  shortcutsBtn.addEventListener("click", () => {
    if (shortcutsPanel.hidden) showShortcuts();
    else hideShortcuts();
  });
}
if (shortcutsPanel) {
  shortcutsPanel.addEventListener("click", (e) => {
    if (e.target === shortcutsPanel) hideShortcuts();
  });
  shortcutsPanel.addEventListener("keydown", (e) => trapFocus(e, shortcutsPanel));
}
document.getElementById("shortcuts-close").addEventListener("click", hideShortcuts);

// ---------- in-app qr camera scanner (mobile / PWA) ----------

let scanStream = null;
let scanTimer = null;
const scanPanel = document.getElementById("scan-panel");
const scanVideo = document.getElementById("scan-video");
const scanClose = document.getElementById("scan-close");
const scanBtn = document.getElementById("scan-btn");

async function startScan() {
  openModal(scanPanel, "#scan-close");
  const scanHint = document.getElementById("scan-hint");
  if (scanHint) scanHint.textContent = "point camera at the desktop QR code";
  try {
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      toast("camera not supported", "error");
      return;
    }
    scanStream = await navigator.mediaDevices.getUserMedia({
      video: { facingMode: "environment" },
    });
    if (scanVideo) {
      scanVideo.srcObject = scanStream;
      scanVideo.play();
    }
    if ("BarcodeDetector" in window) {
      const detector = new BarcodeDetector({ formats: ["qr_code"] });
      scanTimer = setInterval(async () => {
        if (!scanVideo || scanVideo.readyState < 2) return;
        try {
          const codes = await detector.detect(scanVideo);
          if (codes && codes.length > 0) {
            handleScannedUrl(codes[0].rawValue);
          }
        } catch {}
      }, 250);
    } else {
      if (scanHint) scanHint.textContent = "scanning...";
    }
  } catch (err) {
    toast("camera permission denied", "error");
    hideScan();
  }
}

function handleScannedUrl(raw) {
  stopScan();
  hideScan();
  try {
    const url = new URL(raw, location.href);
    const scannedToken = url.searchParams.get("token");
    if (scannedToken) {
      toast("connecting...", "info");
      location.href = url.href;
      return;
    }
  } catch {}
  toast("invalid ephe QR code", "error");
}

function stopScan() {
  if (scanTimer) {
    clearInterval(scanTimer);
    scanTimer = null;
  }
  if (scanStream) {
    scanStream.getTracks().forEach((track) => track.stop());
    scanStream = null;
  }
  if (scanVideo) {
    scanVideo.srcObject = null;
  }
}

function hideScan() {
  stopScan();
  closeModal(scanPanel);
}

if (scanBtn) scanBtn.addEventListener("click", startScan);
if (scanClose) scanClose.addEventListener("click", hideScan);
if (scanPanel) {
  scanPanel.addEventListener("click", (e) => {
    if (e.target === scanPanel) hideScan();
  });
  scanPanel.addEventListener("keydown", (e) => trapFocus(e, scanPanel));
}

// ponytail: modals must close on Escape for keyboard accessibility
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    hideShortcuts();
    hideTransform();
    hideHistory();
    hidePaste();
    hideScan();
  }
});

function onMessage(ev) {
  let msg;
  try { msg = JSON.parse(ev.data); } catch { return; }
  switch (msg.type) {
    case "init":
      everJoined = true;
      joinedSelf = false;
      applying = true;
      editor.value = msg.text || "";
      lastSent = editor.value;
      padLocked = !!msg.locked;
      applyLock();
      applying = false;
      updateCounter();
      captureHistory();
      undoStack = [];
      redoStack = [];
      updateUndoRedo();
      if (isDesktop) {
        editor.focus({ preventScroll: true });
      } else {
        peerCount = 1;
        setStatus("connected", "connected");
      }
      break;
    case "text":
      if (msg.text === lastSent) break;
      commitHistory(editor.value);
      // ponytail: keep the caret where it was (reading a scrolled-up spot)
      // unless it sat near the old end — only then jump to the new end.
      const prevLen = editor.value.length;
      const prevStart = editor.selectionStart;
      const wasNearEnd = prevStart >= prevLen - 8;
      applying = true;
      editor.value = msg.text || "";
      const len = editor.value.length;
      if (wasNearEnd) {
        editor.setSelectionRange(len, len);
        hideHint();
      } else {
        const pos = Math.min(prevStart, len);
        editor.setSelectionRange(pos, pos);
        if (len > prevLen) showHint("new text below \u2193");
      }
      lastSent = editor.value;
      applying = false;
      updateCounter();
      captureHistory();
      // read receipt: I got this text — tell the peers (phones only)
      if (!isDesktop && ws.readyState === 1) {
        ws.send(JSON.stringify({ type: "ack" }));
      }
      break;
    case "typing":
      showHint("someone is typing\u2026");
      break;
    case "locked":
      padLocked = !!msg.locked;
      applyLock();
      break;
    case "peer_joined":
      if (isDesktop) {
        // the desktop's own join also arrives here; the authoritative phone
        // count comes with the devices list right after
        if (!joinedSelf) joinedSelf = true;
        else toast("phone connected", "info");
      } else {
        peerCount++;
        setStatus("connected", phoneLabel(peerCount));
      }
      break;
    case "peer_left":
      peerCount = 0;
      joinedSelf = false;
      editor.readOnly = true;
      showReset();
      if (devicesEl) devicesEl.textContent = "";
      setStatus("error", "session ended");
      break;
    case "devices":
      renderDevices(msg.devices);
      if (isDesktop) {
        if (msg.devices.length > 0) {
          setStatus("connected", phoneLabel(msg.devices.length));
        } else {
          setStatus("idle", "waiting");
        }
      }
      break;
    case "seen":
      seenRoundTrip.add(msg.label);
      showHint("seen on " + [...seenRoundTrip].join(" & "));
      break;
    case "kicked":
      // the desktop removed this phone from the session; terminal, no reconnect
      editor.readOnly = true;
      showHint("removed by desktop", true);
      setStatus("error", "session ended");
      break;
    case "reset":
      // server rotated the session and told us to navigate to the new URL
      location.href = msg.url;
      break;
    case "error":
      setStatus("error", msg.reason || "rejected");
      // phones hide the status label, so the reason goes in a toast
      toast(msg.reason || "session ended", "error");
      if (isDesktop && captionEl) captionEl.textContent = msg.reason || "session ended";
      if (!isDesktop) showHint(msg.reason || "session ended", true);
      editor.readOnly = true;
      showReset();
      break;
  }
}

let sendTimer = null;
editor.addEventListener("input", () => {
  updateCounter();
  if (applying) return;
  const prev = lastSent;
  if (editor.value === lastSent) return;
  lastSent = editor.value;
  seenRoundTrip.clear();
  commitHistory(prev);
  if (ws.readyState !== 1) return;
  // ponytail: throttle typing pings — one per 800ms burst, the debounced text
  // send resolves the indicator anyway.
  if (Date.now() - lastTypingSent > 800) {
    lastTypingSent = Date.now();
    ws.send(JSON.stringify({ type: "typing" }));
  }
  // ponytail: tiny debounce so a fast typist doesn't send a packet per keystroke
  clearTimeout(sendTimer);
  sendTimer = setTimeout(() => {
    if (ws.readyState === 1) {
      ws.send(JSON.stringify({ type: "text", text: editor.value }));
    }
  }, 16);
  captureHistory();
});

// ponytail: on mobile, let the browser handle focus scrolling natively.
// Previous code forced scrollTop = scrollHeight which jumped the viewport
// to the bottom even when the user tapped mid-document.

connect();