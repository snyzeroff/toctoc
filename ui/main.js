const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

const SOUNDS = [
  ["carillon", "Carillon"],
  ["goutte", "Goutte"],
  ["toctoc", "Toc toc"],
  ["cloche", "Cloche"],
  ["bulle", "Bulle"],
  ["none", "Aucun son"],
];
const SHORTCUTS = ["CmdOrCtrl+Alt+P", "CmdOrCtrl+Alt+T", "CmdOrCtrl+Alt+Space", ""];

let state = null;
/** choice | host | join */
let pairMode = "choice";
let joining = false;

function shortcutLabel(s) {
  if (!s) return "Aucun";
  const key = s.split("+").pop().replace("Space", "Espace");
  return state.mac ? `⌘ ⌥ ${key}` : `Ctrl + Alt + ${key}`;
}

function fileName(path) {
  return (path || "").split(/[\\/]/).pop();
}

function setOptions(select, options, value) {
  const sig = JSON.stringify(options);
  if (select.dataset.sig !== sig) {
    select.innerHTML = "";
    for (const [v, label] of options) select.add(new Option(label, v));
    select.dataset.sig = sig;
  }
  select.value = value;
  if (select.selectedIndex < 0) select.selectedIndex = 0;
}

function markChoice(container, attr, value) {
  for (const b of container.querySelectorAll("button")) b.classList.toggle("on", b.dataset[attr] === String(value));
}

function render() {
  const s = state.settings;
  $("view-pair").hidden = state.paired;
  $("view-home").hidden = !state.paired;

  const pill = $("status");
  pill.hidden = !state.paired;
  pill.classList.toggle("on", state.online);
  pill.querySelector("span").textContent = state.online ? "Connecté" : "Hors ligne";

  if (!state.paired) {
    if (state.pair_code) pairMode = "host";
    $("pair-choice").hidden = pairMode !== "choice";
    $("pair-host").hidden = pairMode !== "host";
    $("pair-join").hidden = pairMode !== "join";
    if (state.pair_code) $("code").textContent = state.pair_code.slice(0, 4) + "-" + state.pair_code.slice(4);
    $("join-wait").hidden = !joining;
    $("btn-join").disabled = joining;
    return;
  }
  pairMode = "choice";
  joining = false;

  const sounds = SOUNDS.slice();
  if (s.custom_sound) sounds.splice(5, 0, ["custom", "Fichier : " + fileName(s.custom_sound)]);
  setOptions($("sound"), sounds, s.sound);
  if (document.activeElement !== $("volume")) $("volume").value = Math.round(s.volume * 100);
  markChoice($("position"), "pos", s.position);
  markChoice($("size"), "v", s.size);
  markChoice($("duration"), "v", Math.round(s.duration));
  $("dnd").checked = s.dnd;
  $("autostart").checked = s.autostart;
  setOptions($("shortcut"), SHORTCUTS.map((k) => [k, shortcutLabel(k)]), s.shortcut);
  $("shortcut-warn").hidden = state.shortcut_ok || !s.shortcut;
  const where = state.mac ? "la barre de menus" : "la barre des tâches";
  $("ping-hint").textContent = `Encore plus rapide : un clic sur l'icône Toc Toc dans ${where}` +
    (s.shortcut && state.shortcut_ok ? `, ou ${shortcutLabel(s.shortcut)}.` : ".");
  $("version").textContent = "Toc Toc " + state.version;
}

async function refresh() {
  state = await invoke("get_state");
  render();
}

async function update(patch) {
  state.settings = { ...state.settings, ...patch };
  render();
  state = await invoke("set_settings", { settings: state.settings });
  render();
}

let toastTimer;
function toast(text) {
  const t = $("toast");
  t.textContent = text;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 3200);
}

function pairError(text) {
  $("pair-error").textContent = text || "";
  $("pair-error").hidden = !text;
}

// ───────── appairage ─────────
$("btn-create").onclick = async () => {
  pairError("");
  pairMode = "host";
  const code = await invoke("pair_create");
  state.pair_code = code;
  render();
};
$("btn-have").onclick = () => {
  pairError("");
  pairMode = "join";
  render();
  $("code-input").focus();
};
$("btn-cancel-host").onclick = $("btn-cancel-join").onclick = async () => {
  await invoke("pair_cancel");
  pairMode = "choice";
  joining = false;
  pairError("");
  await refresh();
};
$("code-input").oninput = (e) => {
  const raw = e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 8);
  e.target.value = raw.length > 4 ? raw.slice(0, 4) + "-" + raw.slice(4) : raw;
};
$("code-input").onkeydown = (e) => {
  if (e.key === "Enter") $("btn-join").click();
};
$("btn-join").onclick = async () => {
  pairError("");
  try {
    await invoke("pair_join", { code: $("code-input").value });
    joining = true;
    render();
  } catch (err) {
    pairError(String(err));
  }
};

// ───────── ping ─────────
async function sendPing() {
  const input = $("ping-text");
  const btn = $("btn-ping");
  btn.disabled = true;
  try {
    await invoke("ping", { text: input.value });
    input.value = "";
  } catch (err) {
    toast(String(err));
  } finally {
    btn.disabled = false;
  }
}
$("btn-ping").onclick = sendPing;
$("ping-text").onkeydown = (e) => {
  if (e.key === "Enter") sendPing();
};

// ───────── réglages ─────────
$("sound").onchange = async (e) => {
  await update({ sound: e.target.value });
  invoke("preview", { soundOnly: true, silent: false });
};
$("btn-listen").onclick = () => invoke("preview", { soundOnly: true, silent: false });
$("btn-pick").onclick = async () => {
  state = await invoke("pick_sound");
  render();
};
$("volume").onchange = async (e) => {
  await update({ volume: Number(e.target.value) / 100 });
  invoke("preview", { soundOnly: true, silent: false });
};
$("position").onclick = async (e) => {
  const pos = e.target.dataset.pos;
  if (!pos) return;
  await update({ position: pos });
  invoke("preview", { soundOnly: false, silent: true });
};
$("size").onclick = async (e) => {
  const v = e.target.dataset.v;
  if (!v) return;
  await update({ size: v });
  invoke("preview", { soundOnly: false, silent: true });
};
$("duration").onclick = (e) => {
  const v = e.target.dataset.v;
  if (v) update({ duration: Number(v) });
};
$("btn-preview").onclick = () => invoke("preview", { soundOnly: false, silent: false });
$("dnd").onchange = (e) => update({ dnd: e.target.checked });
$("autostart").onchange = (e) => update({ autostart: e.target.checked });
$("shortcut").onchange = (e) => update({ shortcut: e.target.value });

$("btn-unpair").onclick = () => {
  $("unpair-confirm").hidden = false;
  $("btn-unpair").hidden = true;
};
$("btn-unpair-no").onclick = () => {
  $("unpair-confirm").hidden = true;
  $("btn-unpair").hidden = false;
};
$("btn-unpair-yes").onclick = async () => {
  await invoke("unpair");
  $("btn-unpair-no").click();
  await refresh();
};

// ───────── événements venus du programme ─────────
listen("state", refresh);
listen("pair-result", async (e) => {
  joining = false;
  if (e.payload.ok) {
    pairError("");
    $("code-input").value = "";
  } else {
    pairMode = "choice";
    pairError(e.payload.error);
  }
  await refresh();
});
listen("focus-text", () => setTimeout(() => $("ping-text").focus(), 60));

refresh();
