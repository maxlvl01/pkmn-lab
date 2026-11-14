//! PokeMMO's own learnsets, from the PokeMMO Hub data. That repository has no licence, so the file is
//! downloaded on first use and cached on this PC, never bundled (same as the sprites).
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

// Pinned to one commit so a change in the file's layout can't break the parser.
const URL: &str = "https://raw.githubusercontent.com/PokeMMO-Tools/pokemmo-hub/d462e849a7319b52ba6b5f15e368c24e23529dc8/src/data/pokemmo/monster.json";
static FETCHING: AtomicBool = AtomicBool::new(false);

/// German move names where PokeMMO's translation differs from the official games' (from the PokeMMO Hub's
/// de-DE translation.json). "Schnabel" is confirmed in PokeMMO itself; the others are unverified.
/// shortcut: that file covers only some moves, so names missing here may still differ in PokeMMO.
pub const DE_MOVE_NAMES: &[(u16, &str)] = &[
    (4, "Kometenschlag"), (64, "Schnabel"), (73, "Egel-Samen"), (94, "Psycho"), (96, "Meditieren"), (170, "Gedankenleser"),
    (181, "Puderschnee"), (187, "Bauch Trommel"), (199, "Anvisieren"), (234, "Morgensonne"), (236, "Mondlicht"), (268, "Laden"),
    (312, "Aromatherapie"), (346, "Wassersport"), (377, "Heilungshemmer"), (383, "Nachahmung"),
];

pub fn de_move_name(id: u16) -> Option<&'static str> {
    DE_MOVE_NAMES.iter().find(|x| x.0 == id).map(|x| x.1)
}

fn path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("pkmn-lab").join("pokemmo-monster-d462e849.json")
}

pub fn ready() -> bool { path().exists() }

pub fn forget() { let _ = std::fs::remove_file(path()); }

/// Download the file in the background (once per run); repaints when it is there.
pub fn fetch(ctx: eframe::egui::Context) {
    if ready() || FETCHING.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(move || {
        // ~18 MB: the default 20 s limit of `curl` is too short on slow lines (the last --max-time wins).
        if let Ok(bytes) = crate::update::curl(&["--max-time", "600", URL]) {
            let p = path();
            let tmp = p.with_extension("tmp");
            if parse(&bytes).is_some() && std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &p)).is_ok() {
                ctx.request_repaint();
            }
        }
        FETCHING.store(false, Ordering::SeqCst); // failed: try again the next time PokeMMO is picked
    });
}

pub type LevelUp = HashMap<u16, Vec<(u8, u16)>>;
pub type Machine = HashMap<u16, Vec<u16>>;

/// Level-up and machine ("TM??") moves per species 1-649; extra forms and event entries are skipped.
pub fn parse(bytes: &[u8]) -> Option<(LevelUp, Machine)> {
    let all: Vec<serde_json::Value> = serde_json::from_slice(bytes).ok()?;
    let (mut lv, mut tm) = (LevelUp::new(), Machine::new());
    for m in &all {
        let Some(sp) = m["id"].as_u64().filter(|&i| (1..=649).contains(&i)).map(|i| i as u16) else { continue };
        for mv in m["moves"].as_array().into_iter().flatten() {
            let Some(id) = mv["id"].as_u64().map(|i| i as u16) else { continue };
            match mv["type"].as_str() {
                Some("level") => {
                    // PokeMMO repeats a move at several levels (re-learn lists): keep the earliest.
                    let l = mv["level"].as_u64().unwrap_or(1).min(100) as u8;
                    let list = lv.entry(sp).or_default();
                    match list.iter_mut().find(|e| e.1 == id) {
                        Some(e) => e.0 = e.0.min(l),
                        None => list.push((l, id)),
                    }
                }
                Some("TM??") => { let list = tm.entry(sp).or_default(); if !list.contains(&id) { list.push(id); } }
                _ => {}
            }
        }
    }
    (lv.len() > 600).then_some((lv, tm))
}

/// The cached learnsets, or None until the download has finished.
pub fn learnsets() -> Option<(LevelUp, Machine)> {
    parse(&std::fs::read(path()).ok()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_cached_file() {
        // Needs the downloaded file (the app fetches it the first time a PokeMMO region is picked); skipped otherwise.
        let Some((lv, tm)) = super::learnsets() else { return };
        assert!(lv[&25].contains(&(21, 85)), "Pikachu: Donnerblitz at 21 in PokeMMO"); // B2W2: 29
        assert!(lv[&1].iter().filter(|e| e.1 == 45).count() == 1, "Bisasam: Heuler only once");
        assert!(tm[&1].len() > 30);
        assert!(!lv.contains_key(&650));
    }

    #[test]
    #[ignore] // network: cargo test --release -- --ignored
    fn fetch_downloads() {
        super::forget();
        super::fetch(eframe::egui::Context::default());
        for _ in 0..600 {
            if super::ready() { return parses_cached_file(); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("no file after 60 s");
    }
}
