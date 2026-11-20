mod config;
mod crypto;
mod notif;
mod pairing;
mod relay;
mod sound;

use config::{Config, Settings};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt as _, ShortcutState};

type Task = tauri::async_runtime::JoinHandle<()>;

pub struct AppState {
    pub dir: PathBuf,
    pub cfg: Mutex<Config>,
    pub http: reqwest::Client,
    pub online: AtomicBool,
    pub notif: Mutex<Option<notif::Notice>>,
    pub notif_seq: AtomicU64,
    listen_task: Mutex<Option<Task>>,
    pair_task: Mutex<Option<Task>>,
    pair_code: Mutex<Option<String>>,
    seen: Mutex<VecDeque<String>>,
    last_sent: Mutex<Option<String>>,
    last_ping_at: Mutex<Option<Instant>>,
    dnd_item: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    shortcut_ok: AtomicBool,
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
        if seen.len() > 300 {
            seen.pop_front();
        }
        true
    }
}

/// Ce qui voyage (chiffré) entre les deux ordinateurs.
#[derive(Serialize, Deserialize)]
struct Wire {
    /// ping | ack | bye
    t: String,
    id: String,
    from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
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

fn show_main(app: &AppHandle, focus_text: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if focus_text {
            let _ = app.emit_to("main", "focus-text", ());
        }
    }
}

fn play_sound(app: &AppHandle) {
    let st = app.state::<AppState>();
    let settings = st.cfg.lock().unwrap().settings.clone();
    let dir = st.dir.clone();
    sound::play(&settings, move |line| config::log(&dir, line));
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
                    relay::Event::Message(m) => handle_incoming(&app2, &key, &m),
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

fn handle_incoming(app: &AppHandle, key: &[u8; 32], body: &str) {
    let st = app.state::<AppState>();
    let Some(plain) = crypto::decrypt(key, body) else { return };
    let Ok(msg) = serde_json::from_slice::<Wire>(&plain) else { return };
    let (dev, dnd, duration) = {
        let c = st.cfg.lock().unwrap();
        (c.device_id.clone(), c.settings.dnd, c.settings.duration)
    };
    if msg.from == dev || !st.first_time(&msg.id) {
        return;
    }
    match msg.t.as_str() {
        "ping" => {
            let text: String = msg.text.unwrap_or_default().chars().take(200).collect();
            if dnd {
                st.log("ping: reçu, ignoré (ne pas déranger)");
            } else {
                st.log(&format!("ping: reçu ({} caractères)", text.chars().count()));
                notif::show(app, "ping", "Toc toc !", &text, (duration * 1000.0) as u64);
                play_sound(app);
            }
            let ack = Wire {
                t: "ack".into(),
                id: crypto::random_hex(8),
                from: dev,
                text: None,
                reference: Some(msg.id),
                dnd: dnd.then_some(true),
            };
            publish_wire(app, key, ack);
        }
        "ack" => {
            let is_mine = st.last_sent.lock().unwrap().as_deref() == msg.reference.as_deref();
            if is_mine && msg.reference.is_some() {
                *st.last_sent.lock().unwrap() = None;
                if msg.dnd == Some(true) {
                    st.log("ping: accusé reçu (l'autre est en « ne pas déranger »)");
                    notif::show(app, "ack", "Reçu (ne pas déranger)", "", 2200);
                } else {
                    st.log("ping: accusé reçu");
                    notif::show(app, "ack", "Reçu", "", 1500);
                }
            }
        }
        "bye" => {
            st.log("appairage: l'autre ordinateur s'est dissocié");
            forget_pairing(app);
            show_main(app, false);
        }
        _ => {}
    }
}

fn publish_wire(app: &AppHandle, key: &[u8; 32], wire: Wire) {
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

fn forget_pairing(app: &AppHandle) {
    let st = app.state::<AppState>();
    st.cfg.lock().unwrap().pairing = None;
    st.save();
    start_listening(app);
}

// ───────────────────────── envoyer un ping ─────────────────────────

pub async fn send_ping(app: AppHandle, text: String) -> Result<(), String> {
    let st = app.state::<AppState>();
    let (pairing, relay_url, dev) = {
        let c = st.cfg.lock().unwrap();
        (c.pairing.clone(), c.relay.clone(), c.device_id.clone())
    };
    let Some(p) = pairing else {
        show_main(&app, false);
        return Err("Cet ordinateur n'est pas encore associé.".into());
    };
    let key = crypto::key_from_b64(&p.key).ok_or("Clé illisible.")?;
    {
        // évite les rafales (touche du raccourci maintenue)
        let mut last = st.last_ping_at.lock().unwrap();
        if last.map(|t| t.elapsed() < Duration::from_millis(700)).unwrap_or(false) {
            return Ok(());
        }
        *last = Some(Instant::now());
    }
    let text: String = text.trim().chars().take(200).collect();
    let id = crypto::random_hex(8);
    st.first_time(&id);
    let wire = Wire {
        t: "ping".into(),
        id: id.clone(),
        from: dev,
        text: (!text.is_empty()).then(|| text.clone()),
        reference: None,
        dnd: None,
    };
    let body = crypto::encrypt(&key, &serde_json::to_vec(&wire).unwrap());
    match relay::publish(&st.http, &relay_url, &p.topic, body).await {
        Ok(()) => {
            *st.last_sent.lock().unwrap() = Some(id);
            st.log("ping: envoyé");
            notif::show(&app, "sent", "Ping envoyé", "", 2000);
            Ok(())
        }
        Err(e) => {
            st.log(&format!("ping: échec de l'envoi ({e})"));
            notif::show(&app, "error", "Ping non envoyé", "Vérifiez la connexion Internet", 3500);
            Err("Ping non envoyé : vérifiez la connexion Internet.".into())
        }
    }
}

fn ping_in_background(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = send_ping(app, String::new()).await;
    });
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
                notif::show(&app2, "info", "Ordinateurs associés", "Envoyez votre premier ping !", 4000);
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

fn apply_shortcut(app: &AppHandle, shortcut: &str) -> bool {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let ok = shortcut.is_empty() || gs.register(shortcut).is_ok();
    app.state::<AppState>().shortcut_ok.store(ok, Ordering::SeqCst);
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
    new.sanitize();
    let old = {
        let mut c = st.cfg.lock().unwrap();
        std::mem::replace(&mut c.settings, new.clone())
    };
    st.save();
    if old.shortcut != new.shortcut {
        let ok = apply_shortcut(app, &new.shortcut);
        st.log(&format!("réglages: raccourci « {} » {}", new.shortcut, if ok { "actif" } else { "indisponible" }));
    }
    if old.autostart != new.autostart {
        let al = app.autolaunch();
        let res = if new.autostart { al.enable() } else { al.disable() };
        #[cfg(windows)]
        if new.autostart {
            quote_autostart_path(&app.package_info().name);
        }
        st.log(&format!("réglages: lancement au démarrage = {} ({})", new.autostart, if res.is_ok() { "ok" } else { "erreur" }));
    }
    if old.dnd != new.dnd {
        if let Some(item) = st.dnd_item.lock().unwrap().as_ref() {
            let _ = item.set_checked(new.dnd);
        }
        st.log(&format!("réglages: ne pas déranger = {}", new.dnd));
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
        "pair_code": *st.pair_code.lock().unwrap(),
        "shortcut_ok": st.shortcut_ok.load(Ordering::SeqCst),
        "mac": cfg!(target_os = "macos"),
        "version": env!("CARGO_PKG_VERSION"),
    })
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: Settings) -> serde_json::Value {
    apply_settings(&app, settings);
    get_state(app)
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
        let bye = Wire { t: "bye".into(), id: crypto::random_hex(8), from: dev, text: None, reference: None, dnd: None };
        publish_wire(&app, &key, bye);
    }
    st.log("appairage: dissocié");
    forget_pairing(&app);
}

#[tauri::command]
async fn ping(app: AppHandle, text: String) -> Result<(), String> {
    send_ping(app, text).await
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
fn notif_current(app: AppHandle) -> Option<notif::Notice> {
    app.state::<AppState>().notif.lock().unwrap().clone()
}

#[tauri::command]
fn notif_done(app: AppHandle, seq: u64) {
    notif::hide_if_current(&app, seq);
}

// ───────────────────────── démarrage ─────────────────────────

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let dnd = st.cfg.lock().unwrap().settings.dnd;
    let ping_item = MenuItemBuilder::with_id("ping", "Envoyer un ping").build(app)?;
    let write_item = MenuItemBuilder::with_id("write", "Écrire un message…").build(app)?;
    let dnd_item = CheckMenuItemBuilder::with_id("dnd", "Ne pas déranger").checked(dnd).build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", "Réglages…").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quitter Toc Toc").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&ping_item)
        .item(&write_item)
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
            "ping" => ping_in_background(app),
            "write" => show_main(app, true),
            "settings" => show_main(app, false),
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
                ping_in_background(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    // TOCTOC_PROFILE=nom : un second profil séparé sur le même ordinateur (pour les essais)
    let profile = std::env::var("TOCTOC_PROFILE")
        .ok()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()));

    let mut builder = tauri::Builder::default();
    if profile.is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app, false);
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        ping_in_background(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_settings,
            pair_create,
            pair_join,
            pair_cancel,
            unpair,
            ping,
            preview,
            pick_sound,
            notif_current,
            notif_done
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
            let shortcut = cfg.settings.shortcut.clone();
            config::save(&dir, &cfg);

            app.manage(AppState {
                dir,
                cfg: Mutex::new(cfg),
                http: relay::client(),
                online: AtomicBool::new(false),
                notif: Mutex::new(None),
                notif_seq: AtomicU64::new(0),
                listen_task: Mutex::new(None),
                pair_task: Mutex::new(None),
                pair_code: Mutex::new(None),
                seen: Mutex::new(VecDeque::new()),
                last_sent: Mutex::new(None),
                last_ping_at: Mutex::new(None),
                dnd_item: Mutex::new(None),
                shortcut_ok: AtomicBool::new(true),
            });
            let st = handle.state::<AppState>();
            st.log(&format!("démarrage (version {})", env!("CARGO_PKG_VERSION")));

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let mut main = WebviewWindowBuilder::new(&handle, "main", WebviewUrl::App("index.html".into()))
                .title("Toc Toc")
                .inner_size(420.0, 720.0)
                .min_inner_size(380.0, 480.0)
                .maximizable(false)
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

            notif::create_window(&handle, webview_dir)?;
            build_tray(&handle)?;
            if !apply_shortcut(&handle, &shortcut) {
                st.log("raccourci: indisponible (déjà pris par une autre application)");
            }
            start_listening(&handle);

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
