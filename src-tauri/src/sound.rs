//! Lecture du son de notification.
use crate::config::Settings;
use std::io::Cursor;
use std::time::{Duration, Instant};

fn builtin(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "carillon" => include_bytes!("../sounds/carillon.wav").as_slice(),
        "goutte" => include_bytes!("../sounds/goutte.wav").as_slice(),
        "toctoc" => include_bytes!("../sounds/toctoc.wav").as_slice(),
        "cloche" => include_bytes!("../sounds/cloche.wav").as_slice(),
        "bulle" => include_bytes!("../sounds/bulle.wav").as_slice(),
        _ => return None,
    })
}

/// Joue le son choisi dans les réglages, sans bloquer. `log` reçoit le résultat.
pub fn play(settings: &Settings, log: impl Fn(&str) + Send + 'static) {
    if settings.sound == "none" || settings.volume <= 0.0 {
        log("son: aucun (muet)");
        return;
    }
    let name = settings.sound.clone();
    let custom = settings.custom_sound.clone();
    // courbe perceptive : le curseur paraît linéaire à l'oreille
    let volume = settings.volume * settings.volume;
    std::thread::spawn(move || {
        let mut used = name.clone();
        let data: Vec<u8> = if name == "custom" {
            match custom.as_deref().map(std::fs::read) {
                Some(Ok(bytes)) if bytes.len() <= 20_000_000 => bytes,
                _ => {
                    used = "carillon (fichier illisible)".into();
                    builtin("carillon").unwrap().to_vec()
                }
            }
        } else {
            builtin(&name).or(builtin("carillon")).unwrap().to_vec()
        };
        let result = (|| -> Result<(), String> {
            let (_stream, handle) = rodio::OutputStream::try_default().map_err(|e| e.to_string())?;
            let sink = rodio::Sink::try_new(&handle).map_err(|e| e.to_string())?;
            sink.set_volume(volume);
            match rodio::Decoder::new(Cursor::new(data)) {
                Ok(s) => sink.append(s),
                Err(_) => {
                    used = "carillon (format non reconnu)".into();
                    let s = rodio::Decoder::new(Cursor::new(builtin("carillon").unwrap().to_vec()))
                        .map_err(|e| e.to_string())?;
                    sink.append(s);
                }
            }
            let start = Instant::now();
            // au plus 10 secondes, même pour un long fichier
            while !sink.empty() && start.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(40));
            }
            sink.stop();
            Ok(())
        })();
        match result {
            Ok(()) => log(&format!("son: joué ({used})")),
            Err(e) => log(&format!("son: erreur ({e})")),
        }
    });
}
