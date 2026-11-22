//! Relais public (ntfy) : on y publie et on y écoute des messages déjà chiffrés.
use std::time::Duration;

pub enum Event {
    /// la connexion d'écoute est ouverte
    Open,
    /// signe de vie du relais
    Alive,
    /// un message (contenu opaque), ou l'adresse de sa pièce jointe s'il était gros
    Message { text: String, attachment: Option<String> },
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("toctoc/0.3")
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

/// Publie un gros contenu binaire : le relais le garde en pièce jointe (3 heures).
pub async fn publish_bytes(http: &reqwest::Client, relay: &str, topic: &str, body: Vec<u8>) -> Result<(), String> {
    let resp = http
        .put(format!("{relay}/{topic}"))
        .timeout(Duration::from_secs(120))
        .header("Filename", "m.bin")
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

/// Télécharge une pièce jointe du relais (taille limitée).
pub async fn download(http: &reqwest::Client, relay: &str, url: &str, max: usize) -> Result<Vec<u8>, String> {
    if !url.starts_with(&format!("{relay}/file/")) {
        return Err("adresse inattendue".into());
    }
    let mut resp = http
        .get(url)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("réseau : {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("relais : HTTP {}", resp.status().as_u16()));
    }
    let mut out = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("réseau : {e}"))? {
        out.extend_from_slice(&chunk);
        if out.len() > max {
            return Err("pièce jointe trop grosse".into());
        }
    }
    Ok(out)
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
                    let text = v.get("message").and_then(|m| m.as_str()).unwrap_or_default().to_string();
                    let attachment = v
                        .pointer("/attachment/url")
                        .and_then(|u| u.as_str())
                        .map(|u| u.to_string());
                    on_event(Event::Message { text, attachment });
                }
                _ => {}
            }
        }
    }
}
