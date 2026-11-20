//! Historique de la conversation, gardé sur cet ordinateur (history.json + dossier media/).
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Au-delà, les plus anciens messages (et leurs images) sont effacés.
const MAX_MESSAGES: usize = 5000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Msg {
    pub id: String,
    /// vrai = envoyé par cet ordinateur
    pub out: bool,
    /// millisecondes depuis 1970
    pub ts: u64,
    /// ping | msg
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// nom du fichier dans media/
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default)]
    pub drawing: bool,
    /// sending | sent | delivered | failed | received
    pub status: String,
}

pub struct History {
    dir: PathBuf,
    msgs: Vec<Msg>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn media_dir(dir: &Path) -> PathBuf {
    dir.join("media")
}

/// Nom de fichier sûr pour une image : identifiant hexadécimal + extension connue.
pub fn valid_media_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else { return false };
    !stem.is_empty()
        && stem.len() <= 32
        && stem.chars().all(|c| c.is_ascii_hexdigit())
        && ["jpg", "png", "gif", "webp"].contains(&ext)
}

pub fn ext_for_mime(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return None,
    })
}

pub fn mime_for_name(name: &str) -> &'static str {
    match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

impl History {
    pub fn load(dir: &Path) -> Self {
        let msgs = std::fs::read(dir.join("history.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { dir: dir.to_path_buf(), msgs }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_vec(&self.msgs) {
            let tmp = self.dir.join("history.json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, self.dir.join("history.json"));
            }
        }
    }

    pub fn add(&mut self, msg: Msg) {
        if self.msgs.iter().any(|m| m.id == msg.id) {
            return;
        }
        self.msgs.push(msg);
        if self.msgs.len() > MAX_MESSAGES {
            let extra = self.msgs.len() - MAX_MESSAGES;
            for old in self.msgs.drain(..extra) {
                if let Some(img) = old.image {
                    let _ = std::fs::remove_file(media_dir(&self.dir).join(img));
                }
            }
        }
        self.save();
    }

    pub fn get(&self, id: &str) -> Option<Msg> {
        self.msgs.iter().find(|m| m.id == id).cloned()
    }

    /// Change le statut ; renvoie le message modifié.
    pub fn set_status(&mut self, id: &str, status: &str) -> Option<Msg> {
        let m = self.msgs.iter_mut().find(|m| m.id == id)?;
        // « reçu par l'autre » ne redescend jamais à « envoyé »
        if m.status == "delivered" && status == "sent" {
            return None;
        }
        m.status = status.to_string();
        let out = m.clone();
        self.save();
        Some(out)
    }

    /// Les `limit` messages qui précèdent `before` (ou les derniers).
    pub fn page(&self, before: Option<&str>, limit: usize) -> Vec<Msg> {
        let end = before
            .and_then(|id| self.msgs.iter().position(|m| m.id == id))
            .unwrap_or(self.msgs.len());
        let start = end.saturating_sub(limit);
        self.msgs[start..end].to_vec()
    }

    pub fn clear(&mut self) {
        self.msgs.clear();
        let _ = std::fs::remove_dir_all(media_dir(&self.dir));
        self.save();
    }

    /// Au démarrage : un envoi interrompu est marqué « échec » (on pourra le renvoyer).
    pub fn fail_pending(&mut self) {
        let mut changed = false;
        for m in self.msgs.iter_mut().filter(|m| m.status == "sending") {
            m.status = "failed".into();
            changed = true;
        }
        if changed {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_media_name("0a1b.jpg"));
        assert!(!valid_media_name("../x.jpg"));
        assert!(!valid_media_name("abc.exe"));
        assert!(!valid_media_name(".png"));
    }

    #[test]
    fn paging() {
        let dir = std::env::temp_dir().join(format!("toctoc-test-{}", crate::crypto::random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        let mut h = History::load(&dir);
        for i in 0..10 {
            h.add(Msg {
                id: format!("{i}"),
                out: i % 2 == 0,
                ts: i,
                kind: "msg".into(),
                text: Some(format!("m{i}")),
                image: None,
                drawing: false,
                status: "sent".into(),
            });
        }
        assert_eq!(h.page(None, 3).iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["7", "8", "9"]);
        assert_eq!(h.page(Some("7"), 2).iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["5", "6"]);
        h.set_status("9", "delivered");
        assert!(h.set_status("9", "sent").is_none());
        let again = History::load(&dir);
        assert_eq!(again.page(None, 100).len(), 10);
        assert_eq!(again.get("9").unwrap().status, "delivered");
        let _ = std::fs::remove_dir_all(dir);
    }
}
