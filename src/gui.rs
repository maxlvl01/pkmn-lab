//! The app window (egui). Everything the user sees lives here; maths is in analysis.rs.
use crate::analysis::{self, Game, Mon, Report, Source, Verdict};
use crate::data::{Boss, Dex, EvoHow, Lang, Learnsets};
use crate::sprites::Sprites;
use crate::{rom, save, update};
use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Ui, Vec2};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq)]
enum Tab { Team, Defense, Offense, Projection, Catch, Bosses }

/// What survives a restart.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Saved {
    english: bool,
    save_path: Option<String>,
    rom_path: Option<String>,
    game_id: String,
    badges: Vec<bool>,
    team: Vec<Mon>,
}

struct RomData {
    bosses: Vec<Boss>,
    level_up: HashMap<u16, Vec<(u8, u16)>>,
    evolutions: HashMap<u16, Vec<(u16, EvoHow)>>,
}

pub struct App {
    dex: Dex,
    lang: Lang,
    save_path: Option<String>,
    rom_path: Option<String>,
    trainer: Option<String>,
    game_id: String,
    badges: Vec<bool>,
    team: Vec<Mon>,
    /// TM/HM numbers from the save's bag; None = unknown (manual team)
    owned_tms: Option<Vec<u16>>,
    rom: Option<RomData>,
    /// Learnsets and bosses of the current game (rebuilt when game or ROM changes)
    learn: Learnsets,
    bosses: Vec<Boss>,
    data_key: String,
    report: Option<Report>,
    dirty: bool,
    tab: Tab,
    /// boss the projection is made for; None = the next one
    proj_focus: Option<usize>,
    message: Option<(bool, String)>, // (is_error, text)
    search: Vec<String>,
    update_status: Arc<Mutex<Option<String>>>,
    sprites: Sprites,
}

fn t(lang: Lang, en: &'static str, de: &'static str) -> &'static str {
    if lang == Lang::De { de } else { en }
}

// ---------------- colours & small widgets ----------------

const BG: Color32 = Color32::from_rgb(0x16, 0x1a, 0x24);
const CARD: Color32 = Color32::from_rgb(0x21, 0x27, 0x35);
const CARD_HI: Color32 = Color32::from_rgb(0x2a, 0x32, 0x44);
const ACCENT: Color32 = Color32::from_rgb(0xe8, 0x4a, 0x4a);
const MUTED: Color32 = Color32::from_rgb(0x9a, 0xa3, 0xb5);
const GOOD: Color32 = Color32::from_rgb(0x4c, 0xc2, 0x7a);
const OKAY: Color32 = Color32::from_rgb(0xf0, 0xb9, 0x3a);
const BAD: Color32 = Color32::from_rgb(0xe8, 0x5a, 0x5a);

fn type_color(t: u8) -> Color32 {
    let hex = match t {
        1 => 0xA8A77A, 2 => 0xC22E28, 3 => 0xA98FF3, 4 => 0xA33EA1, 5 => 0xE2BF65, 6 => 0xB6A136,
        7 => 0xA6B91A, 8 => 0x735797, 9 => 0xB7B7CE, 10 => 0xEE8130, 11 => 0x6390F0, 12 => 0x7AC74C,
        13 => 0xF7D02C, 14 => 0xF95587, 15 => 0x96D9D6, 16 => 0x6F35FC, 17 => 0x705746, 18 => 0xD685AD,
        _ => 0x68A090,
    };
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn on_color(c: Color32) -> Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 165.0 { Color32::from_rgb(0x1a, 0x1a, 0x1a) } else { Color32::WHITE }
}

fn pill(ui: &mut Ui, text: &str, fill: Color32) {
    egui::Frame::new().fill(fill).corner_radius(CornerRadius::same(9)).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
        ui.label(RichText::new(text).color(on_color(fill)).strong().size(12.0));
    });
}

fn mult_text(m: f32) -> String {
    match m {
        x if x == 0.0 => "0".into(),
        x if x == 0.25 => "¼".into(),
        x if x == 0.5 => "½".into(),
        x if x == 1.0 => "·".into(),
        x => format!("{x}"),
    }
}

fn mult_color(m: f32, attacking: bool) -> Color32 {
    // attacking: high is good for us; defending: high is bad for us
    let good = if attacking { m >= 2.0 } else { m < 1.0 };
    let bad = if attacking { m < 1.0 } else { m > 1.0 };
    match () {
        _ if m == 1.0 => CARD_HI,
        _ if good && (m >= 4.0 || m == 0.0 || m == 0.25) => Color32::from_rgb(0x2f, 0x9e, 0x5c),
        _ if good => Color32::from_rgb(0x2d, 0x6e, 0x4a),
        _ if bad && (m >= 4.0 || m == 0.0) => Color32::from_rgb(0xc0, 0x39, 0x39),
        _ if bad => Color32::from_rgb(0x8a, 0x33, 0x33),
        _ => CARD_HI,
    }
}

fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new().fill(CARD).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(14)).show(ui, |ui| ui.vertical(add).inner).inner
}

fn bar_color(pct: u32) -> Color32 {
    if pct >= 70 { Color32::from_rgb(0x2f, 0x9e, 0x5c) } else if pct >= 50 { Color32::from_rgb(0xb8, 0x8a, 0x1e) } else { Color32::from_rgb(0xb0, 0x3a, 0x3a) }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(18.0).strong().color(Color32::WHITE));
}

/// Round score badge: a ring filled to the rating, coloured red/yellow/green.
fn rating_ring(ui: &mut Ui, rating: u32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = size / 2.0 - 5.0;
    p.circle_stroke(c, r, Stroke::new(8.0, CARD_HI));
    let col = if rating >= 70 { GOOD } else if rating >= 50 { OKAY } else { BAD };
    let n = 64;
    let end = (rating.min(100) as f32 / 100.0) * std::f32::consts::TAU;
    let pts: Vec<egui::Pos2> = (0..=n).map(|i| {
        let a = -std::f32::consts::FRAC_PI_2 + end * i as f32 / n as f32;
        c + Vec2::new(a.cos(), a.sin()) * r
    }).collect();
    p.add(egui::Shape::line(pts, Stroke::new(8.0, col)));
    p.text(c, egui::Align2::CENTER_CENTER, rating.to_string(), egui::FontId::proportional(size * 0.32), Color32::WHITE);
}

// ---------------- app ----------------

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = CARD;
        style.visuals.extreme_bg_color = Color32::from_rgb(0x10, 0x13, 0x1b);
        style.visuals.selection.bg_fill = ACCENT;
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        cc.egui_ctx.set_global_style(style);

        let saved: Saved = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        let mut app = App {
            dex: Dex::load(),
            lang: if saved.english { Lang::En } else { Lang::De },
            save_path: None,
            rom_path: None,
            trainer: None,
            game_id: if saved.game_id.is_empty() { "heartgold-soulsilver".into() } else { saved.game_id },
            badges: saved.badges,
            team: saved.team,
            owned_tms: None,
            rom: None,
            learn: Learnsets::default(),
            bosses: vec![],
            data_key: String::new(),
            report: None,
            dirty: true,
            tab: Tab::Team,
            proj_focus: None,
            message: None,
            search: vec![],
            update_status: Arc::new(Mutex::new(None)),
            sprites: Sprites::new(cc.egui_ctx.clone()),
        };
        // Reopen last session's files: the save is re-read so the team is always current.
        if let Some(p) = saved.rom_path { app.load_rom(&p); }
        if let Some(p) = saved.save_path { app.load_save(&p); }
        app.message = None;
        // "Open with" / drag onto the exe: files on the command line win over the remembered ones.
        for p in std::env::args().skip(1) {
            if p.to_lowercase().ends_with(".nds") { app.load_rom(&p) } else { app.load_save(&p) }
        }
        app.start_update_check(false);
        app
    }

    fn start_update_check(&self, force: bool) {
        let status = self.update_status.clone();
        let de = self.lang == Lang::De;
        if force {
            *status.lock().unwrap() = Some(if de { "Suche nach Updates …" } else { "Checking for updates …" }.into());
        }
        std::thread::spawn(move || {
            let msg = match update::check(force) {
                Ok(update::Outcome::Installed(v)) => Some(if de {
                    format!("Version {v} installiert, aktiv beim nächsten Start")
                } else {
                    format!("Version {v} installed, active after a restart")
                }),
                Ok(update::Outcome::UpToDate) if force => Some(if de { "Du hast die neueste Version".into() } else { "You're up to date".into() }),
                Ok(_) => None,
                Err(e) if force => Some(e),
                Err(_) => None, // offline etc.: a background check stays quiet
            };
            *status.lock().unwrap() = msg;
        });
    }

    fn load_save(&mut self, path: &str) {
        let result = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| save::read(&b, &self.dex));
        match result {
            Ok(info) => {
                if let Some(g) = info.game.filter(|g| self.dex.vg(g).is_some()) { self.game_id = g; }
                self.badges = info.badges;
                self.team = info.team;
                self.trainer = info.trainer;
                self.owned_tms = info.machines;
                self.save_path = Some(path.into());
                self.search.clear();
                self.dirty = true;
                self.tab = Tab::Team;
                self.message = Some((false, format!("{} {}", t(self.lang, "Loaded", "Geladen:"), file_name(path))));
            }
            Err(e) => self.message = Some((true, e)),
        }
    }

    fn load_rom(&mut self, path: &str) {
        let result = std::fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| {
            let game = "heartgold-soulsilver";
            let mut bosses = self.dex.bosses.iter().find(|g| g.id == game).map(|g| g.bosses.clone()).unwrap_or_default();
            rom::apply_rom_teams(&bytes, game, &mut bosses, &self.dex)?;
            Ok(RomData { bosses, level_up: rom::hgss_learnsets(&bytes).unwrap_or_default(), evolutions: rom::hgss_evolutions(&bytes).unwrap_or_default() })
        });
        match result {
            Ok(r) => {
                self.rom = Some(r);
                self.rom_path = Some(path.into());
                self.dirty = true;
                self.message = Some((false, format!("{} {}", t(self.lang, "ROM loaded:", "ROM geladen:"), file_name(path))));
            }
            Err(e) => self.message = Some((true, e)),
        }
    }

    /// Bosses and learnsets for the current game; a loaded HGSS ROM overrides the built-in data.
    fn ensure_game_data(&mut self) {
        let key = format!("{}|{}", self.game_id, self.rom_path.as_deref().unwrap_or(""));
        if key == self.data_key { return; }
        let vg = self.dex.vg(&self.game_id).expect("valid game id");
        self.learn = self.dex.learnsets(vg);
        self.bosses = self.dex.bosses.iter().find(|g| g.id == self.game_id).map(|g| g.bosses.clone()).unwrap_or_default();
        if let (true, Some(r)) = (self.game_id == "heartgold-soulsilver", &self.rom) {
            self.bosses = r.bosses.clone();
            if !r.level_up.is_empty() { self.learn.level_up = r.level_up.clone(); }
            if !r.evolutions.is_empty() { self.learn.evolutions = r.evolutions.clone(); }
        }
        self.data_key = key;
    }

    fn recompute(&mut self) {
        self.dirty = false;
        self.ensure_game_data();
        self.team.retain(|m| m.species != 0);
        if self.team.is_empty() { self.report = None; return; }
        let vg = self.dex.vg(&self.game_id).unwrap();
        let owned: Option<HashSet<u16>> = self.owned_tms.as_ref().map(|ns| {
            self.learn.machine_no.iter().filter(|(_, n)| ns.contains(n)).map(|(&mv, _)| mv).collect()
        });
        let game = Game { vg, bosses: &self.bosses, badges: &self.badges, learn: &self.learn, owned_machines: owned.as_ref() };
        self.report = Some(analysis::analyse(&self.dex, &game, &self.team, self.proj_focus));
    }
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or(path.into())
}

/// A small Pokéball drawn with shapes: the placeholder while a sprite loads, and the app's logo.
fn pokeball(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = size * 0.42;
    let dark = Color32::from_rgb(0x22, 0x22, 0x22);
    p.circle_filled(c, r, Color32::from_rgb(0xee, 0xee, 0xee));
    let n = 24;
    let top: Vec<egui::Pos2> = (0..=n).map(|i| {
        let a = std::f32::consts::PI + std::f32::consts::PI * i as f32 / n as f32;
        c + Vec2::new(a.cos(), a.sin()) * r
    }).collect();
    p.add(egui::Shape::convex_polygon(top, ACCENT, Stroke::NONE));
    p.line_segment([c + Vec2::new(-r, 0.0), c + Vec2::new(r, 0.0)], Stroke::new(size * 0.06, dark));
    p.circle_stroke(c, r, Stroke::new(size * 0.05, dark));
    p.circle_filled(c, r * 0.28, dark);
    p.circle_filled(c, r * 0.17, Color32::WHITE);
}

/// Sprite (kept in its aspect ratio, pixel-sharp) or a Pokéball while it downloads.
fn sprite(ui: &mut Ui, uri: Option<String>, size: f32) {
    match uri {
        Some(u) => {
            ui.add(egui::Image::new(u).max_size(Vec2::splat(size)).fit_to_exact_size(Vec2::splat(size))
                .maintain_aspect_ratio(true).texture_options(egui::TextureOptions::NEAREST));
        }
        None => pokeball(ui, size),
    }
}

fn machine_label(n: u16, lang: Lang) -> String {
    // PokeAPI numbers HMs after the TMs (101 = HM01).
    if n > 100 { format!("{}{:02}", lang.pick("HM", "VM"), n - 100) } else { format!("TM{n:02}") }
}

fn mult_label(m: f32) -> String {
    format!("{}x", mult_text(m).replace('·', "1"))
}

impl App {
    fn type_badge(&self, ui: &mut Ui, ty: u8) {
        pill(ui, self.dex.type_name(ty, self.lang), type_color(ty));
    }

    fn types_row(&self, ui: &mut Ui, types: &[u8]) {
        ui.horizontal(|ui| { for &x in types { self.type_badge(ui, x); } });
    }

    fn species_sprite(&self, ui: &mut Ui, species: u16, size: f32) {
        sprite(ui, self.sprites.pokemon(species, &self.game_id), size);
    }

    fn mname(&self, id: u16) -> String { self.dex.move_name(id, self.lang) }
    fn sname(&self, id: u16) -> String { self.dex.species_name(id, self.lang) }

    fn open_save_dialog(&mut self) {
        let l = self.lang;
        if let Some(p) = rfd::FileDialog::new()
            .add_filter(t(l, "Save file", "Spielstand"), &["sav", "dsv", "srm", "sa1", "sa2", "dat", "main", "bin"])
            .add_filter(t(l, "All files", "Alle Dateien"), &["*"])
            .pick_file()
        {
            self.load_save(&p.to_string_lossy());
        }
    }

    // ---------------- side panel: inputs ----------------

    fn side_panel(&mut self, ui: &mut Ui) {
        let l = self.lang;
        egui::ScrollArea::vertical().show(ui, |ui| {
            heading(ui, t(l, "Save file", "Spielstand"));
            if ui.add_sized([ui.available_width(), 32.0], egui::Button::new(t(l, "Open save file …", "Spielstand öffnen …"))).clicked() {
                self.open_save_dialog();
            }
            if let Some(p) = &self.save_path { ui.label(RichText::new(file_name(p)).color(MUTED).small()); }
            ui.label(RichText::new(t(l, "or drag it into the window", "oder einfach ins Fenster ziehen")).color(MUTED).small());
            ui.add_space(4.0);
            if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(t(l, "Open ROM … (optional)", "ROM öffnen … (optional)"))).clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("NDS ROM", &["nds"]).pick_file() {
                    self.load_rom(&p.to_string_lossy());
                }
            }
            let rom_note = match &self.rom_path {
                Some(p) => file_name(p),
                None => t(l, "HeartGold/SoulSilver: real boss teams and level-up moves, also for ROM hacks",
                    "HeartGold/SoulSilver: echte Boss-Teams und Level-Attacken, auch bei ROM-Hacks").into(),
            };
            ui.label(RichText::new(rom_note).color(MUTED).small());

            ui.add_space(10.0);
            ui.separator();
            heading(ui, t(l, "Game", "Spiel"));
            let current = self.dex.vg(&self.game_id).map(|v| l.pick(&v.name_en, &v.name_de).to_string()).unwrap_or_default();
            let mut chosen = self.game_id.clone();
            egui::ComboBox::from_id_salt("game").width(ui.available_width()).selected_text(current).show_ui(ui, |ui| {
                for g in &self.dex.bosses {
                    if let Some(v) = self.dex.vg(&g.id) {
                        ui.selectable_value(&mut chosen, g.id.clone(), format!("{}  (Gen {})", l.pick(&v.name_en, &v.name_de), v.gn));
                    }
                }
            });
            if chosen != self.game_id {
                self.game_id = chosen;
                self.badges.clear();
                self.owned_tms = None;
                self.dirty = true;
            }
            let gyms = self.dex.bosses.iter().find(|g| g.id == self.game_id)
                .map(|g| g.bosses.iter().filter(|b| b.kind == "gym").count()).unwrap_or(8);
            let mut count = self.badges.iter().filter(|&&b| b).count();
            ui.horizontal(|ui| {
                ui.label(t(l, "Badges", "Orden"));
                if ui.add(egui::DragValue::new(&mut count).range(0..=gyms)).changed() {
                    self.badges = (0..gyms).map(|i| i < count).collect();
                    self.dirty = true;
                }
                ui.label(RichText::new(format!("/ {gyms}")).color(MUTED));
            });

            ui.add_space(10.0);
            ui.separator();
            heading(ui, &format!("Team ({}/6)", self.team.len()));
            ui.label(RichText::new(t(l, "Change anything to see how the analysis reacts.", "Ändere etwas und sieh, wie sich die Analyse ändert."))
                .color(MUTED).small());
            self.team_editor(ui);
        });
    }

    fn team_editor(&mut self, ui: &mut Ui) {
        let l = self.lang;
        if self.search.len() != self.team.len() {
            self.search = self.team.iter().map(|m| if m.species == 0 { String::new() } else { self.sname(m.species) }).collect();
        }
        let gn = self.dex.vg(&self.game_id).map(|v| v.gn).unwrap_or(9);
        let mut remove = None;
        for i in 0..self.team.len() {
            egui::Frame::new().fill(CARD).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.species_sprite(ui, self.team[i].species, 48.0);
                    ui.vertical(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.search[i]).hint_text(t(l, "Pokémon name", "Pokémon-Name")).desired_width(150.0));
                        ui.horizontal(|ui| {
                            ui.label("Lv");
                            if ui.add(egui::DragValue::new(&mut self.team[i].level).range(1..=100)).changed() { self.dirty = true; }
                            if ui.small_button(t(l, "Remove", "Entfernen")).clicked() { remove = Some(i); }
                        });
                    });
                });
                // Name suggestions while typing.
                let q = crate::data::norm(&self.search[i]);
                let current = if self.team[i].species == 0 { String::new() } else { crate::data::norm(&self.sname(self.team[i].species)) };
                if !q.is_empty() && q != current {
                    let mut hits: Vec<(u16, String)> = self.dex.species.values()
                        .filter(|s| s.gn <= gn)
                        .map(|s| (s.id, l.pick(&s.name_en, &s.name_de).to_string()))
                        .filter(|(_, n)| crate::data::norm(n).starts_with(&q))
                        .collect();
                    hits.sort_by(|a, b| a.1.cmp(&b.1));
                    for (id, name) in hits.into_iter().take(6) {
                        ui.horizontal(|ui| {
                            self.species_sprite(ui, id, 28.0);
                            if ui.selectable_label(false, &name).clicked() {
                                self.team[i] = Mon { species: id, level: self.team[i].level, moves: vec![], nickname: None };
                                self.search[i] = name.clone();
                                self.dirty = true;
                            }
                        });
                    }
                }
                if self.team[i].species != 0 { self.move_pickers(ui, i); }
            });
            ui.add_space(4.0);
        }
        if let Some(i) = remove {
            self.team.remove(i);
            self.search.remove(i);
            self.dirty = true;
        }
        if self.team.len() < 6
            && ui.add_sized([ui.available_width(), 28.0], egui::Button::new(t(l, "+ Add Pokémon", "+ Pokémon hinzufügen"))).clicked()
        {
            self.team.push(Mon { species: 0, level: 5, moves: vec![], nickname: None });
            self.search.push(String::new());
        }
    }

    /// Four move menus offering only what this Pokémon can learn in the chosen game.
    fn move_pickers(&mut self, ui: &mut Ui, i: usize) {
        let sp = self.team[i].species;
        let mut options: Vec<u16> = self.learn.level_up.get(&sp).into_iter().flatten().map(|x| x.1)
            .chain(self.learn.machine.get(&sp).into_iter().flatten().copied())
            .chain(self.team[i].moves.iter().copied())
            .collect();
        options.sort();
        options.dedup();
        let mut named: Vec<(u16, String)> = options.into_iter().map(|id| (id, self.mname(id))).collect();
        named.sort_by(|a, b| a.1.cmp(&b.1));
        let mut moves = self.team[i].moves.clone();
        moves.resize(4, 0);
        egui::Grid::new(("moves", i)).num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
            for (k, slot) in moves.iter_mut().enumerate() {
                let label = if *slot == 0 { "–".to_string() } else { self.mname(*slot) };
                egui::ComboBox::from_id_salt(("move", i, k)).width(118.0).selected_text(label).show_ui(ui, |ui| {
                    ui.selectable_value(slot, 0, "–");
                    for (id, name) in &named { ui.selectable_value(slot, *id, name); }
                });
                if k % 2 == 1 { ui.end_row(); }
            }
        });
        let moves: Vec<u16> = moves.into_iter().filter(|&m| m != 0).collect();
        if moves != self.team[i].moves {
            self.team[i].moves = moves;
            self.dirty = true;
        }
    }

    // ---------------- central panel: results ----------------

    fn welcome(&mut self, ui: &mut Ui) {
        let l = self.lang;
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.16);
            pokeball(ui, 110.0);
            ui.add_space(12.0);
            ui.label(RichText::new("PKMN Lab").size(34.0).strong().color(Color32::WHITE));
            ui.label(RichText::new(t(l, "Analyse your team, plan the next boss.", "Analysiere dein Team, plane den nächsten Boss.")).size(16.0).color(MUTED));
            ui.add_space(24.0);
            ui.label(RichText::new(t(l, "Drag your save file (.sav / .dsv) into this window", "Zieh deinen Spielstand (.sav / .dsv) in dieses Fenster")).size(15.0));
            ui.add_space(10.0);
            if ui.add_sized([220.0, 36.0], egui::Button::new(t(l, "Open save file", "Spielstand öffnen"))).clicked() {
                self.open_save_dialog();
            }
            if ui.add_sized([220.0, 36.0], egui::Button::new(t(l, "Enter team by hand", "Team von Hand eingeben"))).clicked() {
                self.team.push(Mon { species: 0, level: 5, moves: vec![], nickname: None });
            }
        });
    }

    fn header(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                rating_ring(ui, r.summary.rating, 92.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(t(l, "Team rating", "Team-Bewertung")).size(18.0).strong());
                    ui.label(RichText::new(format!("{} {}/100", t(l, "with projected attacks:", "mit projizierten Attacken:"), r.projection.rating_after)).color(MUTED));
                    ui.add(egui::ProgressBar::new(r.summary.offense_pct as f32 / 100.0).desired_width(230.0).fill(bar_color(r.summary.offense_pct))
                        .text(format!("{} {}%", t(l, "Offensive coverage", "Angriffs-Abdeckung"), r.summary.offense_pct)));
                    ui.add(egui::ProgressBar::new(r.summary.defense_pct as f32 / 100.0).desired_width(230.0).fill(bar_color(r.summary.defense_pct))
                        .text(format!("{} {}%", t(l, "Types handled", "Typen im Griff"), r.summary.defense_pct)));
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    if let Some(w) = r.worst {
                        ui.horizontal(|ui| { ui.label(RichText::new(t(l, "Biggest weakness", "Größte Schwäche")).color(MUTED)); self.type_badge(ui, w); });
                    }
                    if let Some(w) = r.worst_uncountered {
                        ui.horizontal(|ui| { ui.label(RichText::new(t(l, "Nobody answers", "Ungekontert")).color(MUTED)); self.type_badge(ui, w); });
                    }
                    let mut stomps: Vec<&analysis::OffRow> = r.offense.iter().filter(|o| o.hitters >= 2).collect();
                    stomps.sort_by_key(|o| std::cmp::Reverse(o.hitters));
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(t(l, "Dominates", "Dominiert")).color(MUTED));
                        if stomps.is_empty() { ui.label(RichText::new(t(l, "nothing yet", "noch nichts")).color(MUTED)); }
                        for o in stomps.iter().take(4) { self.type_badge(ui, o.def); }
                    });
                    if let Some(bv) = r.bosses.first() {
                        let b = &self.bosses[bv.boss];
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t(l, "Next boss", "Nächster Boss")).color(MUTED));
                            ui.label(RichText::new(format!("{} (Lv {})", l.pick(&b.name_en, &b.name_de), bv.their_max)).strong());
                            self.verdict_pill(ui, bv.verdict);
                        });
                    }
                });
            });
        });
    }

    fn verdict_pill(&self, ui: &mut Ui, v: Verdict) {
        let l = self.lang;
        match v {
            Verdict::Dominate => pill(ui, t(l, "You dominate", "Du dominierst"), GOOD),
            Verdict::Even => pill(ui, t(l, "Even fight", "Ausgeglichen"), OKAY),
            Verdict::Struggle => pill(ui, t(l, "Tough", "Wird schwer"), BAD),
        }
    }

    fn tab_team(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        let items: Vec<_> = self.team.iter().zip(&r.members).collect();
        card_rows(ui, &items, 272.0, |ui, &(m, v)| {
                card(ui, |ui| {
                    ui.set_width(272.0);
                    ui.horizontal(|ui| {
                        self.species_sprite(ui, m.species, 84.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(self.sname(m.species)).size(17.0).strong());
                            if let Some(n) = m.nickname.as_ref().filter(|n| crate::data::norm(n) != crate::data::norm(&self.sname(m.species))) {
                                ui.label(RichText::new(format!("\"{n}\"")).color(MUTED));
                            }
                            ui.label(RichText::new(format!("Lv {}   BST {}", m.level, v.bst)).color(MUTED));
                            self.types_row(ui, &v.types);
                        });
                    });
                    ui.add_space(4.0);
                    for &(id, ty, p) in &v.moves {
                        ui.horizontal(|ui| {
                            self.type_badge(ui, ty);
                            ui.label(self.mname(id));
                            ui.label(RichText::new(p.map(|p| p.to_string()).unwrap_or("–".into())).color(MUTED));
                        });
                    }
                    if v.no_damage {
                        ui.colored_label(BAD, t(l, "No damaging moves", "Keine Schadensattacken"));
                    } else if v.no_stab {
                        ui.colored_label(OKAY, t(l, "No attack of its own type (no STAB)", "Keine Attacke vom eigenen Typ (kein STAB)"));
                    }
                });
        });
    }

    fn tab_defense(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        card(ui, |ui| {
            if let Some(w) = r.worst {
                let row = r.defense.iter().find(|d| d.atk == w).unwrap();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(l, "Biggest weakness:", "Größte Schwäche:")).strong());
                    self.type_badge(ui, w);
                    ui.label(format!("{} {}, {} {}", row.weak, t(l, "weak", "schwach"), row.resist, t(l, "resist", "resistent")));
                });
                if row.counters.is_empty() {
                    ui.colored_label(BAD, t(l, "Nobody can hit it back super effectively yet.", "Noch niemand kann sie sehr effektiv zurückschlagen."));
                } else {
                    let who: Vec<String> = row.counters.iter().map(|&(i, mv)| format!("{} ({})", self.sname(self.team[i].species), self.mname(mv))).collect();
                    ui.colored_label(GOOD, format!("{}: {}", t(l, "Hits it back super effectively", "Schlägt sehr effektiv zurück"), who.join(", ")));
                }
            }
            if let Some(w) = r.worst_uncountered {
                ui.horizontal(|ui| { ui.label(RichText::new(t(l, "Biggest weakness nobody answers:", "Größte ungekonterte Schwäche:")).strong()); self.type_badge(ui, w); });
            }
            if !r.also_countered.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(l, "Also weak to, but countered:", "Auch anfällig, aber gekontert:"));
                    for &x in &r.also_countered { self.type_badge(ui, x); }
                });
            }
        });
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.label(RichText::new(t(l, "How each attack type hits your team", "Wie jeder Angriffstyp dein Team trifft")).strong());
            egui::Grid::new("def").spacing([6.0, 4.0]).show(ui, |ui| {
                ui.label("");
                for m in &self.team { self.species_sprite(ui, m.species, 40.0); }
                ui.label(RichText::new(t(l, "weak", "schwach")).color(MUTED));
                ui.label(RichText::new(t(l, "resist", "resist.")).color(MUTED));
                ui.label(RichText::new(t(l, "counter", "Konter")).color(MUTED));
                ui.end_row();
                for d in &r.defense {
                    self.type_badge(ui, d.atk);
                    for &m in &d.mults { mult_cell(ui, m, false, 40.0); }
                    ui.label(RichText::new(d.weak.to_string()).color(if d.problem { BAD } else { Color32::WHITE }));
                    ui.label(d.resist.to_string());
                    ui.label(RichText::new(d.counters.len().to_string()).color(if d.counters.is_empty() { MUTED } else { GOOD }));
                    ui.end_row();
                }
            });
        });
    }

    fn tab_offense(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        card(ui, |ui| {
            ui.label(RichText::new(t(l, "Your best hit against each type", "Dein bester Treffer gegen jeden Typ")).strong());
            egui::Grid::new("off").spacing([10.0, 4.0]).show(ui, |ui| {
                for o in &r.offense {
                    self.type_badge(ui, o.def);
                    mult_cell(ui, o.best, true, 48.0);
                    let frac = o.hitters as f32 / self.team.len().max(1) as f32;
                    ui.add(egui::ProgressBar::new(frac).desired_width(240.0).fill(Color32::from_rgb(0x3d, 0x7b, 0xd8))
                        .text(format!("{} {}", o.hitters, t(l, "hit it super effectively", "treffen sehr effektiv"))));
                    ui.end_row();
                }
            });
        });
    }

    fn tab_projection(&mut self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        let p = &r.projection;
        let vg = self.dex.vg(&self.game_id).unwrap();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} {}", t(l, "Your team at level", "Dein Team auf Level"), p.target)).size(17.0).strong());
                ui.label(RichText::new(t(l, "for", "für")).size(17.0));
                let name = |b: usize| l.pick(&self.bosses[b].name_en, &self.bosses[b].name_de).to_string();
                let current = p.for_boss.map(name).unwrap_or_default();
                let mut pick = p.for_boss;
                egui::ComboBox::from_id_salt("proj_boss").selected_text(current).show_ui(ui, |ui| {
                    for bv in &r.bosses {
                        ui.selectable_value(&mut pick, Some(bv.boss), format!("{}  (Lv {})", name(bv.boss), bv.their_max));
                    }
                });
                if pick != p.for_boss {
                    self.proj_focus = pick;
                    self.dirty = true;
                }
            });
            ui.label(RichText::new(t(l,
                "Each member raised to the boss's top level: evolutions by level happen, then the best 4 attacks for this fight and the two after it.",
                "Jedes Mitglied auf das höchste Level des Bosses gebracht: Level-Entwicklungen passieren, dann die besten 4 Attacken für diesen Kampf und die zwei danach.")).color(MUTED));
            ui.label(RichText::new(match &self.owned_tms {
                None => t(l, "Your TMs are unknown, so sets use level-up moves; TM ideas are shown separately.",
                    "Deine TMs sind unbekannt, daher nur Level-Attacken im Set; TM-Ideen stehen extra."),
                Some(_) => t(l, "TMs/HMs in your bag are included; TM ideas are ones you don't have yet.",
                    "TMs/VMs aus deinem Beutel zählen mit; TM-Ideen sind welche, die du noch nicht hast."),
            }).color(MUTED));
            ui.label(format!("{}: {} » {}/100", t(l, "Team rating", "Team-Bewertung"), r.summary.rating, p.rating_after));
        });
        ui.add_space(8.0);
        let items: Vec<_> = self.team.iter().zip(&p.members).collect();
        card_rows(ui, &items, 300.0, |ui, &(m, pm)| {
            let g = &pm.growth;
            card(ui, |ui| {
                ui.set_width(300.0);
                ui.horizontal(|ui| {
                    self.species_sprite(ui, g.species, 72.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.sname(g.species)).size(16.0).strong());
                        ui.label(RichText::new(format!("Lv {} » {}", m.level, g.level)).color(MUTED));
                        let types = self.dex.species.get(&g.species).map(|s| self.dex.species_types(s, vg.gn)).unwrap_or_default();
                        self.types_row(ui, &types);
                    });
                });
                for &(to, at) in &g.evolved {
                    ui.colored_label(GOOD, format!("{} {} {} Lv {at}", t(l, "Evolves into", "Entwickelt sich zu"), self.sname(to), t(l, "at", "bei")));
                }
                for &(to, how) in &g.could {
                    let how = match how {
                        EvoHow::Trade => t(l, "by trade", "durch Tausch"),
                        EvoHow::Item => t(l, "with an item", "mit Item"),
                        EvoHow::Friendship => t(l, "with friendship", "durch Freundschaft"),
                        _ => t(l, "special condition", "besondere Bedingung"),
                    };
                    ui.colored_label(OKAY, format!("{} {} ({how})", t(l, "Could become", "Kann werden:"), self.sname(to)));
                }
                stat_bars(ui, &g.stats, l);
                if !pm.learns.is_empty() {
                    ui.label(RichText::new(t(l, "Learns on the way", "Lernt bis dahin")).color(MUTED));
                    ui.horizontal_wrapped(|ui| {
                        for &(lv, id) in &pm.learns { pill(ui, &format!("Lv {lv} {}", self.mname(id)), CARD_HI); }
                    });
                }
                ui.label(RichText::new(t(l, "Best attacks", "Beste Attacken")).color(MUTED));
                if pm.picks.is_empty() { ui.label(t(l, "none found", "keine gefunden")); }
                for (id, src) in &pm.picks {
                    let (ty, pw) = self.dex.move_stats(&self.dex.moves[id], vg);
                    ui.horizontal(|ui| {
                        self.type_badge(ui, ty);
                        ui.label(self.mname(*id));
                        ui.label(RichText::new(pw.map(|p| p.to_string()).unwrap_or_default()).color(MUTED));
                        match src {
                            Source::Known => {}
                            Source::Level(lv) => pill(ui, &format!("{} Lv {lv}", t(l, "new", "neu")), Color32::from_rgb(0x3d, 0x7b, 0xd8)),
                            Source::Machine(n) => pill(ui, &machine_label(*n, l), Color32::from_rgb(0x8a, 0x5c, 0xd6)),
                        }
                    });
                }
                if !pm.tm_ideas.is_empty() {
                    ui.label(RichText::new(t(l, "TM ideas", "TM-Ideen")).color(MUTED));
                    ui.horizontal_wrapped(|ui| {
                        for &(n, id) in &pm.tm_ideas { pill(ui, &format!("{} {}", machine_label(n, l), self.mname(id)), CARD_HI); }
                    });
                }
                if !pm.keep_status.is_empty() {
                    let names: Vec<String> = pm.keep_status.iter().map(|&id| self.mname(id)).collect();
                    ui.label(RichText::new(format!("{}: {}", t(l, "Free slots, e.g. keep", "Freie Plätze, z.B. behalten"), names.join(", "))).color(MUTED).small());
                }
            });
        });
    }

    fn tab_catch(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        if r.catch.is_empty() {
            card(ui, |ui| ui.label(t(l, "No single typing improves the team; work on moves and levels.", "Kein Typ verbessert das Team; arbeite an Attacken und Leveln.")));
            return;
        }
        ui.label(RichText::new(t(l, "Examples come from the whole Pokédex up to this generation; check what you can catch in your game.",
            "Beispiele aus dem ganzen Pokédex bis zu dieser Generation; prüf, was in deinem Spiel fangbar ist.")).color(MUTED));
        for c in &r.catch {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("+{}", c.gain)).size(26.0).strong().color(GOOD));
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        self.types_row(ui, &c.types);
                        if let Some(i) = c.replacing {
                            ui.label(RichText::new(format!("{} {}", t(l, "replacing", "statt"), self.sname(self.team[i].species))).color(MUTED));
                        }
                    });
                    ui.add_space(12.0);
                    for &id in &c.examples {
                        ui.vertical(|ui| {
                            self.species_sprite(ui, id, 64.0);
                            ui.label(RichText::new(self.sname(id)).small());
                        });
                    }
                });
            });
            ui.add_space(6.0);
        }
    }

    fn tab_bosses(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        if r.bosses.is_empty() {
            card(ui, |ui| ui.label(t(l, "No bosses left (or no boss data for this game).", "Keine Bosse mehr übrig (oder keine Boss-Daten für dieses Spiel).")));
            return;
        }
        let our_max = self.team.iter().map(|m| m.level).max().unwrap_or(1);
        for (k, bv) in r.bosses.iter().enumerate() {
            let b = &self.bosses[bv.boss];
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    sprite(ui, self.sprites.trainer(&b.name_en, &self.game_id), 72.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(l.pick(&b.name_en, &b.name_de)).size(17.0).strong());
                            if let Some(ty) = b.type_.as_deref().and_then(|x| self.dex.type_by_ident(x)) { self.type_badge(ui, ty); }
                            self.verdict_pill(ui, bv.verdict);
                            if k == 0 { pill(ui, t(l, "next", "als Nächstes"), ACCENT); }
                        });
                        let place = l.pick(b.location_en.as_deref().unwrap_or(""), b.location_de.as_deref().unwrap_or(""));
                        let var = b.variant.as_ref().map(|v| format!("  ({v})")).unwrap_or_default();
                        ui.label(RichText::new(format!("{place}{var}")).color(MUTED));
                        let gap = bv.their_max as i32 - our_max as i32;
                        let col = if gap > 3 { BAD } else if gap > 0 { OKAY } else { GOOD };
                        ui.label(RichText::new(format!("{} {}  ·  {} {}", t(l, "Their top level", "Ihr höchstes Level"), bv.their_max, t(l, "yours now", "deins jetzt"), our_max)).color(col));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t(l, "Judged with your team at their level:", "Bewertet mit deinem Team auf ihrem Level:")).color(MUTED).small());
                            for &(sp, _) in &bv.team { self.species_sprite(ui, sp, 32.0); }
                        });
                    });
                });
                egui::CollapsingHeader::new(t(l, "Their team and your answers", "Ihr Team und deine Konter"))
                    .id_salt(("boss", bv.boss)).default_open(k < 2).show(ui, |ui| {
                    egui::Grid::new(("bossgrid", bv.boss)).spacing([14.0, 6.0]).show(ui, |ui| {
                        for bm in &bv.mons {
                            ui.horizontal(|ui| {
                                self.species_sprite(ui, bm.species, 48.0);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(format!("{}  Lv {}", self.sname(bm.species), bm.level)).strong());
                                    self.types_row(ui, &bm.types);
                                });
                            });
                            ui.vertical(|ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                for &mv in &bm.moves { ui.label(RichText::new(self.mname(mv)).small().color(MUTED)); }
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new(t(l, "Your answer", "Dein Konter")).small().color(MUTED));
                                if let Some((i, mult)) = bm.answer {
                                    ui.horizontal(|ui| {
                                        self.species_sprite(ui, bv.team[i].0, 32.0);
                                        ui.label(self.sname(bv.team[i].0));
                                        pill(ui, &mult_label(mult), mult_color(mult, true));
                                    });
                                }
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new(t(l, "Threatens", "Bedroht")).small().color(MUTED));
                                let col = if bm.threatens == 0 { GOOD } else if bm.threatens * 2 < self.team.len() { OKAY } else { BAD };
                                ui.label(RichText::new(format!("{}/{}", bm.threatens, self.team.len())).color(col).strong());
                            });
                            ui.end_row();
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
    }
}

fn mult_cell(ui: &mut Ui, m: f32, attacking: bool, width: f32) {
    egui::Frame::new().fill(mult_color(m, attacking)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(0, 3)).show(ui, |ui| {
        ui.set_width(width);
        ui.vertical_centered(|ui| ui.label(RichText::new(if attacking { mult_label(m) } else { mult_text(m) }).strong().color(Color32::WHITE)));
    });
}

const BAR: Color32 = Color32::from_rgb(0x10, 0x13, 0x1b);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Drag and drop: .nds is a ROM, anything else a save.
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            let p = f.path().to_string_lossy().into_owned();
            if p.to_lowercase().ends_with(".nds") { self.load_rom(&p) } else { self.load_save(&p) }
        }
        if self.dirty { self.recompute(); }
        let l = self.lang;

        egui::Panel::top("top").frame(egui::Frame::new().fill(BAR).inner_margin(egui::Margin::symmetric(14, 8))).show(ui, |ui| {
            ui.horizontal(|ui| {
                pokeball(ui, 30.0);
                ui.label(RichText::new("PKMN Lab").size(22.0).strong().color(Color32::WHITE));
                ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).color(MUTED));
                if let Some(tr) = &self.trainer {
                    ui.add_space(12.0);
                    ui.label(RichText::new(format!("Trainer {tr}")).color(MUTED));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(l == Lang::En, "EN").clicked() { self.lang = Lang::En; }
                    if ui.selectable_label(l == Lang::De, "DE").clicked() { self.lang = Lang::De; }
                    ui.add_space(10.0);
                    if ui.button(t(l, "Check for updates", "Nach Updates suchen")).clicked() { self.start_update_check(true); }
                    if let Some(s) = self.update_status.lock().unwrap().clone() { ui.label(RichText::new(s).color(OKAY)); }
                });
            });
        });

        egui::Panel::bottom("status").frame(egui::Frame::new().fill(BAR).inner_margin(egui::Margin::symmetric(14, 4))).show(ui, |ui| {
            match &self.message {
                Some((true, m)) => { ui.colored_label(BAD, m); }
                Some((false, m)) => { ui.colored_label(MUTED, m); }
                None => {
                    ui.label(RichText::new(t(l, "Sprites: PokeAPI / Showdown, downloaded once and kept on this PC",
                        "Sprites: PokeAPI / Showdown, einmal geladen und auf diesem PC gespeichert")).color(MUTED).small());
                }
            }
        });

        egui::Panel::left("inputs").resizable(true).default_size(340.0)
            .frame(egui::Frame::new().fill(Color32::from_rgb(0x1b, 0x20, 0x2c)).inner_margin(egui::Margin::same(12)))
            .show(ui, |ui| self.side_panel(ui));

        egui::CentralPanel::default_margins().show(ui, |ui| {
            let Some(r) = self.report.take() else {
                self.welcome(ui);
                return;
            };
            self.header(ui, &r);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for (tab, en, de) in [
                    (Tab::Team, "Team", "Team"), (Tab::Defense, "Weaknesses", "Schwächen"), (Tab::Offense, "Attack", "Angriff"),
                    (Tab::Projection, "Projection", "Projektion"), (Tab::Catch, "Catch next", "Fangen"), (Tab::Bosses, "Bosses", "Bosse"),
                ] {
                    let selected = self.tab == tab;
                    let text = RichText::new(t(l, en, de)).size(15.0).color(if selected { Color32::WHITE } else { MUTED });
                    let button = egui::Button::new(text).fill(if selected { ACCENT } else { CARD })
                        .corner_radius(CornerRadius::same(8)).min_size(Vec2::new(96.0, 30.0));
                    if ui.add(button).clicked() { self.tab = tab; }
                }
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Team => self.tab_team(ui, &r),
                Tab::Defense => self.tab_defense(ui, &r),
                Tab::Offense => self.tab_offense(ui, &r),
                Tab::Projection => self.tab_projection(ui, &r),
                Tab::Catch => self.tab_catch(ui, &r),
                Tab::Bosses => self.tab_bosses(ui, &r),
            });
            self.report = Some(r);
        });

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let screen = ctx.content_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop")));
            painter.rect_filled(screen, 0.0, Color32::from_black_alpha(190));
            painter.text(screen.center(), egui::Align2::CENTER_CENTER, t(l, "Drop to load", "Loslassen zum Laden"),
                egui::FontId::proportional(32.0), Color32::WHITE);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let saved = Saved {
            english: self.lang == Lang::En,
            save_path: self.save_path.clone(),
            rom_path: self.rom_path.clone(),
            game_id: self.game_id.clone(),
            badges: self.badges.clone(),
            team: self.team.clone(),
        };
        eframe::set_value(storage, eframe::APP_KEY, &saved);
    }
}

/// Cards of a fixed inner width in as many columns as fit (egui can't wrap a card before it knows its size).
fn card_rows<T>(ui: &mut Ui, items: &[T], width: f32, mut add: impl FnMut(&mut Ui, &T)) {
    let outer = width + 28.0 + 8.0; // card margins + spacing
    let cols = ((ui.available_width() + 8.0) / outer).floor().max(1.0) as usize;
    for row in items.chunks(cols) {
        ui.horizontal_top(|ui| for it in row { add(ui, it); });
        ui.add_space(4.0);
    }
}

/// Six small stat bars (scale: 255 at Lv 100 is the top of the bar range).
fn stat_bars(ui: &mut Ui, stats: &[u16; 6], l: Lang) {
    // shortcut: German short stat names as commonly written; not checked against the ROM's text.
    let names = if l == Lang::De { ["KP", "Angr.", "Vert.", "Sp.-Angr.", "Sp.-Vert.", "Init."] } else { ["HP", "Atk", "Def", "SpA", "SpD", "Spe"] };
    let max = stats.iter().copied().max().unwrap_or(1).max(1) as f32;
    egui::Grid::new(ui.next_auto_id()).spacing([6.0, 2.0]).show(ui, |ui| {
        for (i, &v) in stats.iter().enumerate() {
            ui.label(RichText::new(names[i]).small().color(MUTED));
            ui.label(RichText::new(v.to_string()).small().strong());
            ui.add(egui::ProgressBar::new(v as f32 / max).desired_width(150.0).desired_height(6.0)
                .fill(Color32::from_rgb(0x3d, 0x7b, 0xd8)));
            ui.end_row();
        }
    });
}
