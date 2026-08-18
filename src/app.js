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

function toast(text) {
  const el = document.createElement("div");
  el.className = "toast";
  el.textContent = text;
  toastsEl.appendChild(el);
  requestAnimationFrame(() => el.classList.add("in"));
  setTimeout(() => {
    el.classList.remove("in");
    el.classList.add("out");
    setTimeout(() => el.remove(), 200);
  }, 1500);
}

// ---------- char/word counter ----------

function updateCounter() {
  if (!footerCount) return;
  const t = editor.value;
  const words = t.trim() ? t.trim().split(/\s+/).length : 0;
  footerCount.textContent = `${t.length} chars · ${words} words`;
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

async function pasteText() {
  if (editor.readOnly) return;
  let text = null;
  try {
    text = await navigator.clipboard.readText();
  } catch {
    text = window.prompt("paste text below:");
  }
  if (!text) return;
  insertAtCursor(text);
  toast("pasted!");
}

for (const btn of document.querySelectorAll(".top-copy, #copy-btn")) {
  btn.addEventListener("click", copyText);
}
for (const btn of document.querySelectorAll(".top-paste, #paste-btn")) {
  btn.addEventListener("click", pasteText);
}

// ---------- clear ----------

function clearText() {
  if (editor.readOnly || !editor.value) return;
  editor.value = "";
  editor.setSelectionRange(0, 0);
  editor.dispatchEvent(new Event("input", { bubbles: true }));
  toast("cleared!");
}

for (const btn of document.querySelectorAll(".top-clear, #clear-btn")) {
  btn.addEventListener("click", clearText);
}

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

for (const btn of document.querySelectorAll(".top-size, #size-up-btn, #size-down-btn")) {
  btn.addEventListener("click", () => {
    const up = btn.id.includes("up");
    const next = textSize + (up ? 2 : -2);
    if (next > MAX_SIZE || next < MIN_SIZE) {
      toast(up ? "maximum size reached" : "minimum size reached");
      return;
    }
    textSize = next;
    applyTextSize();
  });
}
applyTextSize();

const COPY_KEY = isMac ? "\u2318" : "Ctrl";

if (footerHint) {
  footerHint.innerHTML = `start typing &nbsp;·&nbsp; <kbd>${COPY_KEY}</kbd>+<kbd>C</kbd> to copy`;
}

const captions = {
  waiting: "scan to connect a phone",
  connected: "phone connected",
  error: "session ended",
};

function setStatus(state, label) {
  statusEl.dataset.state = state;
  statusLabel.textContent = label;
  if (captionEl) captionEl.textContent = captions[label] || "";
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
      const res = await fetch("/reset?token=" + encodeURIComponent(token), { method: "POST" });
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
const ws = new WebSocket(wsUrl);

let applying = false;
let lastSent = "";
let lastTypingSent = 0;
let peerCount = 0;
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
    toast("session ended");
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
  for (const b of document.querySelectorAll(".undo-btn, .top-undo")) {
    b.disabled = !undoStack.length;
  }
  for (const b of document.querySelectorAll(".redo-btn, .top-redo")) {
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

for (const btn of document.querySelectorAll(".undo-btn, .top-undo")) {
  btn.addEventListener("click", doUndo);
}
for (const btn of document.querySelectorAll(".redo-btn, .top-redo")) {
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

const historyBtn = document.getElementById("top-history");
if (historyBtn) {
  historyBtn.addEventListener("click", () => {
    renderHistory();
    historyPanel.hidden = false;
  });
}
document.getElementById("history-close").addEventListener("click", hideHistory);
if (historyPanel) {
  historyPanel.addEventListener("click", (e) => {
    if (e.target === historyPanel) hideHistory();
  });
}

ws.addEventListener("open", () => {
  document.body.classList.remove("loading");
  setStatus("idle", "waiting");
});

ws.addEventListener("message", (ev) => {
  let msg;
  try { msg = JSON.parse(ev.data); } catch { return; }
  switch (msg.type) {
    case "init":
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
      if (isDesktop) editor.focus({ preventScroll: true });
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
      peerCount++;
      setStatus("connected", "connected");
      if (isDesktop && captionEl) {
        captionEl.textContent = peerCount === 1 ? "1 phone connected" : peerCount + " phones connected";
      }
      toast("phone connected");
      break;
    case "peer_left":
      peerCount = 0;
      setStatus("error", "error");
      editor.readOnly = true;
      showReset();
      if (devicesEl) devicesEl.textContent = "";
      break;
    case "devices":
      renderDevices(msg.devices);
      break;
    case "seen":
      seenRoundTrip.add(msg.label);
      showHint("seen on " + [...seenRoundTrip].join(" & "));
      break;
    case "kicked":
      // the desktop removed this phone from the session
      editor.readOnly = true;
      showHint("removed by desktop", true);
      break;
    case "reset":
      // server rotated the session and told us to navigate to the new URL
      location.href = msg.url;
      break;
    case "error":
      setStatus("error", msg.reason || "rejected");
      // phones hide the status label, so the reason goes in a toast
      toast(msg.reason || "session ended");
      if (isDesktop && captionEl) captionEl.textContent = msg.reason || "session ended";
      if (!isDesktop) showHint(msg.reason || "session ended", true);
      editor.readOnly = true;
      showReset();
      break;
  }
});

ws.addEventListener("close", () => {
  editor.readOnly = true;
  showReset();
  if (statusEl.dataset.state !== "error") setStatus("error", "error");
});

ws.addEventListener("error", () => {
  editor.readOnly = true;
  showReset();
  if (statusEl.dataset.state !== "error") setStatus("error", "error");
});

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

// ponytail: keep the editor on screen when the mobile keyboard opens.
if (!isDesktop) {
  editor.addEventListener("focus", () => {
    requestAnimationFrame(() => {
      editor.scrollTop = editor.scrollHeight;
    });
  });
}