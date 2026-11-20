//! Chiffrement de bout en bout et dérivation des clés.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("générateur aléatoire du système indisponible");
    b
}

pub fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).expect("générateur aléatoire du système indisponible");
    hex::encode(b)
}

/// 32 caractères sans ambiguïté (pas de I, O, 0, 1).
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 8;

/// Code d'appairage : 8 caractères = 40 bits d'aléa.
pub fn new_code() -> String {
    random_bytes::<CODE_LEN>()
        .iter()
        .map(|b| ALPHABET[(b & 31) as usize] as char)
        .collect()
}

/// Nettoie ce que l'utilisateur a tapé (espaces, tirets, minuscules).
pub fn normalize_code(input: &str) -> Option<String> {
    let s: String = input
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.is_ascii() && ALPHABET.contains(&(*c as u8)))
        .collect();
    (s.len() == CODE_LEN).then_some(s)
}

/// Canal de rendez-vous, dérivé du code par Argon2id (coûteux à énumérer).
pub fn rendezvous_topic(code: &str) -> String {
    let mut out = [0u8; 20];
    argon2::Argon2::default()
        .hash_password_into(code.as_bytes(), b"toctoc-rendezvous-v1", &mut out)
        .expect("argon2");
    format!("ttr{}", hex::encode(out))
}

pub fn hkdf(ikm: &[u8], info: &[u8], out: &mut [u8]) {
    Hkdf::<Sha256>::new(Some(b"toctoc-v1"), ikm)
        .expand(info, out)
        .expect("hkdf");
}

pub struct SessionKeys {
    pub confirm: [u8; 32],
    pub topic: String,
    pub key: [u8; 32],
}

/// À partir du secret partagé issu de SPAKE2 : clé de confirmation, canal et clé durables.
pub fn session_keys(shared: &[u8]) -> SessionKeys {
    let mut confirm = [0u8; 32];
    let mut key = [0u8; 32];
    let mut topic = [0u8; 20];
    hkdf(shared, b"confirm", &mut confirm);
    hkdf(shared, b"message-key", &mut key);
    hkdf(shared, b"topic", &mut topic);
    SessionKeys { confirm, topic: format!("tt{}", hex::encode(topic)), key }
}

/// Chiffre : 24 octets de nonce suivis du texte chiffré authentifié.
pub fn encrypt_raw(key: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = random_bytes::<24>();
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .expect("chiffrement");
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    out
}

pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> String {
    B64.encode(encrypt_raw(key, plaintext))
}

pub fn decrypt(key: &[u8; 32], message: &str) -> Option<Vec<u8>> {
    decrypt_raw(key, &B64.decode(message.trim()).ok()?)
}

pub fn decrypt_raw(key: &[u8; 32], raw: &[u8]) -> Option<Vec<u8>> {
    if raw.len() < 24 + 16 {
        return None;
    }
    let (nonce, ct) = raw.split_at(24);
    XChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(XNonce::from_slice(nonce), ct)
        .ok()
}

pub fn key_to_b64(key: &[u8; 32]) -> String {
    B64.encode(key)
}

pub fn key_from_b64(s: &str) -> Option<[u8; 32]> {
    B64.decode(s).ok()?.try_into().ok()
}

pub fn b64(data: &[u8]) -> String {
    B64.encode(data)
}

pub fn unb64(s: &str) -> Option<Vec<u8>> {
    B64.decode(s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let k = random_bytes::<32>();
        let c = encrypt(&k, "héllo".as_bytes());
        assert_eq!(decrypt(&k, &c).unwrap(), "héllo".as_bytes());
        let k2 = random_bytes::<32>();
        assert!(decrypt(&k2, &c).is_none());
        assert!(decrypt(&k, "pas du base64 !").is_none());
    }

    #[test]
    fn codes() {
        let c = new_code();
        assert_eq!(c.len(), CODE_LEN);
        assert_eq!(normalize_code(&c.to_lowercase()), Some(c.clone()));
        let dashed = format!(" {}-{} ", &c[..4], &c[4..]);
        assert_eq!(normalize_code(&dashed), Some(c.clone()));
        assert_eq!(normalize_code("ABC"), None);
        assert_eq!(rendezvous_topic(&c), rendezvous_topic(&c));
        assert_ne!(rendezvous_topic(&c), rendezvous_topic(&new_code()));
    }

    #[test]
    fn spake_agrees() {
        use spake2::{Ed25519Group, Identity, Password, Spake2};
        let id = Identity::new(b"toctoc-v1");
        let (a, ma) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"ABCDEFGH"), &id);
        let (b, mb) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"ABCDEFGH"), &id);
        let ka = session_keys(&a.finish(&mb).unwrap());
        let kb = session_keys(&b.finish(&ma).unwrap());
        assert_eq!(ka.topic, kb.topic);
        assert_eq!(ka.key, kb.key);
        let (c, _mc) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"ZZZZZZZZ"), &id);
        let kc = session_keys(&c.finish(&ma).unwrap());
        assert_ne!(ka.key, kc.key);
    }
}
