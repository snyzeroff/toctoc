//! La fenêtre de notification : sans bord, transparente, toujours au-dessus.
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Serialize)]
pub struct Notice {
    pub seq: u64,
    /// ping | sent | ack | error | info
    pub kind: String,
    pub title: String,
    pub text: String,
    pub size: String,
    pub duration_ms: u64,
}

/// Marge transparente autour de la carte (place pour l'ombre et la secousse).
const PAD: f64 = 20.0;
/// Écart entre la carte et le bord de l'écran.
const MARGIN: f64 = 12.0;

fn card_size(size: &str) -> (f64, f64) {
    match size {
        "small" => (230.0, 52.0),
        "large" => (370.0, 84.0),
        _ => (290.0, 64.0),
    }
}

pub fn create_window(app: &AppHandle, data_dir: Option<PathBuf>) -> tauri::Result<()> {
    let (cw, ch) = card_size("medium");
    let mut b = WebviewWindowBuilder::new(app, "notif", WebviewUrl::App("notif.html".into()))
        .title("Toc Toc – notification")
        .inner_size(cw + 2.0 * PAD, ch + 2.0 * PAD)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .focused(false)
        .focusable(false)
        .visible_on_all_workspaces(true)
        .visible(false);
    if let Some(d) = data_dir {
        b = b.data_directory(d);
    }
    b.build()?;
    Ok(())
}

fn place(app: &AppHandle, size: &str, position: &str) {
    let Some(win) = app.get_webview_window("notif") else { return };
    let (cw, ch) = card_size(size);
    let (w, h) = (cw + 2.0 * PAD, ch + 2.0 * PAD);
    let _ = win.set_size(LogicalSize::new(w, h));
    let Ok(Some(mon)) = win.primary_monitor() else { return };
    let scale = mon.scale_factor();
    let area = mon.work_area();
    let (ax, ay) = (area.position.x as f64, area.position.y as f64);
    let (aw, ah) = (area.size.width as f64, area.size.height as f64);
    let (pw, ph) = (w * scale, h * scale);
    let edge = (MARGIN - PAD) * scale;
    let x = if position.ends_with("left") {
        ax + edge
    } else if position.ends_with("right") {
        ax + aw - pw - edge
    } else {
        ax + (aw - pw) / 2.0
    };
    let y = if position.starts_with("top") {
        ay + edge
    } else if position.starts_with("bottom") {
        ay + ah - ph - edge
    } else {
        ay + (ah - ph) / 2.0
    };
    let _ = win.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Affiche une notification (remplace celle en cours).
pub fn show(app: &AppHandle, kind: &str, title: &str, text: &str, duration_ms: u64) {
    let st = app.state::<AppState>();
    let (size, position) = {
        let c = st.cfg.lock().unwrap();
        (
            // les petites confirmations (« envoyé », « reçu ») restent toujours discrètes
            if kind == "ping" || kind == "info" { c.settings.size.clone() } else { "small".to_string() },
            c.settings.position.clone(),
        )
    };
    let seq = st.notif_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let notice = Notice {
        seq,
        kind: kind.into(),
        title: title.into(),
        text: text.into(),
        size: size.clone(),
        duration_ms,
    };
    *st.notif.lock().unwrap() = Some(notice.clone());
    place(app, &size, &position);
    if let Some(win) = app.get_webview_window("notif") {
        let _ = app.emit_to("notif", "notify", notice);
        let _ = win.show();
        let _ = win.set_always_on_top(true);
    }
    st.log(&format!("notification: affichée (type {kind}, taille {size}, position {position})"));
    // filet de sécurité si la page ne répond pas
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(duration_ms + 1500)).await;
        hide_if_current(&app2, seq);
    });
}

pub fn hide_if_current(app: &AppHandle, seq: u64) {
    let st = app.state::<AppState>();
    {
        let mut cur = st.notif.lock().unwrap();
        match cur.as_ref() {
            Some(n) if n.seq == seq => *cur = None,
            _ => return,
        }
    }
    if let Some(win) = app.get_webview_window("notif") {
        let _ = win.hide();
    }
}
