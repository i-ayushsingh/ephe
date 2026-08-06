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
const resetPanel = document.getElementById("reset-panel");
const resetBtn = document.getElementById("reset-btn");

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
  if (isDesktop && resetPanel) resetPanel.hidden = false;
}

if (resetBtn) {
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