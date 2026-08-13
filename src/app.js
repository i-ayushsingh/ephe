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

function flash(btn, label) {
  const original = btn.textContent;
  btn.textContent = label;
  setTimeout(() => { btn.textContent = original; }, 1200);
}

function insertAtCursor(text) {
  const start = editor.selectionStart, end = editor.selectionEnd;
  editor.value = editor.value.slice(0, start) + text + editor.value.slice(end);
  const pos = start + text.length;
  editor.setSelectionRange(pos, pos);
  editor.dispatchEvent(new Event("input", { bubbles: true }));
}

async function copyText(btn) {
  const text = editor.value;
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // ponytail: execCommand fallback for non-secure contexts (http://lan-ip)
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
  }
  flash(btn, "copied!");
}

async function pasteText(btn) {
  if (editor.readOnly) return;
  let text = null;
  try {
    text = await navigator.clipboard.readText();
  } catch {
    text = window.prompt("paste text below:");
  }
  if (!text) return;
  insertAtCursor(text);
  flash(btn, "pasted!");
}

for (const btn of document.querySelectorAll(".top-copy, #copy-btn")) {
  btn.addEventListener("click", () => copyText(btn));
}
for (const btn of document.querySelectorAll(".top-paste, #paste-btn")) {
  btn.addEventListener("click", () => pasteText(btn));
}

// ---------- clear ----------

function clearText(btn) {
  if (editor.readOnly || !editor.value) return;
  editor.value = "";
  editor.setSelectionRange(0, 0);
  editor.dispatchEvent(new Event("input", { bubbles: true }));
  flash(btn, "cleared!");
}

for (const btn of document.querySelectorAll(".top-clear, #clear-btn")) {
  btn.addEventListener("click", () => clearText(btn));
}

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
    resetBtn.disabled = true;
    const original = resetBtn.textContent;
    resetBtn.textContent = "starting\u2026";
    try {
      const res = await fetch("/reset", { method: "POST" });
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
      applying = false;
      updateCounter();
      if (isDesktop) editor.focus({ preventScroll: true });
      break;
    case "text":
      if (msg.text === lastSent) break;
      applying = true;
      editor.value = msg.text || "";
      // ponytail: drop the caret at the end of the new text so the user sees
      // where the remote edit landed instead of the cursor freezing at a
      // stale character index.
      const len = editor.value.length;
      editor.setSelectionRange(len, len);
      lastSent = editor.value;
      applying = false;
      updateCounter();
      break;
    case "peer_joined":
      setStatus("connected", "connected");
      break;
    case "peer_left":
      setStatus("error", "error");
      editor.readOnly = true;
      showReset();
      break;
    case "reset":
      // server rotated the session and told us to navigate to the new URL
      location.href = msg.url;
      break;
    case "error":
      setStatus("error", msg.reason || "rejected");
      editor.readOnly = true;
      showReset();
      break;
  }
});

ws.addEventListener("close", () => {
  setStatus("error", "error");
  editor.readOnly = true;
  showReset();
});

ws.addEventListener("error", () => {
  setStatus("error", "error");
});

let sendTimer = null;
editor.addEventListener("input", () => {
  updateCounter();
  if (applying) return;
  if (editor.value === lastSent) return;
  lastSent = editor.value;
  if (ws.readyState !== 1) return;
  // ponytail: tiny debounce so a fast typist doesn't send a packet per keystroke
  clearTimeout(sendTimer);
  sendTimer = setTimeout(() => {
    if (ws.readyState === 1) {
      ws.send(JSON.stringify({ type: "text", text: editor.value }));
    }
  }, 16);
});

// ponytail: keep the editor on screen when the mobile keyboard opens.
if (!isDesktop) {
  editor.addEventListener("focus", () => {
    requestAnimationFrame(() => {
      editor.scrollTop = editor.scrollHeight;
    });
  });
}