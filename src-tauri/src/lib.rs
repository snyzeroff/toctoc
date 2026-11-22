mod config;
mod crypto;
mod history;
mod notif;
mod pairing;
mod relay;
mod sound;

use config::{Config, Settings};
use history::{History, Msg};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_clipboard_manager::ClipboardExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt as _, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt as _;

type Task = tauri::async_runtime::JoinHandle<()>;

/// Taille maximale d'une image (après compression dans l'interface).
const MAX_IMAGE: usize = 8 * 1024 * 1024;
/// Au-delà, le message chiffré part en pièce jointe sur le relais.
const INLINE_LIMIT: usize = 2900;
const SHORTCUT_NAMES: [&str; 3] = ["ping", "quick", "main"];

pub struct AppState {
    pub dir: PathBuf,
    pub cfg: Mutex<Config>,
    pub http: reqwest::Client,
    pub online: AtomicBool,
    pub notif: Mutex<Option<notif::Notice>>,
    pub notif_seq: AtomicU64,
    history: Mutex<History>,
    listen_task: Mutex<Option<Task>>,
    pair_task: Mutex<Option<Task>>,
    pair_code: Mutex<Option<String>>,
    seen: Mutex<VecDeque<String>>,
    last_ping: Mutex<Option<String>>,
    last_ping_at: Mutex<Option<Instant>>,
    dnd_item: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    /// raccourcis enregistrés : (identifiant, action)
    shortcuts: Mutex<Vec<(u32, &'static str)>>,
    shortcut_ok: Mutex<[bool; 3]>,
    /// mise à jour téléchargée, prête à être installée
    pending_update: Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>,
    update_busy: AtomicBool,
}

impl AppState {
    pub fn log(&self, line: &str) {
        config::log(&self.dir, line);
    }
    fn save(&self) {
        let cfg = self.cfg.lock().unwrap().clone();
        config::save(&self.dir, &cfg);
    }
    /// Vrai si l'identifiant n'avait pas encore été vu.
    fn first_time(&self, id: &str) -> bool {
        let mut seen = self.seen.lock().unwrap();
        if seen.iter().any(|s| s == id) {
            return false;
        }
        seen.push_back(id.to_string());
        if seen.len() > 500 {
            seen.pop_front();
        }
        true
    }
}

/// Ce qui voyage (chiffré) entre les deux ordinateurs.
#[derive(Serialize, Deserialize, Default)]
struct Wire {
    /// ping | msg | ack | bye
    t: String,
    id: String,
    from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// image en base64
    #[serde(default, skip_serializing_if = "Option::is_none")]
    img: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mime: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    drawing: bool,
    /// prénom de l'expéditeur
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dnd: Option<bool>,
}

// ───────────────────────── état et fenêtres ─────────────────────────

fn emit_state(app: &AppHandle) {
    let st = app.state::<AppState>();
    let paired = st.cfg.lock().unwrap().pairing.is_some();
    let tip = if !paired {
        "Toc Toc — pas encore associé"
    } else if st.online.load(Ordering::SeqCst) {
        "Toc Toc — connecté"
    } else {
        "Toc Toc — hors ligne"
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(tip));
    }
    let _ = app.emit("state", ());
}

fn emit_msg(app: &AppHandle, op: &str, msg: &Msg) {
    let _ = app.emit("history", json!({ "op": op, "msg": msg }));
}

fn window_focused(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
        .unwrap_or(false)
}

/// Ouvre la fenêtre principale. `intent` : « draw », « settings » ou rien.
fn show_main(app: &AppHandle, intent: Option<&str>) {
    hide_quick(app);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(i) = intent {
            let _ = app.emit_to("main", "intent", i);
        }
    }
}

fn show_quick(app: &AppHandle) {
    if app.state::<AppState>().cfg.lock().unwrap().pairing.is_none() {
        show_main(app, None);
        return;
    }
    let Some(w) = app.get_webview_window("quick") else { return };
    if let (Ok(Some(mon)), Ok(size)) = (w.primary_monitor(), w.outer_size()) {
        let area = mon.work_area();
        let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
        let y = area.position.y + (area.size.height as f64 * 0.16) as i32;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to("quick", "quick-open", ());
}

fn hide_quick(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.hide();
    }
}

fn toggle_quick(app: &AppHandle) {
    if window_focused(app, "quick") {
        hide_quick(app);
    } else {
        show_quick(app);
    }
}

fn play_sound(app: &AppHandle) {
    let st = app.state::<AppState>();
    let settings = st.cfg.lock().unwrap().settings.clone();
    let dir = st.dir.clone();
    sound::play(&settings, move |line| config::log(&dir, line));
}

fn peer_label(app: &AppHandle) -> String {
    let name = app.state::<AppState>().cfg.lock().unwrap().peer_name.clone();
    if name.is_empty() {
        "Nouveau message".into()
    } else {
        name
    }
}

/// Aperçu en texte simple d'un message en markdown.
fn preview_text(md: &str) -> String {
    let mut out = String::new();
    let mut chars = md.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '_' | '~' | '`' | '#' | '>' => {}
            '[' => {
                // [texte](lien) → texte
                let mut label = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    label.push(d);
                }
                if chars.peek() == Some(&'(') {
                    for d in chars.by_ref() {
                        if d == ')' {
                            break;
                        }
                    }
                }
                out.push_str(&label);
            }
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().chars().take(140).collect()
}

// ───────────────────────── écoute du relais ─────────────────────────

fn start_listening(app: &AppHandle) {
    let st = app.state::<AppState>();
    if let Some(h) = st.listen_task.lock().unwrap().take() {
        h.abort();
    }
    st.online.store(false, Ordering::SeqCst);
    let (pairing, relay_url) = {
        let c = st.cfg.lock().unwrap();
        (c.pairing.clone(), c.relay.clone())
    };
    let Some(p) = pairing else {
        emit_state(app);
        return;
    };
    let Some(key) = crypto::key_from_b64(&p.key) else {
        st.log("écoute: clé illisible");
        return;
    };
    let http = st.http.clone();
    let app2 = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let mut last_alive: Option<SystemTime> = None;
        let mut backoff = 2u64;
        loop {
            // après une coupure, on rattrape au plus les 10 dernières minutes
            let since = last_alive.map(|t| {
                let gap = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
                format!("{}s", (gap + 5).min(600))
            });
            let mut opened = false;
            let res = relay::listen(&http, &relay_url, &p.topic, since, |ev| {
                last_alive = Some(SystemTime::now());
                match ev {
                    relay::Event::Open => {
                        opened = true;
                        set_online(&app2, true);
                    }
                    relay::Event::Alive => {}
                    relay::Event::Message { text, attachment: None } => {
                        if let Some(plain) = crypto::decrypt(&key, &text) {
                            handle_plain(&app2, &key, &plain);
                        }
                    }
                    relay::Event::Message { attachment: Some(url), .. } => {
                        let (app3, http, relay_url) = (app2.clone(), http.clone(), relay_url.clone());
                        tauri::async_runtime::spawn(async move {
                            match relay::download(&http, &relay_url, &url, MAX_IMAGE * 2).await {
                                Ok(raw) => {
                                    if let Some(plain) = crypto::decrypt_raw(&key, &raw) {
                                        handle_plain(&app3, &key, &plain);
                                    }
                                }
                                Err(e) => app3.state::<AppState>().log(&format!("pièce jointe: échec ({e})")),
                            }
                        });
                    }
                }
            })
            .await;
            set_online(&app2, false);
            if let Err(e) = res {
                app2.state::<AppState>().log(&format!("écoute: coupée ({e})"));
            }
            backoff = if opened { 2 } else { (backoff * 2).min(30) };
            tokio::time::sleep(Duration::from_secs(backoff)).await;
        }
    });
    *st.listen_task.lock().unwrap() = Some(handle);
    emit_state(app);
}

fn set_online(app: &AppHandle, online: bool) {
    let st = app.state::<AppState>();
    if st.online.swap(online, Ordering::SeqCst) != online {
        st.log(if online { "écoute: connecté au relais" } else { "écoute: hors ligne" });
        emit_state(app);
    }
}

fn handle_plain(app: &AppHandle, key: &[u8; 32], plain: &[u8]) {
    let st = app.state::<AppState>();
    let Ok(msg) = serde_json::from_slice::<Wire>(plain) else { return };
    let (dev, dnd, duration) = {
        let c = st.cfg.lock().unwrap();
        (c.device_id.clone(), c.settings.dnd, c.settings.duration)
    };
    if msg.from == dev || !st.first_time(&msg.id) {
        return;
    }
    if let Some(name) = &msg.name {
        let name: String = name.trim().chars().filter(|c| !c.is_control()).take(30).collect();
        let mut c = st.cfg.lock().unwrap();
        if c.peer_name != name {
            c.peer_name = name;
            drop(c);
            st.save();
            emit_state(app);
        }
    }
    match msg.t.as_str() {
        "ping" | "msg" => {
            let text: Option<String> = msg
                .text
                .as_deref()
                .map(|t| t.chars().take(4000).collect::<String>())
                .filter(|t| !t.trim().is_empty());
            // image : on la range dans media/
            let mut image = None;
            if let (Some(b64), Some(ext)) = (&msg.img, msg.mime.as_deref().and_then(history::ext_for_mime)) {
                if let Some(bytes) = crypto::unb64(b64).filter(|b| b.len() <= MAX_IMAGE) {
                    let name = format!("{}.{ext}", crypto::random_hex(8));
                    let dir = history::media_dir(&st.dir);
                    let _ = std::fs::create_dir_all(&dir);
                    if std::fs::write(dir.join(&name), bytes).is_ok() {
                        image = Some(name);
                    }
                }
            }
            if msg.t == "msg" && text.is_none() && image.is_none() {
                return;
            }
            let entry = Msg {
                id: crypto::random_hex(8),
                out: false,
                ts: history::now_ms(),
                kind: msg.t.clone(),
                text: text.clone(),
                image: image.clone(),
                drawing: msg.drawing,
                status: "received".into(),
            };
            st.history.lock().unwrap().add(entry.clone());
            emit_msg(app, "add", &entry);
            if dnd {
                st.log(&format!("{}: reçu, sans alerte (ne pas déranger)", msg.t));
            } else {
                st.log(&format!("{}: reçu", msg.t));
                // conversation déjà sous les yeux : juste le son
                let watching = window_focused(app, "main") || window_focused(app, "quick");
                if !watching {
                    let ms = (duration * 1000.0) as u64;
                    if msg.t == "ping" {
                        let title = app.state::<AppState>().cfg.lock().unwrap().peer_name.clone();
                        let title = if title.is_empty() { "Toc toc !".to_string() } else { format!("{title} : toc toc !") };
                        notif::show_with(app, "ping", &title, &preview_text(text.as_deref().unwrap_or("")), ms, None);
                    } else {
                        let body = match (&text, &image, msg.drawing) {
                            (Some(t), _, _) => preview_text(t),
                            (None, Some(_), true) => "✏️ Dessin".into(),
                            _ => "📷 Image".into(),
                        };
                        notif::show_with(app, "msg", &peer_label(app), &body, ms, image.clone());
                    }
                }
                play_sound(app);
            }
            let ack = Wire {
                t: "ack".into(),
                id: crypto::random_hex(8),
                from: dev,
                reference: Some(msg.id),
                dnd: dnd.then_some(true),
                ..Default::default()
            };
            publish_small(app, key, ack);
        }
        "ack" => {
            let Some(reference) = msg.reference else { return };
            if let Some(m) = st.history.lock().unwrap().set_status(&reference, "delivered") {
                emit_msg(app, "update", &m);
            }
            // pastille « Reçu » seulement pour un ping
            let was_ping = st.last_ping.lock().unwrap().as_deref() == Some(reference.as_str());
            if was_ping {
                *st.last_ping.lock().unwrap() = None;
                if msg.dnd == Some(true) {
                    notif::show(app, "ack", "Reçu (ne pas déranger)", "", 2200);
                } else {
                    notif::show(app, "ack", "Reçu", "", 1500);
                }
            }
        }
        "bye" => {
            st.log("appairage: l'autre ordinateur s'est dissocié");
            forget_pairing(app);
            show_main(app, None);
        }
        _ => {}
    }
}

/// Envoie un petit message de service (accusé, au revoir) sans attendre.
fn publish_small(app: &AppHandle, key: &[u8; 32], wire: Wire) {
    let st = app.state::<AppState>();
    let (relay_url, topic) = {
        let c = st.cfg.lock().unwrap();
        (c.relay.clone(), c.pairing.as_ref().map(|p| p.topic.clone()))
    };
    let Some(topic) = topic else { return };
    st.first_time(&wire.id);
    let body = crypto::encrypt(key, &serde_json::to_vec(&wire).unwrap());
    let http = st.http.clone();
    tauri::async_runtime::spawn(async move {
        let _ = relay::publish(&http, &relay_url, &topic, body).await;
    });
}

/// Chiffre et publie ; les gros messages partent en pièce jointe.
async fn transmit(app: &AppHandle, wire: &Wire) -> Result<(), String> {
    let st = app.state::<AppState>();
    let (pairing, relay_url) = {
        let c = st.cfg.lock().unwrap();
        (c.pairing.clone(), c.relay.clone())
    };
    let p = pairing.ok_or("Cet ordinateur n'est pas encore associé.")?;
    let key = crypto::key_from_b64(&p.key).ok_or("Clé illisible.")?;
    st.first_time(&wire.id);
    let raw = crypto::encrypt_raw(&key, &serde_json::to_vec(wire).unwrap());
    if raw.len() <= INLINE_LIMIT {
        relay::publish(&st.http, &relay_url, &p.topic, crypto::b64(&raw)).await
    } else {
        relay::publish_bytes(&st.http, &relay_url, &p.topic, raw).await
    }
}

fn forget_pairing(app: &AppHandle) {
    let st = app.state::<AppState>();
    {
        let mut c = st.cfg.lock().unwrap();
        c.pairing = None;
        c.peer_name.clear();
    }
    st.save();
    start_listening(app);
}

fn my_name(app: &AppHandle) -> Option<String> {
    let n = app.state::<AppState>().cfg.lock().unwrap().settings.name.clone();
    (!n.is_empty()).then_some(n)
}

// ───────────────────────── envoyer ─────────────────────────

/// Envoie (ou renvoie) un message de l'historique.
async fn deliver(app: AppHandle, id: String) {
    let st = app.state::<AppState>();
    let Some(m) = st.history.lock().unwrap().get(&id) else { return };
    let dev = st.cfg.lock().unwrap().device_id.clone();
    let mut wire = Wire {
        t: m.kind.clone(),
        id: m.id.clone(),
        from: dev,
        text: m.text.clone(),
        drawing: m.drawing,
        name: my_name(&app),
        ..Default::default()
    };
    if let Some(img) = &m.image {
        match std::fs::read(history::media_dir(&st.dir).join(img)) {
            Ok(bytes) => {
                wire.img = Some(crypto::b64(&bytes));
                wire.mime = Some(history::mime_for_name(img).into());
            }
            Err(_) => {
                st.log("envoi: image introuvable");
            }
        }
    }
    let res = transmit(&app, &wire).await;
    let status = if res.is_ok() { "sent" } else { "failed" };
    match &res {
        Ok(()) => st.log(&format!("{}: envoyé", m.kind)),
        Err(e) => st.log(&format!("{}: échec de l'envoi ({e})", m.kind)),
    }
    if let Some(m2) = st.history.lock().unwrap().set_status(&id, status) {
        emit_msg(&app, "update", &m2);
    }
    if m.kind == "ping" {
        // le ping part souvent sans fenêtre ouverte : une petite pastille confirme
        if res.is_ok() {
            *st.last_ping.lock().unwrap() = Some(id);
            notif::show(&app, "sent", "Ping envoyé", "", 2000);
        } else {
            notif::show(&app, "error", "Ping non envoyé", "Vérifiez la connexion Internet", 3500);
        }
    }
}

fn add_outgoing(app: &AppHandle, kind: &str, text: Option<String>, image: Option<String>, drawing: bool) -> Msg {
    let st = app.state::<AppState>();
    let m = Msg {
        id: crypto::random_hex(8),
        out: true,
        ts: history::now_ms(),
        kind: kind.into(),
        text,
        image,
        drawing,
        status: "sending".into(),
    };
    st.history.lock().unwrap().add(m.clone());
    emit_msg(app, "add", &m);
    m
}

fn ping_now(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.cfg.lock().unwrap().pairing.is_none() {
        show_main(app, None);
        return;
    }
    {
        // évite les rafales (touche du raccourci maintenue)
        let mut last = st.last_ping_at.lock().unwrap();
        if last.map(|t| t.elapsed() < Duration::from_millis(700)).unwrap_or(false) {
            return;
        }
        *last = Some(Instant::now());
    }
    let m = add_outgoing(app, "ping", None, None, false);
    tauri::async_runtime::spawn(deliver(app.clone(), m.id));
}

// ───────────────────────── appairage ─────────────────────────

fn start_pairing(app: &AppHandle, code: String, host: bool) {
    let st = app.state::<AppState>();
    if let Some(h) = st.pair_task.lock().unwrap().take() {
        h.abort();
    }
    *st.pair_code.lock().unwrap() = host.then(|| code.clone());
    st.log(if host { "appairage: code créé, en attente" } else { "appairage: code saisi" });
    let app2 = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let res = pairing::run(app2.clone(), code, host).await;
        let st = app2.state::<AppState>();
        *st.pair_code.lock().unwrap() = None;
        match res {
            Ok(p) => {
                st.cfg.lock().unwrap().pairing = Some(p);
                st.save();
                st.log("appairage: réussi");
                start_listening(&app2);
                let _ = app2.emit("pair-result", json!({ "ok": true }));
                notif::show(&app2, "info", "Ordinateurs associés", "Envoyez votre premier message !", 4000);
            }
            Err(e) => {
                st.log(&format!("appairage: échec ({e})"));
                let _ = app2.emit("pair-result", json!({ "ok": false, "error": e }));
                emit_state(&app2);
            }
        }
    });
    *st.pair_task.lock().unwrap() = Some(handle);
}

// ───────────────────────── réglages ─────────────────────────

/// (Ré)enregistre les trois raccourcis globaux.
fn apply_shortcuts(app: &AppHandle) -> [bool; 3] {
    let st = app.state::<AppState>();
    let s = st.cfg.lock().unwrap().settings.clone();
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map: Vec<(u32, &'static str)> = Vec::new();
    let mut ok = [true; 3];
    for (i, accel) in [&s.shortcut, &s.shortcut_quick, &s.shortcut_main].into_iter().enumerate() {
        if accel.is_empty() {
            continue;
        }
        match accel.parse::<Shortcut>() {
            Ok(sc) if !map.iter().any(|(id, _)| *id == sc.id()) => {
                if gs.register(sc).is_ok() {
                    map.push((sc.id(), SHORTCUT_NAMES[i]));
                } else {
                    ok[i] = false;
                }
            }
            _ => ok[i] = false,
        }
    }
    *st.shortcuts.lock().unwrap() = map;
    *st.shortcut_ok.lock().unwrap() = ok;
    ok
}

/// Windows : met le chemin du programme entre guillemets dans la clé de démarrage
/// (le dossier d'installation contient une espace).
#[cfg(windows)]
fn quote_autostart_path(name: &str) {
    use winreg::{enums::*, RegKey};
    let run = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ | KEY_WRITE,
    );
    if let (Ok(run), Ok(exe)) = (run, std::env::current_exe()) {
        let exe = exe.to_string_lossy().to_string();
        if let Ok(current) = run.get_value::<String, _>(name) {
            if current.starts_with(&exe) {
                let _ = run.set_value(name, &format!("\"{}\"{}", exe, &current[exe.len()..]));
            }
        }
    }
}

fn apply_settings(app: &AppHandle, mut new: Settings) {
    let st = app.state::<AppState>();
    let old = st.cfg.lock().unwrap().settings.clone();
    // les raccourcis se changent uniquement par set_shortcut
    new.shortcut = old.shortcut.clone();
    new.shortcut_quick = old.shortcut_quick.clone();
    new.shortcut_main = old.shortcut_main.clone();
    new.sanitize();
    st.cfg.lock().unwrap().settings = new.clone();
    st.save();
    if old.autostart != new.autostart {
        let al = app.autolaunch();
        let res = if new.autostart { al.enable() } else { al.disable() };
        #[cfg(windows)]
        if new.autostart {
            quote_autostart_path(&app.package_info().name);
        }
        st.log(&format!(
            "réglages: lancement au démarrage = {} ({})",
            new.autostart,
            if res.is_ok() { "ok" } else { "erreur" }
        ));
    }
    if old.dnd != new.dnd {
        if let Some(item) = st.dnd_item.lock().unwrap().as_ref() {
            let _ = item.set_checked(new.dnd);
        }
        st.log(&format!("réglages: ne pas déranger = {}", new.dnd));
    }
    if old.name != new.name {
        st.log("réglages: prénom modifié");
    }
}

// ───────────────────────── commandes appelées par l'interface ─────────────────────────

#[tauri::command]
fn get_state(app: AppHandle) -> serde_json::Value {
    let st = app.state::<AppState>();
    let c = st.cfg.lock().unwrap();
    json!({
        "paired": c.pairing.is_some(),
        "online": st.online.load(Ordering::SeqCst),
        "settings": c.settings,
        "peer_name": c.peer_name,
        "pair_code": *st.pair_code.lock().unwrap(),
        "shortcut_ok": *st.shortcut_ok.lock().unwrap(),
        "mac": cfg!(target_os = "macos"),
        "update": st.pending_update.lock().unwrap().as_ref().map(|(u, _)| u.version.clone()),
        "version": env!("CARGO_PKG_VERSION"),
    })
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: Settings) -> serde_json::Value {
    apply_settings(&app, settings);
    emit_state(&app);
    get_state(app)
}

fn shortcut_field(s: &mut Settings, idx: usize) -> &mut String {
    match idx {
        0 => &mut s.shortcut,
        1 => &mut s.shortcut_quick,
        _ => &mut s.shortcut_main,
    }
}

#[tauri::command]
fn set_shortcut(app: AppHandle, which: String, accel: String) -> Result<serde_json::Value, String> {
    let idx = SHORTCUT_NAMES.iter().position(|n| *n == which).ok_or("Raccourci inconnu.")?;
    let accel = accel.trim().to_string();
    if !accel.is_empty() && accel.parse::<Shortcut>().is_err() {
        return Err("Raccourci non reconnu.".into());
    }
    let st = app.state::<AppState>();
    let old = {
        let mut c = st.cfg.lock().unwrap();
        std::mem::replace(shortcut_field(&mut c.settings, idx), accel.clone())
    };
    let ok = apply_shortcuts(&app);
    if !accel.is_empty() && !ok[idx] {
        *shortcut_field(&mut st.cfg.lock().unwrap().settings, idx) = old;
        apply_shortcuts(&app);
        return Err("Ce raccourci est déjà pris (par une autre application ou un autre raccourci).".into());
    }
    st.save();
    st.log(&format!("réglages: raccourci {which} = « {accel} »"));
    emit_state(&app);
    Ok(get_state(app))
}

#[tauri::command]
fn pair_create(app: AppHandle) -> String {
    let code = crypto::new_code();
    start_pairing(&app, code.clone(), true);
    code
}

#[tauri::command]
fn pair_join(app: AppHandle, code: String) -> Result<(), String> {
    let code = crypto::normalize_code(&code).ok_or("Le code fait 8 caractères (lettres et chiffres).")?;
    start_pairing(&app, code, false);
    Ok(())
}

#[tauri::command]
fn pair_cancel(app: AppHandle) {
    let st = app.state::<AppState>();
    if let Some(h) = st.pair_task.lock().unwrap().take() {
        h.abort();
    }
    *st.pair_code.lock().unwrap() = None;
    st.log("appairage: annulé");
}

#[tauri::command]
fn unpair(app: AppHandle) {
    let st = app.state::<AppState>();
    let (key, dev) = {
        let c = st.cfg.lock().unwrap();
        (c.pairing.as_ref().and_then(|p| crypto::key_from_b64(&p.key)), c.device_id.clone())
    };
    if let Some(key) = key {
        // prévient l'autre ordinateur, puis oublie le canal
        let bye = Wire { t: "bye".into(), id: crypto::random_hex(8), from: dev, ..Default::default() };
        publish_small(&app, &key, bye);
    }
    st.log("appairage: dissocié");
    forget_pairing(&app);
}

#[tauri::command]
fn ping(app: AppHandle) {
    ping_now(&app);
}

#[tauri::command]
fn send_message(
    app: AppHandle,
    text: String,
    image: Option<String>,
    mime: Option<String>,
    drawing: bool,
) -> Result<Msg, String> {
    let st = app.state::<AppState>();
    if st.cfg.lock().unwrap().pairing.is_none() {
        return Err("Cet ordinateur n'est pas encore associé.".into());
    }
    let text = text.trim().to_string();
    if text.chars().count() > 4000 {
        return Err("Message trop long (4000 caractères au plus).".into());
    }
    let mut image_name = None;
    if let Some(b64) = image {
        let ext = mime.as_deref().and_then(history::ext_for_mime).ok_or("Format d'image non pris en charge.")?;
        let bytes = crypto::unb64(&b64).ok_or("Image illisible.")?;
        if bytes.len() > MAX_IMAGE {
            return Err("Image trop lourde (8 Mo au plus).".into());
        }
        let name = format!("{}.{ext}", crypto::random_hex(8));
        let dir = history::media_dir(&st.dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
        image_name = Some(name);
    }
    if text.is_empty() && image_name.is_none() {
        return Err("Message vide.".into());
    }
    let m = add_outgoing(&app, "msg", (!text.is_empty()).then_some(text), image_name, drawing);
    tauri::async_runtime::spawn(deliver(app.clone(), m.id.clone()));
    Ok(m)
}

#[tauri::command]
fn retry(app: AppHandle, id: String) {
    let st = app.state::<AppState>();
    let m = st.history.lock().unwrap().set_status(&id, "sending");
    if let Some(m) = m {
        emit_msg(&app, "update", &m);
        tauri::async_runtime::spawn(deliver(app.clone(), id));
    }
}

#[tauri::command]
fn history(app: AppHandle, before: Option<String>, limit: Option<usize>) -> Vec<Msg> {
    let st = app.state::<AppState>();
    let h = st.history.lock().unwrap();
    h.page(before.as_deref(), limit.unwrap_or(60).clamp(1, 500))
}

/// Relit une image de l'historique (pour dessiner dessus).
#[tauri::command]
fn read_media(app: AppHandle, name: String) -> Result<String, String> {
    if !history::valid_media_name(&name) {
        return Err("Image inconnue.".into());
    }
    let st = app.state::<AppState>();
    std::fs::read(history::media_dir(&st.dir).join(&name))
        .map(|b| crypto::b64(&b))
        .map_err(|_| "Image introuvable.".to_string())
}

#[tauri::command]
fn clear_history(app: AppHandle) {
    let st = app.state::<AppState>();
    st.history.lock().unwrap().clear();
    st.log("historique: effacé");
    let _ = app.emit("history", json!({ "op": "clear" }));
}

#[tauri::command]
fn preview(app: AppHandle, sound_only: bool, silent: bool) {
    if !sound_only {
        let duration = app.state::<AppState>().cfg.lock().unwrap().settings.duration;
        notif::show(&app, "ping", "Toc toc !", "Voici à quoi ressemble un ping", (duration * 1000.0) as u64);
    }
    if !silent {
        play_sound(&app);
    }
}

#[tauri::command]
async fn pick_sound(app: AppHandle) -> Result<serde_json::Value, String> {
    let a = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        a.dialog()
            .file()
            .set_title("Choisir un son")
            .add_filter("Sons", &["wav", "mp3", "ogg", "flac"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(Ok(path)) = picked.map(|f| f.into_path()) {
        let mut s = app.state::<AppState>().cfg.lock().unwrap().settings.clone();
        s.custom_sound = Some(path.to_string_lossy().to_string());
        s.sound = "custom".into();
        apply_settings(&app, s);
        play_sound(&app);
    }
    Ok(get_state(app))
}

#[tauri::command]
fn open_quick(app: AppHandle) {
    show_quick(&app);
}

#[tauri::command]
fn close_quick(app: AppHandle) {
    hide_quick(&app);
}

#[tauri::command]
fn open_main(app: AppHandle, intent: Option<String>) {
    show_main(&app, intent.as_deref());
}

#[tauri::command]
fn open_link(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:")) {
        return Err("Lien non pris en charge.".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn notif_current(app: AppHandle) -> Option<notif::Notice> {
    app.state::<AppState>().notif.lock().unwrap().clone()
}

#[tauri::command]
fn notif_done(app: AppHandle, seq: u64) {
    notif::hide_if_current(&app, seq);
}

/// Clic sur une notification : on la ferme et on ouvre la fenêtre rapide pour répondre.
#[tauri::command]
fn notif_click(app: AppHandle, seq: u64, kind: String) {
    notif::hide_if_current(&app, seq);
    if kind == "ping" || kind == "msg" {
        show_quick(&app);
    }
}

// ───────────────────────── copier un message ─────────────────────────

/// Copie un message : son texte (tel qu'affiché), sinon son image.
#[tauri::command]
fn copy_message(app: AppHandle, id: String, text: Option<String>) -> Result<&'static str, String> {
    let st = app.state::<AppState>();
    let m = st.history.lock().unwrap().get(&id).ok_or("Message introuvable.")?;
    if let Some(t) = text.filter(|t| !t.trim().is_empty()).or(m.text.clone()) {
        app.clipboard().write_text(t).map_err(|e| e.to_string())?;
        return Ok("text");
    }
    let name = m.image.ok_or("Rien à copier.")?;
    let bytes = std::fs::read(history::media_dir(&st.dir).join(&name)).map_err(|_| "Image introuvable.".to_string())?;
    let rgba = image::load_from_memory(&bytes).map_err(|_| "Image illisible.".to_string())?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let img = tauri::image::Image::new_owned(rgba.into_raw(), w, h);
    app.clipboard().write_image(&img).map_err(|e| e.to_string())?;
    Ok("image")
}

// ───────────────────────── mises à jour automatiques ─────────────────────────

/// Cherche une nouvelle version ; si elle existe, la télécharge en arrière-plan.
async fn look_for_update(app: &AppHandle) -> Result<Option<String>, String> {
    let st = app.state::<AppState>();
    if let Some((u, _)) = st.pending_update.lock().unwrap().as_ref() {
        return Ok(Some(u.version.clone()));
    }
    if st.update_busy.swap(true, Ordering::SeqCst) {
        return Ok(None);
    }
    let res = async {
        let updater = app.updater().map_err(|e| e.to_string())?;
        let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        st.log(&format!("mise à jour: version {} trouvée, téléchargement", update.version));
        let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
        let version = update.version.clone();
        *st.pending_update.lock().unwrap() = Some((update, bytes));
        st.log(&format!("mise à jour: version {version} prête"));
        let _ = app.emit("update-ready", &version);
        notif::show(app, "info", "Mise à jour prête", &format!("Toc Toc {version} : redémarrez depuis la fenêtre"), 6000);
        emit_state(app);
        Ok(Some(version))
    }
    .await;
    st.update_busy.store(false, Ordering::SeqCst);
    if let Err(e) = &res {
        st.log(&format!("mise à jour: vérification impossible ({e})"));
    }
    res
}

/// Au démarrage (après 20 s) puis toutes les 6 heures.
fn start_update_loop(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            let _ = look_for_update(&app).await;
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<String>, String> {
    look_for_update(&app).await.map_err(|_| "Impossible de vérifier les mises à jour (connexion ?).".to_string())
}

/// Installe la mise à jour téléchargée puis relance Toc Toc.
#[tauri::command]
fn install_update(app: AppHandle) -> Result<(), String> {
    let st = app.state::<AppState>();
    let Some((update, bytes)) = st.pending_update.lock().unwrap().take() else {
        return Err("Aucune mise à jour prête.".into());
    };
    st.log(&format!("mise à jour: installation de {}", update.version));
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}

// ───────────────────────── démarrage ─────────────────────────

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let dnd = st.cfg.lock().unwrap().settings.dnd;
    let ping_item = MenuItemBuilder::with_id("ping", "Envoyer un ping").build(app)?;
    let quick_item = MenuItemBuilder::with_id("quick", "Écrire un message…").build(app)?;
    let conv_item = MenuItemBuilder::with_id("conv", "Ouvrir la conversation").build(app)?;
    let dnd_item = CheckMenuItemBuilder::with_id("dnd", "Ne pas déranger").checked(dnd).build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", "Réglages…").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quitter Toc Toc").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&ping_item)
        .item(&quick_item)
        .item(&conv_item)
        .separator()
        .item(&dnd_item)
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()?;
    *st.dnd_item.lock().unwrap() = Some(dnd_item);

    let icon_bytes: &[u8] = if cfg!(target_os = "macos") {
        include_bytes!("../icons/tray-mac.png")
    } else {
        include_bytes!("../icons/tray.png")
    };
    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(icon_bytes)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Toc Toc")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "ping" => ping_now(app),
            "quick" => show_quick(app),
            "conv" => show_main(app, None),
            "settings" => show_main(app, Some("settings")),
            "dnd" => {
                let st = app.state::<AppState>();
                let mut s = st.cfg.lock().unwrap().settings.clone();
                s.dnd = !s.dnd;
                apply_settings(app, s);
                emit_state(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // un simple clic sur l'icône envoie un ping
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                ping_now(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Sert les images de l'historique à l'interface (adresse tocmedia://…).
fn media_response(app: &AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    let name = path.trim_start_matches('/');
    let not_found = || tauri::http::Response::builder().status(404).body(Vec::new()).unwrap();
    if !history::valid_media_name(name) {
        return not_found();
    }
    let Some(st) = app.try_state::<AppState>() else { return not_found() };
    match std::fs::read(history::media_dir(&st.dir).join(name)) {
        Ok(bytes) => tauri::http::Response::builder()
            .header("Content-Type", history::mime_for_name(name))
            .header("Cache-Control", "max-age=31536000, immutable")
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes)
            .unwrap(),
        Err(_) => not_found(),
    }
}

pub fn run() {
    // TOCTOC_PROFILE=nom : un second profil séparé sur le même ordinateur (pour les essais)
    let profile = std::env::var("TOCTOC_PROFILE")
        .ok()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()));

    let mut builder = tauri::Builder::default();
    if profile.is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app, None);
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let action = app
                        .state::<AppState>()
                        .shortcuts
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|(id, _)| *id == shortcut.id())
                        .map(|(_, a)| *a);
                    match action {
                        Some("ping") => ping_now(app),
                        Some("quick") => toggle_quick(app),
                        Some("main") => show_main(app, None),
                        _ => {}
                    }
                })
                .build(),
        )
        .register_uri_scheme_protocol("tocmedia", |ctx, request| media_response(ctx.app_handle(), request.uri().path()))
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_settings,
            set_shortcut,
            pair_create,
            pair_join,
            pair_cancel,
            unpair,
            ping,
            send_message,
            retry,
            history,
            read_media,
            clear_history,
            preview,
            pick_sound,
            open_quick,
            close_quick,
            open_main,
            open_link,
            notif_current,
            notif_done,
            notif_click,
            copy_message,
            check_update,
            install_update
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let mut dir = handle.path().app_config_dir()?;
            if let Some(p) = &profile {
                dir = dir.join("profiles").join(p);
            }
            std::fs::create_dir_all(&dir)?;
            let webview_dir = profile.as_ref().map(|_| dir.join("webview"));
            let mut cfg = config::load(&dir);
            // la vérité sur le lancement au démarrage est celle du système
            cfg.settings.autostart = handle.autolaunch().is_enabled().unwrap_or(false);
            let paired = cfg.pairing.is_some();
            config::save(&dir, &cfg);
            let mut hist = History::load(&dir);
            hist.fail_pending();

            app.manage(AppState {
                dir,
                cfg: Mutex::new(cfg),
                http: relay::client(),
                online: AtomicBool::new(false),
                notif: Mutex::new(None),
                notif_seq: AtomicU64::new(0),
                history: Mutex::new(hist),
                listen_task: Mutex::new(None),
                pair_task: Mutex::new(None),
                pair_code: Mutex::new(None),
                seen: Mutex::new(VecDeque::new()),
                last_ping: Mutex::new(None),
                last_ping_at: Mutex::new(None),
                dnd_item: Mutex::new(None),
                shortcuts: Mutex::new(Vec::new()),
                shortcut_ok: Mutex::new([true; 3]),
                pending_update: Mutex::new(None),
                update_busy: AtomicBool::new(false),
            });
            let st = handle.state::<AppState>();
            st.log(&format!("démarrage (version {})", env!("CARGO_PKG_VERSION")));

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let mut main = WebviewWindowBuilder::new(&handle, "main", WebviewUrl::App("index.html".into()))
                .title("Toc Toc")
                .inner_size(480.0, 740.0)
                .min_inner_size(380.0, 500.0)
                .visible(false)
                .center();
            if let Some(d) = &webview_dir {
                main = main.data_directory(d.clone());
            }
            let main = main.build()?;
            // fermer la fenêtre ne quitte pas : l'application reste dans la barre
            let main2 = main.clone();
            main.on_window_event(move |e| {
                if let WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = main2.hide();
                }
            });

            let mut quick = WebviewWindowBuilder::new(&handle, "quick", WebviewUrl::App("quick.html".into()))
                .title("Toc Toc – message rapide")
                .inner_size(440.0, 540.0)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .visible_on_all_workspaces(true)
                .visible(false);
            if let Some(d) = &webview_dir {
                quick = quick.data_directory(d.clone());
            }
            let quick = quick.build()?;
            let quick2 = quick.clone();
            quick.on_window_event(move |e| {
                if let WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = quick2.hide();
                }
            });

            notif::create_window(&handle, webview_dir)?;
            build_tray(&handle)?;
            let ok = apply_shortcuts(&handle);
            if ok.contains(&false) {
                st.log(&format!("raccourcis: certains indisponibles {ok:?}"));
            }
            start_listening(&handle);
            start_update_loop(&handle);

            let hidden = std::env::args().any(|a| a == "--hidden");
            if !paired || !hidden {
                let _ = main.show();
                let _ = main.set_focus();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("erreur au lancement de Toc Toc")
        .run(|_app, event| {
            // pas de fenêtre visible ≠ quitter : on vit dans la barre des tâches / de menus
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
