//! Relais public (ntfy) : on y publie et on y écoute des messages déjà chiffrés.
use std::time::Duration;

pub enum Event {
    /// la connexion d'écoute est ouverte
    Open,
    /// signe de vie du relais
    Alive,
    /// un message (contenu opaque)
    Message(String),
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("toctoc/0.1")
        .connect_timeout(Duration::from_secs(15))
        .build()
        .expect("client HTTP")
}

pub async fn publish(http: &reqwest::Client, relay: &str, topic: &str, body: String) -> Result<(), String> {
    let resp = http
        .post(format!("{relay}/{topic}"))
        .timeout(Duration::from_secs(15))
        .body(body)
        .send()
        .await
        .map_err(|e| format!("réseau : {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("relais : HTTP {}", resp.status().as_u16()))
    }
}

/// Écoute un canal jusqu'à la coupure. `since` (ex. "30s") rejoue les messages récents.
pub async fn listen(
    http: &reqwest::Client,
    relay: &str,
    topic: &str,
    since: Option<String>,
    mut on_event: impl FnMut(Event),
) -> Result<(), String> {
    let mut url = format!("{relay}/{topic}/json");
    if let Some(s) = since {
        url.push_str(&format!("?since={s}"));
    }
    let mut resp = http.get(url).send().await.map_err(|e| format!("réseau : {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("relais : HTTP {}", resp.status().as_u16()));
    }
    let mut buf: Vec<u8> = Vec::new();
    loop {
        // le relais envoie un signe de vie toutes les ~45 s
        let chunk = tokio::time::timeout(Duration::from_secs(100), resp.chunk())
            .await
            .map_err(|_| "silence du relais".to_string())?
            .map_err(|e| format!("réseau : {e}"))?;
        let Some(chunk) = chunk else {
            return Err("connexion fermée".into());
        };
        buf.extend_from_slice(&chunk);
        if buf.len() > 1_000_000 {
            return Err("réponse trop grosse".into());
        }
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&line) else {
                continue;
            };
            match v.get("event").and_then(|e| e.as_str()) {
                Some("open") => on_event(Event::Open),
                Some("keepalive") => on_event(Event::Alive),
                Some("message") => {
                    if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
                        on_event(Event::Message(m.to_string()));
                    }
                }
                _ => {}
            }
        }
    }
}
