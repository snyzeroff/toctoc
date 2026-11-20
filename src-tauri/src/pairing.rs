//! Appairage par code court.
//!
//! Les deux ordinateurs connaissent le même code de 8 caractères. Ils se retrouvent sur un
//! canal de rendez-vous dérivé du code, y échangent un message SPAKE2 chacun (un échange de
//! clés protégé par mot de passe : écouter l'échange ne permet pas de retrouver la clé, même
//! en devinant le code après coup), puis se confirment mutuellement la clé obtenue.
//! Le canal et la clé définitifs sortent de ce secret partagé, pas du code.
use crate::{config::Pairing, crypto, relay, AppState};
use serde::{Deserialize, Serialize};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize)]
struct RdvMsg {
    t: String,
    from: String,
    m: String,
}

struct AbortOnDrop(tauri::async_runtime::JoinHandle<()>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub async fn run(app: AppHandle, code: String, host: bool) -> Result<Pairing, String> {
    let st = app.state::<AppState>();
    let (relay_url, dev) = {
        let c = st.cfg.lock().unwrap();
        (c.relay.clone(), c.device_id.clone())
    };
    let http = st.http.clone();

    let code_for_topic = code.clone();
    let topic = tauri::async_runtime::spawn_blocking(move || crypto::rendezvous_topic(&code_for_topic))
        .await
        .map_err(|e| e.to_string())?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<relay::Event>();
    let _listener = AbortOnDrop({
        let (http, relay_url, topic) = (http.clone(), relay_url.clone(), topic.clone());
        tauri::async_runtime::spawn(async move {
            loop {
                let txc = tx.clone();
                let _ = relay::listen(&http, &relay_url, &topic, Some("15m".into()), move |e| {
                    let _ = txc.send(e);
                })
                .await;
                if tx.is_closed() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        })
    });

    let (state, my_msg) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(code.as_bytes()),
        &Identity::new(b"toctoc-v1"),
    );
    let mut state = Some(state);
    let mut keys: Option<crypto::SessionKeys> = None;
    let mut peer_confirm: Option<String> = None;
    let mut sent_pake = false;

    let wait = if host { 600 } else { 25 };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait);
    let timeout_msg = if host {
        "Le code a expiré. Créez-en un nouveau."
    } else {
        "Code introuvable. Vérifiez-le, et que l'autre ordinateur affiche bien ce code."
    };

    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .map_err(|_| timeout_msg.to_string())?
            .ok_or_else(|| "Appairage interrompu.".to_string())?;
        match ev {
            relay::Event::Open => {
                if !sent_pake {
                    let msg = RdvMsg { t: "pake".into(), from: dev.clone(), m: crypto::b64(&my_msg) };
                    relay::publish(&http, &relay_url, &topic, serde_json::to_string(&msg).unwrap())
                        .await
                        .map_err(|_| "Pas de connexion Internet.".to_string())?;
                    sent_pake = true;
                }
            }
            relay::Event::Alive => {}
            relay::Event::Message(body) => {
                let Ok(m) = serde_json::from_str::<RdvMsg>(&body) else { continue };
                if m.from == dev {
                    continue;
                }
                match m.t.as_str() {
                    "pake" => {
                        if let (Some(s), Some(bytes)) = (state.take(), crypto::unb64(&m.m)) {
                            let shared = s
                                .finish(&bytes)
                                .map_err(|_| "Échange de clés invalide. Recommencez.".to_string())?;
                            let k = crypto::session_keys(&shared);
                            let confirm = RdvMsg {
                                t: "confirm".into(),
                                from: dev.clone(),
                                m: crypto::encrypt(&k.confirm, dev.as_bytes()),
                            };
                            relay::publish(&http, &relay_url, &topic, serde_json::to_string(&confirm).unwrap())
                                .await
                                .map_err(|_| "Pas de connexion Internet.".to_string())?;
                            keys = Some(k);
                        }
                    }
                    "confirm" => peer_confirm = Some(m.m),
                    _ => {}
                }
            }
        }
        if let (Some(k), Some(c)) = (&keys, &peer_confirm) {
            match crypto::decrypt(&k.confirm, c) {
                Some(pt) if pt != dev.as_bytes() => {
                    return Ok(Pairing { topic: k.topic.clone(), key: crypto::key_to_b64(&k.key) });
                }
                // confirmation d'une ancienne tentative : on l'ignore
                _ => peer_confirm = None,
            }
        }
    }
}
