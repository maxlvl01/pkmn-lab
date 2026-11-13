//! Pokémon and trainer sprites, downloaded on first use and cached on this PC.
//! They are Nintendo/Game Freak art, so they are never bundled with the program.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

const POKEAPI: &str = "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/";
const SHOWDOWN: &str = "https://play.pokemonshowdown.com/sprites/trainers/";

pub struct Sprites {
    dir: PathBuf,
    /// key -> file:// uri once downloaded; None = queued, missing or failed this session
    known: Arc<Mutex<HashMap<String, Option<String>>>>,
    queue: Sender<(String, Vec<String>)>,
}

fn uri(path: &std::path::Path) -> String {
    // Windows needs file:///C:/... or the drive letter is read as a host name.
    format!("file:///{}", path.display().to_string().replace('\\', "/").trim_start_matches('/'))
}

/// The game's own sprite folder (research/sprites.md); later games use the HOME renders.
fn folder(game_id: &str) -> String {
    match game_id {
        "red-blue" | "yellow" => format!("versions/generation-i/{game_id}/transparent/"),
        "gold-silver" => "versions/generation-ii/gold/transparent/".into(),
        "crystal" => "versions/generation-ii/crystal/transparent/".into(),
        "ruby-sapphire" | "emerald" | "firered-leafgreen" => format!("versions/generation-iii/{game_id}/"),
        "diamond-pearl" | "platinum" | "heartgold-soulsilver" => format!("versions/generation-iv/{game_id}/"),
        "black-white" | "black-2-white-2" => "versions/generation-v/black-white/".into(),
        "x-y" | "omega-ruby-alpha-sapphire" => format!("versions/generation-vi/{game_id}/"),
        _ => "other/home/".into(),
    }
}

impl Sprites {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let dir = base.join("pkmn-lab").join("sprites");
        let _ = std::fs::create_dir_all(&dir);
        let known: Arc<Mutex<HashMap<String, Option<String>>>> = Arc::default();
        let (queue, rx) = channel::<(String, Vec<String>)>();
        let (k, d) = (known.clone(), dir.clone());
        // One worker, one request at a time: polite to the hosts, and sprites appear as they arrive.
        std::thread::spawn(move || {
            for (key, urls) in rx {
                let path = d.join(format!("{key}.png"));
                let mut all_missing = true;
                for url in urls {
                    match crate::update::curl(&[&url]) {
                        Ok(bytes) if bytes.starts_with(b"\x89PNG") => {
                            let tmp = path.with_extension("tmp");
                            if std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &path)).is_ok() {
                                k.lock().unwrap().insert(key.clone(), Some(uri(&path)));
                                ctx.request_repaint();
                            }
                            all_missing = false;
                            break;
                        }
                        Err(e) if e.contains("404") => continue,
                        _ => { all_missing = false; break; } // offline or odd reply: try again next start
                    }
                }
                if all_missing { let _ = std::fs::write(path.with_extension("missing"), b""); }
            }
        });
        Sprites { dir, known, queue }
    }

    fn get(&self, key: String, urls: impl FnOnce() -> Vec<String>) -> Option<String> {
        let mut known = self.known.lock().unwrap();
        if let Some(v) = known.get(&key) { return v.clone(); }
        let path = self.dir.join(format!("{key}.png"));
        let found = path.exists().then(|| uri(&path));
        if found.is_none() && !path.with_extension("missing").exists() {
            let _ = self.queue.send((key.clone(), urls()));
        }
        known.insert(key, found.clone());
        found
    }

    /// Front sprite in the style of the given game; falls back to the default sprite and official artwork.
    pub fn pokemon(&self, species: u16, game_id: &str) -> Option<String> {
        if species == 0 { return None; }
        let f = folder(game_id);
        let key = format!("{}-{species}", f.trim_end_matches('/').replace('/', "-"));
        self.get(key, || vec![
            format!("{POKEAPI}{f}{species}.png"),
            format!("{POKEAPI}{species}.png"),
            format!("{POKEAPI}other/official-artwork/{species}.png"),
        ])
    }

    /// Trainer portrait from Showdown ("Lt. Surge" -> "ltsurge"), preferring the art of the game's generation.
    pub fn trainer(&self, name_en: &str, game_id: &str) -> Option<String> {
        let name: String = name_en.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if name.is_empty() { return None; }
        let gen_tag = match game_id {
            "red-blue" | "yellow" => "-gen1",
            "gold-silver" | "crystal" => "-gen2",
            "ruby-sapphire" | "emerald" | "firered-leafgreen" => "-gen3",
            // Showdown's plain files are the Gen 4 DS art (HGSS, DPPt), so those games need no suffix.
            _ => "",
        };
        self.get(format!("trainer-{name}{gen_tag}"), || {
            let mut v = vec![];
            if !gen_tag.is_empty() { v.push(format!("{SHOWDOWN}{name}{gen_tag}.png")); }
            v.push(format!("{SHOWDOWN}{name}.png"));
            v
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_png_loads() {
        let dir = std::env::var("LOCALAPPDATA").unwrap() + "/pkmn-lab/sprites/versions-generation-iv-heartgold-soulsilver-74.png";
        let ctx = eframe::egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let uri = super::uri(std::path::Path::new(&dir));
        for _ in 0..50 {
            match ctx.try_load_image(&uri, Default::default()) {
                Ok(eframe::egui::load::ImagePoll::Ready { .. }) => return,
                Ok(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
                Err(e) => panic!("{e}"),
            }
        }
        panic!("timeout");
    }
}
