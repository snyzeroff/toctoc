const { invoke, listen, icon, toast } = Toc;
const $ = (id) => document.getElementById(id);

const SOUNDS = [
  ["goutte", "Goutte"],
  ["carillon", "Carillon"],
  ["bulle", "Bulle"],
  ["cloche", "Cloche"],
  ["toctoc", "Toc toc"],
  ["none", "Aucun son"],
];

let state = null;
/** choice | host | join */
let pairMode = "choice";
let joining = false;
let conv = null;
let composer = null;

$("btn-settings").innerHTML = icon("gear");
document.querySelectorAll("#shortcuts .x").forEach((b) => (b.innerHTML = icon("close")));

// ───────── raccourcis : affichage et enregistrement ─────────
function accelLabel(accel) {
  if (!accel) return "Aucun";
  const mac = state?.mac;
  const names = {
    CmdOrCtrl: mac ? "⌘" : "Ctrl", Cmd: "⌘", Super: mac ? "⌘" : "Win", Ctrl: mac ? "⌃" : "Ctrl",
    Alt: mac ? "⌥" : "Alt", Shift: mac ? "⇧" : "Maj", Space: "Espace", Enter: "Entrée",
    ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Backquote: "²", Comma: ",",
    Period: ".", Slash: "/", Semicolon: ";", Quote: "'", Minus: "-", Equal: "=",
  };
  return accel.split("+").map((p) => names[p] ?? p).join(mac ? " " : " + ");
}

const SPECIAL = ["Space", "Enter", "Tab", "Backquote", "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash",
  "Semicolon", "Quote", "Comma", "Period", "Slash", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
  "Home", "End", "PageUp", "PageDown", "Insert", "Delete"];

/** Transforme un appui de touches en raccourci ; null tant qu'il n'y a que des modificateurs. */
function accelFromEvent(e) {
  if (["Control", "Shift", "Alt", "Meta", "AltGraph"].includes(e.key)) return null;
  const mods = [];
  if (state.mac) {
    if (e.metaKey) mods.push("Cmd");
    if (e.ctrlKey) mods.push("Ctrl");
  } else {
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.metaKey) mods.push("Super");
  }
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  let key = null;
  // Windows : la lettre telle qu'elle est imprimée sur la touche (AZERTY compris)
  if (!state.mac && /^[a-z]$/i.test(e.key)) key = e.key.toUpperCase();
  else if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
  else if (/^F\d{1,2}$/.test(e.code)) key = e.code;
  else if (SPECIAL.includes(e.code)) key = e.code;
  if (!key) return { error: "Cette touche ne peut pas servir de raccourci." };
  if (!mods.length && !/^F\d/.test(key)) {
    return { error: state.mac ? "Ajoutez ⌘, ⌥, ⌃ ou ⇧." : "Ajoutez Ctrl, Alt ou Maj." };
  }
  return { accel: mods.concat(key).join("+") };
}

let recording = null;
function stopRecording() {
  if (!recording) return;
  recording.btn.classList.remove("recording");
  window.removeEventListener("keydown", recording.handler, true);
  recording = null;
  render();
}
function startRecording(which, btn) {
  stopRecording();
  btn.classList.add("recording");
  btn.textContent = "Appuyez sur les touches…";
  const handler = async (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") return stopRecording();
    const r = accelFromEvent(e);
    if (!r) return;
    if (r.error) return toast(r.error);
    stopRecording();
    try {
      state = await invoke("set_shortcut", { which, accel: r.accel });
      render();
      toast("Raccourci enregistré : " + accelLabel(r.accel));
    } catch (err) {
      toast(String(err));
    }
  };
  recording = { btn, handler };
  window.addEventListener("keydown", handler, true);
}
document.querySelectorAll("#shortcuts .row").forEach((row) => {
  const which = row.dataset.sc;
  row.querySelector(".keycap").onclick = (e) => startRecording(which, e.currentTarget);
  row.querySelector(".x").onclick = async () => {
    stopRecording();
    state = await invoke("set_shortcut", { which, accel: "" });
    render();
  };
});

// ───────── affichage ─────────
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
const fileName = (path) => (path || "").split(/[\\/]/).pop();
function markChoice(container, attr, value) {
  for (const b of container.querySelectorAll("button")) b.classList.toggle("on", b.dataset[attr] === String(value));
}

function render() {
  const s = state.settings;
  $("view-pair").hidden = state.paired;
  $("view-chat").hidden = !state.paired;
  $("btn-settings").hidden = !state.paired;
  if (!state.paired) $("sheet").hidden = true;
  $("title").textContent = state.paired ? state.peer_name || "Conversation" : "Toc Toc";

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

  if (document.activeElement !== $("name")) $("name").value = s.name;
  const sounds = SOUNDS.slice();
  if (s.custom_sound) sounds.splice(5, 0, ["custom", "Fichier : " + fileName(s.custom_sound)]);
  setOptions($("sound"), sounds, s.sound);
  if (document.activeElement !== $("volume")) $("volume").value = Math.round(s.volume * 100);
  markChoice($("position"), "pos", s.position);
  markChoice($("size"), "v", s.size);
  markChoice($("duration"), "v", Math.round(s.duration));
  $("dnd").checked = s.dnd;
  $("autostart").checked = s.autostart;
  const accels = { ping: s.shortcut, quick: s.shortcut_quick, main: s.shortcut_main };
  const okIdx = { ping: 0, quick: 1, main: 2 };
  document.querySelectorAll("#shortcuts .row").forEach((row) => {
    const which = row.dataset.sc;
    const cap = row.querySelector(".keycap");
    if (recording?.btn === cap) return;
    cap.textContent = accelLabel(accels[which]);
    cap.title = state.shortcut_ok[okIdx[which]] ? "" : "Indisponible : déjà pris par une autre application";
    cap.style.color = state.shortcut_ok[okIdx[which]] ? "" : "var(--danger)";
    row.querySelector(".x").hidden = !accels[which];
  });
  $("version").textContent = "Toc Toc " + state.version;
}

async function refresh() {
  const wasPaired = state?.paired;
  state = await invoke("get_state");
  render();
  if (state.paired && !wasPaired) startChat();
}

async function update(patch) {
  state.settings = { ...state.settings, ...patch };
  render();
  state = await invoke("set_settings", { settings: state.settings });
  render();
}

// ───────── conversation ─────────
async function sendDrawing(background) {
  const blob = await Toc.draw({ background });
  if (!blob) return composer.focus();
  await composer.attach(blob, true);
  await composer.send();
}
async function annotate(m) {
  try {
    const b64 = await invoke("read_media", { name: m.image });
    await sendDrawing(`data:${m.image.endsWith(".png") ? "image/png" : "image/jpeg"};base64,${b64}`);
  } catch (err) {
    toast(String(err));
  }
}
function startChat() {
  if (!conv) {
    conv = new Toc.Conversation($("conv"), { pageSize: 80, onImage: (m) => Toc.lightbox(m, { onAnnotate: annotate }) });
    composer = new Toc.Composer($("composer"), { toolbar: true, onDraw: () => sendDrawing(null) });
  }
  conv.load();
  setTimeout(() => composer.focus(), 50);
}

// ───────── appairage ─────────
function pairError(text) {
  $("pair-error").textContent = text || "";
  $("pair-error").hidden = !text;
}
$("btn-create").onclick = async () => {
  pairError("");
  pairMode = "host";
  state.pair_code = await invoke("pair_create");
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

// ───────── réglages ─────────
function openSettings() {
  $("sheet").hidden = false;
}
function closeSettings() {
  stopRecording();
  $("sheet").hidden = true;
  composer?.focus();
}
$("btn-settings").onclick = openSettings;
$("btn-done").onclick = closeSettings;
$("name").onchange = (e) => update({ name: e.target.value });
$("name").onkeydown = (e) => e.key === "Enter" && e.target.blur();
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

function askClear() {
  $("clear-zone").innerHTML =
    '<button class="chip danger" id="clear-yes">Tout effacer</button><button class="chip" id="clear-no">Annuler</button>';
  $("clear-no").onclick = restoreClear;
  $("clear-yes").onclick = async () => {
    await invoke("clear_history");
    restoreClear();
    toast("Historique effacé sur cet ordinateur.");
  };
}
function restoreClear() {
  $("clear-zone").innerHTML = '<button id="btn-clear" class="chip">Effacer…</button>';
  $("btn-clear").onclick = askClear;
}
$("btn-clear").onclick = askClear;

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

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !$("sheet").hidden && !recording) closeSettings();
});

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
listen("intent", (e) => {
  if (!state?.paired) return;
  if (e.payload === "settings") openSettings();
  else if (e.payload === "draw") {
    closeSettings();
    sendDrawing(null);
  }
});

refresh();
