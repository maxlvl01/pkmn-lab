//! The app window (egui). Everything the user sees lives here; maths is in analysis.rs.
use crate::analysis::{self, Game, Mon, Report, Source, Verdict};
use crate::data::{Boss, Daytime, Dex, EvoHow, Lang, Learnsets};
use crate::sprites::Sprites;
use crate::{rom, save, update};
use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Ui, Vec2};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq)]
enum Tab { Team, Stats, Defense, Offense, Projection, Catch, Bosses, Trainers }

/// What survives a restart.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Saved {
    english: bool,
    save_path: Option<String>,
    rom_path: Option<String>,
    game_id: String,
    badges: Vec<bool>,
    team: Vec<Mon>,
    #[serde(default)]
    national_dex: bool,
    /// UI scale; 0 = pick from the monitor size on first start
    #[serde(default)]
    zoom: f32,
}

struct RomData {
    bosses: Vec<Boss>,
    level_up: HashMap<u16, Vec<(u8, u16)>>,
    evolutions: HashMap<u16, Vec<(u16, EvoHow)>>,
    trainers: Vec<Boss>,
    wild: HashMap<u16, Vec<crate::data::WildSpot>>,
    places: Vec<String>,
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
    /// all other trainers of the game (from the ROM)
    trainers: Vec<Boss>,
    /// trainer ids the save marks as beaten
    beaten: Option<HashSet<u16>>,
    show_beaten_bosses: bool,
    trainer_filter: u8,
    trainer_search: String,
    trainer_limit: usize,
    /// Spoiler protection is on at every start; the player confirms once per session to lift it.
    spoilers_shown: bool,
    spoiler_ask: bool,
    /// Werte tab: member shown in the profile, members drawn in the base-stat chart
    profile: usize,
    radar_show: [bool; 6],
    data_key: String,
    report: Option<Report>,
    dirty: bool,
    tab: Tab,
    /// boss the projection is made for; None = the next one
    proj_focus: Option<usize>,
    /// Fangen tab: whole national Pokédex instead of the game's own
    national_dex: bool,
    zoom: f32,
    message: Option<(bool, String)>, // (is_error, text)
    search: Vec<String>,
    update_status: Arc<Mutex<Option<String>>>,
    sprites: Sprites,
}

fn daytime(w: Daytime, l: Lang) -> &'static str {
    match w { Daytime::Any => "", Daytime::Day => t(l, " (by day)", " (tagsüber)"), Daytime::Night => t(l, " (at night)", " (nachts)") }
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
            trainers: vec![],
            beaten: None,
            show_beaten_bosses: false,
            trainer_filter: 0,
            trainer_search: String::new(),
            trainer_limit: 30,
            spoilers_shown: false,
            spoiler_ask: false,
            profile: 0,
            radar_show: [true; 6],
            data_key: String::new(),
            report: None,
            dirty: true,
            tab: Tab::Team,
            proj_focus: None,
            national_dex: saved.national_dex,
            zoom: saved.zoom,
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
                self.beaten = info.beaten_trainers.map(|v| v.into_iter().collect());
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
            Ok(RomData { bosses, level_up: rom::hgss_learnsets(&bytes).unwrap_or_default(), evolutions: rom::hgss_evolutions(&bytes).unwrap_or_default(), trainers: rom::hgss_trainers(&bytes, &self.dex), wild: rom::hgss_wild(&bytes), places: rom::hgss_places(&bytes) })
        });
        match result {
            Ok(r) => {
                self.rom = Some(r);
                self.rom_path = Some(path.into());
                if self.game_id != "heartgold-soulsilver" {
                    self.game_id = "heartgold-soulsilver".into();
                    self.badges.clear();
                }
                self.dirty = true;
                self.message = Some((false, format!("{} {}", t(self.lang, "ROM loaded:", "ROM geladen:"), file_name(path))));
            }
            Err(e) => self.message = Some((true, e)),
        }
    }

    /// Bosses and learnsets for the current game; a loaded HGSS ROM overrides the built-in data.
    fn ensure_game_data(&mut self) {
        let mmo = self.game_id.starts_with(crate::data::POKEMMO) && crate::pokemmo::ready();
        let key = format!("{}|{}{}", self.game_id, self.rom_path.as_deref().unwrap_or(""), if mmo { "|mmo" } else { "" });
        if key == self.data_key { return; }
        let vg = self.dex.vg(&self.game_id).expect("valid game id");
        self.learn = self.dex.learnsets(vg);
        let mut key = key;
        if mmo {
            match crate::pokemmo::learnsets() {
                Some((lv, tm)) => { self.learn.level_up = lv; self.learn.machine = tm; }
                // Unreadable (damaged) file: stay on B2W2 and drop it, so it is downloaded again.
                None => { crate::pokemmo::forget(); key.truncate(key.len() - 4); }
            }
        }
        self.bosses = self.dex.bosses.iter().find(|g| g.id == self.game_id).map(|g| g.bosses.clone()).unwrap_or_default();
        self.trainers.clear();
        if let (true, Some(r)) = (self.game_id == "heartgold-soulsilver", &self.rom) {
            self.bosses = r.bosses.clone();
            if !r.level_up.is_empty() { self.learn.level_up = r.level_up.clone(); }
            if !r.evolutions.is_empty() { self.learn.evolutions = r.evolutions.clone(); }
            self.trainers = r.trainers.clone();
            self.learn.wild = r.wild.clone();
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
        let game = Game { vg, bosses: &self.bosses, badges: &self.badges, learn: &self.learn, owned_machines: owned.as_ref(), regional: !self.national_dex, trainers: &self.trainers, beaten: self.beaten.as_ref() };
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
            if let Some(p) = self.save_path.clone() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(file_name(&p)).color(MUTED).small());
                    if ui.small_button(t(l, "Close", "Schließen"))
                        .on_hover_text(t(l, "Forget this save: the team stays and can be edited, and the game can be chosen again.",
                            "Spielstand vergessen: das Team bleibt und lässt sich ändern, das Spiel ist wieder wählbar."))
                        .clicked()
                    {
                        self.save_path = None;
                        self.trainer = None;
                        self.owned_tms = None;
                        self.beaten = None;
                        self.dirty = true;
                    }
                });
            }
            ui.label(RichText::new(t(l, "or drag it into the window", "oder einfach ins Fenster ziehen")).color(MUTED).small());
            ui.add_space(4.0);
            if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(t(l, "Open ROM … (optional)", "ROM öffnen … (optional)"))).clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("NDS ROM", &["nds"]).pick_file() {
                    self.load_rom(&p.to_string_lossy());
                }
            }
            match self.rom_path.clone() {
                Some(p) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(file_name(&p)).color(MUTED).small());
                        if ui.small_button(t(l, "Close", "Schließen")).clicked() {
                            self.rom = None;
                            self.rom_path = None;
                            self.dirty = true;
                        }
                    });
                }
                None => {
                    ui.label(RichText::new(t(l, "HeartGold/SoulSilver: real boss teams, all trainers and wild Pokémon, also for ROM hacks",
                        "HeartGold/SoulSilver: echte Boss-Teams, alle Trainer und wilde Pokémon, auch bei ROM-Hacks")).color(MUTED).small());
                }
            }

            ui.add_space(10.0);
            ui.separator();
            heading(ui, t(l, "Game", "Spiel"));
            let current = self.dex.vg(&self.game_id).map(|v| l.pick(&v.name_en, &v.name_de).to_string()).unwrap_or_default();
            let mut chosen = self.game_id.clone();
            let locked = self.save_path.is_some() || self.rom_path.is_some();
            ui.add_enabled_ui(!locked, |ui| {
                egui::ComboBox::from_id_salt("game").width(ui.available_width()).selected_text(current).show_ui(ui, |ui| {
                    for g in &self.dex.bosses {
                        if let Some(v) = self.dex.vg(&g.id) {
                            let label = format!("{}  (Gen {})", l.pick(&v.name_en, &v.name_de), v.gn);
                            match self.not_in_game(v.gn) {
                                None => { ui.selectable_value(&mut chosen, g.id.clone(), label); }
                                Some(why) => { ui.add_enabled(false, egui::Button::selectable(false, label)).on_disabled_hover_text(why); }
                            }
                        }
                    }
                });
            }).response.on_disabled_hover_text(t(l, "Set by your save / ROM. Close it above to choose another game.",
                "Vom Spielstand / der ROM festgelegt. Schließe sie oben, um ein anderes Spiel zu wählen."));
            if locked {
                ui.label(RichText::new(t(l, "Set by your save / ROM", "Vom Spielstand / der ROM festgelegt")).color(MUTED).small());
            }
            if self.game_id.starts_with(crate::data::POKEMMO) {
                let moves = if self.data_key.ends_with("|mmo") {
                    t(l, "Attacks: PokeMMO's own learnsets (PokeMMO Hub data).", "Attacken: PokeMMOs eigene Lernlisten (Daten vom PokeMMO Hub).")
                } else {
                    t(l, "Attacks: Black 2/White 2 until PokeMMO's learnsets are downloaded (needs internet).",
                        "Attacken: wie Schwarz 2/Weiß 2, bis PokeMMOs Lernlisten geladen sind (braucht Internet).")
                };
                ui.label(RichText::new(t(l, "Gen 5 rules; boss teams from the original games, PokeMMO's own may differ. No save to load: enter your team by hand.",
                    "Regeln wie Gen 5; Boss-Teams aus den Originalspielen, bei PokeMMO können sie abweichen. Kein Spielstand zum Laden: Team von Hand eingeben.")).color(MUTED).small());
                ui.label(RichText::new(moves).color(MUTED).small());
            }
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

    /// Why the team can't be used in a game of generation `gn` (a Pokémon or move from a later generation), if so.
    fn not_in_game(&self, gn: u8) -> Option<String> {
        let l = self.lang;
        for m in self.team.iter().filter(|m| m.species != 0) {
            if let Some(s) = self.dex.species.get(&m.species).filter(|s| s.gn > gn) {
                return Some(format!("{} {} {}", self.sname(s.id), t(l, "only exists from Gen", "gibt es erst ab Gen"), s.gn));
            }
            if let Some(mv) = m.moves.iter().filter_map(|id| self.dex.moves.get(id)).find(|mv| mv.gn > gn) {
                return Some(format!("{} ({}) {} {}", self.mname(mv.id), self.sname(m.species), t(l, "only exists from Gen", "gibt es erst ab Gen"), mv.gn));
            }
        }
        None
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
                                self.team[i] = Mon { species: id, level: self.team[i].level, moves: vec![], nickname: None, details: None };
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
            self.team.push(Mon { species: 0, level: 5, moves: vec![], nickname: None, details: None });
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
                self.team.push(Mon { species: 0, level: 5, moves: vec![], nickname: None, details: None });
            }
        });
    }

    fn header(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        card(ui, |ui| {
            ui.set_width(ui.available_width() - 28.0);
            ui.horizontal(|ui| {
                rating_ring(ui, r.summary.rating, 92.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(t(l, "Team rating", "Team-Bewertung")).size(18.0).strong());
                    ui.label(RichText::new(format!("{} {}  ·  {} {} (Lv {})", t(l, "Now", "Jetzt"), r.rating_now, t(l, "Potential", "Potenzial"), r.potential, r.potential_level)).color(MUTED))
                        .on_hover_text(t(l, "Rating = 60 % today's team + 40 % its potential: evolved, at the level of the last boss ahead, with its best moves by then. Today's team loses up to 25 points when its levels are uneven.",
                            "Bewertung = 60 % heutiges Team + 40 % Potenzial: entwickelt, auf dem Level des letzten Bosses, mit den besten Attacken bis dahin. Das heutige Team verliert bis zu 25 Punkte, wenn die Level ungleich sind."));
                    ui.add(egui::ProgressBar::new(r.summary.offense_pct as f32 / 100.0).desired_width(230.0).fill(bar_color(r.summary.offense_pct))
                        .text(format!("{} {}%", t(l, "Offensive coverage", "Angriffs-Abdeckung"), r.summary.offense_pct)));
                    ui.add(egui::ProgressBar::new(r.summary.defense_pct as f32 / 100.0).desired_width(230.0).fill(bar_color(r.summary.defense_pct))
                        .text(format!("{} {}%", t(l, "Types handled", "Typenvorteile"), r.summary.defense_pct)));
                    let bal = (r.comp.level_balance * 100.0).round() as u32;
                    ui.add(egui::ProgressBar::new(r.comp.level_balance).desired_width(230.0).fill(bar_color(bal))
                        .text(format!("{} {bal}%", t(l, "Level balance", "Level-Balance"))))
                        .on_hover_text(format!("{} −{} {}", t(l, "Uneven levels cost", "Ungleiche Level kosten"), r.comp.level_penalty,
                            t(l, "points of today's rating.", "Punkte der heutigen Bewertung.")));
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
                if ui.available_width() > 6.0 * 64.0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for m in self.team.iter().rev() { self.species_sprite(ui, m.species, 64.0); }
                    });
                }
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

    fn tab_stats(&mut self, ui: &mut Ui, r: &Report) {
        use analysis::Axis::*;
        let l = self.lang;
        let c = &r.comp;
        let axis_text = |a: analysis::Axis| -> (&'static str, &'static str) {
            match a {
                Offense => (t(l, "Coverage", "Angriffs-Abdeckung"), t(l, "Types your moves hit super effectively.", "Typen, die deine Attacken sehr effektiv treffen.")),
                Defense => (t(l, "Types handled", "Typenvorteile"), t(l, "Attack types your team resists or answers.", "Angriffstypen, die dein Team abwehrt oder kontert.")),
                Levels => (t(l, "Level balance", "Level-Balance"), t(l, "How close everyone is to your strongest member.", "Wie nah alle an deinem stärksten Pokémon sind.")),
                PhysSpec => (t(l, "Phys./Spec. mix", "Physisch/Spezial"), t(l, "Even mix of physical and special attacks: walls can't stop you with one stat.", "Ausgewogene physische und spezielle Attacken: eine Mauer stoppt dich nicht mit einem Wert.")),
                Speed => (t(l, "Speed", "Tempo"), t(l, "Average base Init.; 110 counts as full.", "Durchschnittlicher Basiswert Init.; 110 zählt voll.")),
                Bulk => (t(l, "Bulk", "Robustheit"), t(l, "Average base KP + Vert. + Sp. Ver.; 300 counts as full.", "Durchschnitt Basis-KP + Vert. + Sp. Ver.; 300 zählt voll.")),
                Power => (t(l, "Hitting power", "Angriffskraft"), t(l, "Average of each member's better attack stat; 120 counts as full.", "Durchschnitt des besseren Angriffswerts jedes Mitglieds; 120 zählt voll.")),
                Stab => (t(l, "STAB moves", "STAB-Attacken"), t(l, "Members with a damaging move of their own type (1.5x damage).", "Mitglieder mit einer Schadensattacke ihres eigenen Typs (1,5-facher Schaden).")),
                Variety => (t(l, "Type variety", "Typ-Vielfalt"), t(l, "Different types in the team.", "Verschiedene Typen im Team.")),
                Size => (t(l, "Team size", "Teamgröße"), t(l, "Members out of 6.", "Mitglieder von 6.")),
            }
        };
        let wide = ui.available_width() > 1200.0;
        let size = 300.0;
        let profile = |ui: &mut Ui| {
            card(ui, |ui| {
                ui.label(RichText::new(t(l, "Team profile", "Team-Profil")).size(17.0).strong());
                let labels: Vec<String> = c.axes.iter().map(|&(a, v)| format!("{} {}%", axis_text(a).0, (v * 100.0).round())).collect();
                radar(ui, size, &labels, &[(ACCENT, c.axes.iter().map(|a| a.1).collect())]);
            });
        };
        let show = std::cell::Cell::new(self.radar_show);
        let bases = |ui: &mut Ui| {
            card(ui, |ui| {
                ui.label(RichText::new(t(l, "Base stats (150 = edge)", "Basiswerte (150 = Rand)")).size(17.0).strong());
                let base_of = |i: usize| self.dex.species.get(&self.team[i].species).map(|s| s.base_stats);
                let mut max = [0f32; 6];
                for i in 0..self.team.len() { if let Some(b) = base_of(i) { for k in 0..6 { max[k] = max[k].max(b[k] as f32); } } }
                let labels: Vec<String> = stat_short(l).iter().enumerate()
                    .map(|(k, n)| format!("{n}  Ø {} · max {}", c.avg_base[k].round(), max[k])).collect();
                let mut series: Vec<(Color32, Vec<f32>)> = vec![
                    (Color32::from_rgb(0xff, 0xd0, 0x60), max.iter().map(|&b| b / 150.0).collect()),
                    (Color32::WHITE, c.avg_base.iter().map(|&b| b / 150.0).collect()),
                ];
                let on = show.get();
                for i in 0..self.team.len().min(6) {
                    if let (true, Some(b)) = (on[i], base_of(i)) { series.push((MEMBER_COLORS[i], b.iter().map(|&x| x as f32 / 150.0).collect())); }
                }
                radar(ui, size, &labels, &series);
                ui.horizontal_wrapped(|ui| {
                    swatch(ui, Color32::from_rgb(0xff, 0xd0, 0x60));
                    ui.label(RichText::new(t(l, "team maximum", "Team-Maximum")).small());
                    swatch(ui, Color32::WHITE);
                    ui.label(RichText::new(t(l, "team average", "Team-Durchschnitt")).small());
                });
                ui.horizontal_wrapped(|ui| {
                    let mut on = show.get();
                    for i in 0..self.team.len().min(6) {
                        swatch(ui, MEMBER_COLORS[i]);
                        if ui.selectable_label(on[i], self.sname(self.team[i].species)).clicked() { on[i] = !on[i]; }
                    }
                    if ui.small_button(t(l, "all", "alle")).clicked() { on = [true; 6]; }
                    if ui.small_button(t(l, "none", "keine")).clicked() { on = [false; 6]; }
                    show.set(on);
                });
            });
        };
        if wide {
            ui.horizontal_top(|ui| { profile(ui); ui.add_space(8.0); bases(ui); });
        } else {
            profile(ui);
            ui.add_space(8.0);
            bases(ui);
        }
        self.radar_show = show.get();
        ui.add_space(8.0);
        self.matchup_matrix(ui, r);
        ui.add_space(8.0);
        // Player profile: pick a member, see everything about it.
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t(l, "Profile", "Profil")).size(17.0).strong());
                ui.add_space(8.0);
                for i in 0..self.team.len() {
                    let on = self.profile == i;
                    let b = egui::Button::new(RichText::new(self.nickname(i).unwrap_or_else(|| self.sname(self.team[i].species)))
                        .color(if on { Color32::WHITE } else { MUTED })).fill(if on { ACCENT } else { CARD_HI }).corner_radius(CornerRadius::same(8));
                    if ui.add(b).clicked() { self.profile = i; }
                }
            });
        });
        if self.profile >= self.team.len() { self.profile = 0; }
        if !self.team.is_empty() { self.member_profile(ui, self.profile); }
        ui.add_space(8.0);
        // What each axis means, with its value.
        card(ui, |ui| {
            ui.label(RichText::new(t(l, "What the profile means", "Was das Profil bedeutet")).size(17.0).strong());
            egui::Grid::new("axes").spacing([12.0, 4.0]).show(ui, |ui| {
                for &(a, v) in &c.axes {
                    let pct = (v * 100.0).round() as u32;
                    ui.label(RichText::new(axis_text(a).0).strong());
                    ui.add(egui::ProgressBar::new(v).desired_width(160.0).fill(bar_color(pct)).text(format!("{pct}%")));
                    ui.label(RichText::new(axis_text(a).1).color(MUTED));
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.label(format!("{}: {} {}, {} {}, {} {}", t(l, "Moves", "Attacken"), c.physical, t(l, "physical", "physisch"),
                c.special, t(l, "special", "speziell"), c.status, t(l, "status", "Status")));
            if c.level_penalty > 0 {
                ui.label(RichText::new(format!("{} −{} {}", t(l, "Uneven levels cost", "Ungleiche Level kosten"), c.level_penalty,
                    t(l, "points of today's rating.", "Punkte der heutigen Bewertung."))).color(OKAY));
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t(l, "Types in the team:", "Typen im Team:")).color(MUTED));
                for &(ty, n) in &c.type_counts {
                    self.type_badge(ui, ty);
                    if n > 1 { ui.label(RichText::new(format!("×{n}")).strong().color(if n >= 3 { BAD } else { OKAY })); }
                }
            });
        });
        ui.add_space(8.0);
        // Every member in numbers: stats at its level (estimate), base stat total and role.
        card(ui, |ui| {
            ui.label(RichText::new(t(l, "Members", "Mitglieder")).size(17.0).strong());
            ui.label(RichText::new(t(l, "Stats at the current level: from your save where it has them (Gen 4), otherwise estimated with average IVs, no EVs and a neutral nature.",
                "Werte auf dem aktuellen Level: aus deinem Spielstand, wo er sie hat (Gen 4), sonst geschätzt mit durchschnittlichen DVs, ohne Fleißpunkte und mit neutralem Wesen.")).color(MUTED).small());
            egui::Grid::new("members").striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                ui.label("");
                ui.label("");
                for n in stat_short(l) { ui.label(RichText::new(n).color(MUTED)); }
                ui.label(RichText::new(t(l, "Base total", "Basis-Summe")).color(MUTED));
                ui.label(RichText::new(t(l, "Role", "Rolle")).color(MUTED));
                ui.end_row();
                for (i, m) in self.team.iter().enumerate() {
                    let Some(s) = self.dex.species.get(&m.species) else { continue };
                    let stats = m.details.as_ref().map_or_else(|| analysis::stats_at(&s.base_stats, m.level), |d| d.stats);
                    ui.horizontal(|ui| {
                        swatch(ui, MEMBER_COLORS[i % 6]);
                        self.species_sprite(ui, m.species, 40.0);
                        ui.label(RichText::new(self.sname(m.species)).strong());
                    });
                    ui.label(format!("Lv {}", m.level));
                    let top = (0..6).max_by_key(|&k| s.base_stats[k]).unwrap_or(0);
                    for (k, v) in stats.iter().enumerate() {
                        let txt = RichText::new(v.to_string());
                        ui.label(if k == top { txt.strong().color(GOOD) } else { txt });
                    }
                    ui.label(s.base_stats.iter().sum::<u16>().to_string());
                    ui.label(role(&s.base_stats, l));
                    ui.end_row();
                }
            });
        });
    }

    /// Members against every boss ahead: + for each of their Pokémon you hit super effectively, − for each that hits you.
    fn matchup_matrix(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        if r.bosses.is_empty() || self.team.is_empty() { return; }
        let gn = self.dex.vg(&self.game_id).map_or(9, |v| v.gn);
        let vg = self.dex.vg(&self.game_id).unwrap();
        card(ui, |ui| {
            ui.label(RichText::new(t(l, "Matchups against the bosses ahead", "Duelle gegen die kommenden Bosse")).size(17.0).strong());
            ui.label(RichText::new(t(l, "Per member, with today's types and moves: +1 for each of their Pokémon it hits super effectively, −1 for each whose type hits it super effectively.",
                "Pro Mitglied, mit heutigen Typen und Attacken: +1 für jedes ihrer Pokémon, das es sehr effektiv trifft, −1 für jedes, dessen Typ es sehr effektiv trifft.")).color(MUTED).small());
            egui::ScrollArea::horizontal().id_salt("matrix").show(ui, |ui| {
                egui::Grid::new("matrix_grid").spacing([6.0, 6.0]).show(ui, |ui| {
                    ui.label("");
                    for bv in r.bosses.iter().take(12) {
                        let b = &self.bosses[bv.boss];
                        ui.vertical_centered(|ui| {
                            sprite(ui, self.sprites.trainer(&b.name_en, &self.game_id), 40.0);
                            ui.label(RichText::new(l.pick(&b.name_en, &b.name_de)).small());
                        });
                    }
                    ui.end_row();
                    for (i, m) in self.team.iter().enumerate() {
                        let Some(s) = self.dex.species.get(&m.species) else { continue };
                        let my_types = self.dex.species_types(s, gn);
                        let attacks: Vec<u8> = m.moves.iter().filter_map(|id| self.dex.moves.get(id))
                            .filter(|mv| mv.class != 1).map(|mv| self.dex.move_stats(mv, vg)).filter(|x| x.1.is_some()).map(|x| x.0).collect();
                        ui.horizontal(|ui| { swatch(ui, MEMBER_COLORS[i % 6]); self.species_sprite(ui, m.species, 32.0); });
                        for bv in r.bosses.iter().take(12) {
                            let (mut good, mut bad) = (vec![], vec![]);
                            for bm in &bv.mons {
                                if attacks.iter().any(|&a| self.dex.eff(a, &bm.types, gn) >= 2.0) { good.push(bm.species); }
                                if bm.types.iter().any(|&a| self.dex.eff(a, &my_types, gn) >= 2.0) { bad.push(bm.species); }
                            }
                            let net = good.len() as i32 - bad.len() as i32;
                            let col = match net { n if n >= 2 => GOOD, 1 => Color32::from_rgb(0x2f, 0x7e, 0x50), 0 => CARD_HI, -1 => Color32::from_rgb(0x8a, 0x5a, 0x1e), _ => BAD };
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(64.0, 34.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 6.0, col);
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{net:+}"), egui::FontId::proportional(15.0), Color32::WHITE);
                            if self.spoilers_shown || bv.beaten {
                                let names = |v: &[u16]| v.iter().map(|&x| self.sname(x)).collect::<Vec<_>>().join(", ");
                                resp.on_hover_text(format!("{}: {}\n{}: {}", t(l, "You hit", "Du triffst"), names(&good), t(l, "Hits you", "Trifft dich"), names(&bad)));
                            }
                        }
                        ui.end_row();
                    }
                });
            });
        });
    }

    /// The member's nickname, if it has a real one (the games store the species name in capitals by default).
    fn nickname(&self, i: usize) -> Option<String> {
        let m = &self.team[i];
        m.nickname.clone().filter(|n| !n.is_empty() && n.to_lowercase() != self.sname(m.species).to_lowercase()
            && n.to_lowercase() != self.dex.species_name(m.species, Lang::En).to_lowercase())
    }

    fn item_name(&self, id: u16) -> Option<String> {
        self.dex.items4.get(&id).map(|(en, de)| self.lang.pick(en, de).to_string())
    }

    /// Football-manager style card: everything known about one member.
    fn member_profile(&self, ui: &mut Ui, i: usize) {
        let l = self.lang;
        let m = &self.team[i];
        let Some(s) = self.dex.species.get(&m.species) else { return };
        let gn = self.dex.vg(&self.game_id).map_or(9, |v| v.gn);
        let vg = self.dex.vg(&self.game_id).unwrap();
        let d = m.details.as_ref();
        let types = self.dex.species_types(s, gn);
        let stats = d.map_or_else(|| analysis::stats_at(&s.base_stats, m.level), |d| d.stats);
        card(ui, |ui| {
            ui.set_width(ui.available_width() - 28.0);
            // --- header ---
            ui.horizontal(|ui| {
                self.species_sprite(ui, m.species, 120.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let name = self.nickname(i);
                        ui.label(RichText::new(name.clone().unwrap_or_else(|| self.sname(m.species))).size(24.0).strong());
                        if name.is_some() { ui.label(RichText::new(self.sname(m.species)).size(16.0).color(MUTED)); }
                        ui.label(RichText::new(format!("Lv {}", m.level)).size(18.0));
                        self.types_row(ui, &types);
                        if d.is_some_and(|d| d.shiny) { pill(ui, t(l, "Shiny", "Schillernd"), OKAY); }
                        if d.is_some_and(|d| d.pokerus & 0x0F != 0) { pill(ui, "Pokérus", Color32::from_rgb(0xb0, 0x6a, 0xe0)); }
                    });
                    match d {
                        None => { ui.label(RichText::new(t(l, "Entered by hand: nature, IVs, EVs and item are unknown; stats are estimates.",
                            "Von Hand eingegeben: Wesen, DVs, Fleißpunkte und Item sind unbekannt; die Werte sind geschätzt.")).color(MUTED)); }
                        Some(d) => {
                            egui::Grid::new(("facts", i)).spacing([18.0, 3.0]).show(ui, |ui| {
                                let fact = |ui: &mut Ui, k: &str, v: String| { ui.label(RichText::new(k).color(MUTED)); ui.label(v); };
                                let nat = NATURES[d.nature as usize % 25];
                                let eff = analysis::nature_effect(d.nature).map(|(u, dn)| format!("  (+{} −{})", stat_short(l)[u], stat_short(l)[dn])).unwrap_or_default();
                                fact(ui, t(l, "Nature", "Wesen"), format!("{}{eff}", l.pick(nat.0, nat.1)));
                                let ab = self.dex.abilities.get(&d.ability).map(|(en, de)| l.pick(en, de).to_string()).unwrap_or_else(|| format!("#{}", d.ability));
                                fact(ui, t(l, "Ability", "Fähigkeit"), ab);
                                ui.end_row();
                                fact(ui, "Item", if d.item == 0 { "–".into() } else { self.item_name(d.item).unwrap_or_else(|| format!("#{}", d.item)) });
                                let ball = if (1..=16).contains(&d.ball) { self.item_name(d.ball).unwrap_or_default() } else { format!("#{}", d.ball) };
                                fact(ui, t(l, "Ball", "Ball"), ball);
                                ui.end_row();
                                let place = self.rom.as_ref().and_then(|r| r.places.get(d.met_place as usize)).cloned()
                                    .unwrap_or_else(|| format!("{} #{}", t(l, "place", "Ort"), d.met_place));
                                fact(ui, t(l, "Caught", "Gefangen"), if d.met_level == 0 { format!("{place} ({})", t(l, "hatched", "geschlüpft")) } else { format!("{place}, Lv {}", d.met_level) });
                                // Evoli has two (Psiana by day, Nachtara at night), so list them all.
                                let mut f = format!("{} / 255", d.friendship);
                                for &(to, h) in self.learn.evolutions.get(&m.species).into_iter().flatten() {
                                    if let EvoHow::Friendship(w) = h { f += &format!(" · {} {}", self.sname(to), self.friendship_progress(d.friendship, w, gn)); }
                                }
                                fact(ui, t(l, "Friendship", "Freundschaft"), f);
                                ui.end_row();
                            });
                            // HP and EXP bars
                            ui.horizontal(|ui| {
                                let hp = d.hp_now as f32 / d.stats[0].max(1) as f32;
                                ui.add(egui::ProgressBar::new(hp).desired_width(220.0).fill(if hp > 0.5 { GOOD } else if hp > 0.2 { OKAY } else { BAD })
                                    .text(format!("KP {} / {}", d.hp_now, d.stats[0])));
                                let (lo, hi) = (crate::data::exp_at(s.growth, m.level), crate::data::exp_at(s.growth, m.level.saturating_add(1).min(100)));
                                let frac = if hi > lo { (d.exp.saturating_sub(lo)) as f32 / (hi - lo) as f32 } else { 1.0 };
                                ui.add(egui::ProgressBar::new(frac).desired_width(260.0).fill(Color32::from_rgb(0x3d, 0x7b, 0xd8))
                                    .text(format!("EP {}  ·  {} {} Lv {}", d.exp, hi.saturating_sub(d.exp), t(l, "to", "bis"), (m.level + 1).min(100))));
                            });
                        }
                    }
                });
            });
            ui.add_space(8.0);
            // --- attributes 1-20, three columns like a football manager ---
            let reference = analysis::real_stats(&[130; 6], &[31; 6], &[0; 6], 0, m.level);
            let rate = |v: f32| (v.clamp(0.0, 1.0) * 19.0).round() as u8 + 1;
            let damaging: Vec<(u8, u16, Option<u8>)> = m.moves.iter().filter_map(|id| self.dex.moves.get(id)).filter(|mv| mv.class != 1)
                .filter_map(|mv| { let (ty, p) = self.dex.move_stats(mv, vg); Some((ty, p?, mv.accuracy)) }).collect();
            let all = self.dex.types_in_gen(gn);
            let hits = all.iter().filter(|&&def| damaging.iter().any(|(a, ..)| self.dex.eff(*a, &[def], gn) >= 2.0)).count();
            let resists = all.iter().filter(|&&a| self.dex.eff(a, &types, gn) < 1.0).count();
            let weak = all.iter().filter(|&&a| self.dex.eff(a, &types, gn) > 1.0).count();
            let final_bst = self.final_bst(m.species);
            let cols: [(&str, Vec<(String, Option<u8>, String)>); 3] = [
                (t(l, "Stats", "Werte"), (0..6).map(|k| (stat_short(l)[k].to_string(), Some(rate(stats[k] as f32 / reference[k] as f32)),
                    format!("{} {} ({} {})", stats[k], t(l, "at this level", "auf diesem Level"), t(l, "base", "Basis"), s.base_stats[k]))).collect()),
                (t(l, "Battle", "Kampf"), vec![
                    (t(l, "Coverage", "Abdeckung").into(), Some(rate(hits as f32 / all.len() as f32 * 2.0)), format!("{hits} {}", t(l, "types hit super effectively", "Typen sehr effektiv"))),
                    (t(l, "Move power", "Attackenstärke").into(), (!damaging.is_empty()).then(|| rate(damaging.iter().map(|x| x.1 as f32).sum::<f32>() / damaging.len() as f32 / 100.0)),
                        t(l, "average power of its damaging moves (100 = full)", "durchschnittliche Stärke der Schadensattacken (100 = voll)").into()),
                    (t(l, "Accuracy", "Genauigkeit").into(), (!damaging.is_empty()).then(|| rate((damaging.iter().map(|x| x.2.unwrap_or(100) as f32).sum::<f32>() / damaging.len() as f32 - 50.0) / 50.0)),
                        t(l, "average accuracy of its damaging moves", "durchschnittliche Genauigkeit der Schadensattacken").into()),
                    ("STAB".into(), Some(if damaging.iter().any(|x| types.contains(&x.0)) { 20 } else { 1 }), t(l, "has a damaging move of its own type", "hat eine Schadensattacke des eigenen Typs").into()),
                    (t(l, "Resistances", "Resistenzen").into(), Some(rate(resists as f32 / 8.0)), format!("{resists} {}", t(l, "attack types resisted", "Angriffstypen abgewehrt"))),
                    (t(l, "Few weaknesses", "Wenig Schwächen").into(), Some(rate(1.0 - weak as f32 / 6.0)), format!("{weak} {}", t(l, "weaknesses", "Schwächen"))),
                ]),
                (t(l, "Development", "Entwicklung"), vec![
                    (t(l, "Potential", "Potenzial").into(), Some(rate((final_bst as f32 - 250.0) / 350.0)), format!("{} {final_bst}", t(l, "base total when fully evolved:", "Basis-Summe voll entwickelt:"))),
                    (t(l, "IV quality", "DV-Qualität").into(), d.map(|d| rate(d.ivs.iter().map(|&x| x as f32).sum::<f32>() / 186.0)),
                        d.map_or("–".into(), |d| format!("{} / 186", d.ivs.iter().map(|&x| x as u32).sum::<u32>()))),
                    (t(l, "Training (EVs)", "Training (Fleißpunkte)").into(), d.map(|d| rate(d.evs.iter().map(|&x| x as f32).sum::<f32>() / 510.0)),
                        d.map_or("–".into(), |d| format!("{} / 510", d.evs.iter().map(|&x| x as u32).sum::<u32>()))),
                    (t(l, "Friendship", "Freundschaft").into(), d.map(|d| rate(d.friendship as f32 / 255.0)), d.map_or("–".into(), |d| format!("{} / 255", d.friendship))),
                    (t(l, "Level in team", "Level im Team").into(), Some(rate(m.level as f32 / self.team.iter().map(|x| x.level).max().unwrap_or(1) as f32)),
                        t(l, "compared with your strongest member", "im Vergleich zu deinem stärksten Mitglied").into()),
                ]),
            ];
            ui.horizontal_top(|ui| {
                for (title, rows) in cols.iter() {
                    egui::Frame::new().fill(CARD_HI).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                        ui.set_width(250.0);
                        ui.vertical(|ui| {
                        ui.label(RichText::new(*title).strong());
                        egui::Grid::new((title, i)).spacing([10.0, 3.0]).min_col_width(40.0).show(ui, |ui| {
                            for (name, v, tip) in rows {
                                ui.label(name).on_hover_text(tip);
                                match v {
                                    Some(v) => ui.label(RichText::new(v.to_string()).strong().size(16.0).color(attr_color(*v))).on_hover_text(tip),
                                    None => ui.label(RichText::new("–").color(MUTED)),
                                };
                                ui.end_row();
                            }
                        });
                        });
                    });
                }
            });
            ui.label(RichText::new(t(l, "1-20 like a football manager. Stats: 20 = what a base-130 Pokémon with perfect IVs reaches at this level. Hover for the numbers.",
                "1–20 wie in einem Fußballmanager. Werte: 20 = was ein Pokémon mit Basiswert 130 und perfekten DVs auf diesem Level erreicht. Für die Zahlen mit der Maus darüberfahren.")).color(MUTED).small());
            ui.add_space(8.0);
            // --- IVs and EVs ---
            if let Some(d) = d {
                ui.horizontal_top(|ui| {
                    for (title, vals, max, total) in [(t(l, "IVs (0-31)", "DVs (0–31)"), d.ivs.map(|x| x as u32), 31u32, 186u32), (t(l, "EVs (0-255)", "Fleißpunkte (0–255)"), d.evs.map(|x| x as u32), 255, 510)] {
                        egui::Frame::new().fill(CARD_HI).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                            ui.set_width(380.0);
                            ui.vertical(|ui| {
                            ui.label(RichText::new(format!("{title}   {} / {total}", vals.iter().sum::<u32>())).strong());
                            egui::Grid::new((title, "bars", i)).spacing([8.0, 3.0]).show(ui, |ui| {
                                for k in 0..6 {
                                    let nat = analysis::nature_effect(d.nature);
                                    let mark = match nat { Some((u, _)) if u == k => " +", Some((_, dn)) if dn == k => " −", _ => "" };
                                    ui.label(RichText::new(format!("{}{mark}", stat_short(l)[k])).color(MUTED));
                                    ui.label(RichText::new(vals[k].to_string()).strong());
                                    let frac = vals[k] as f32 / max as f32;
                                    ui.add(egui::ProgressBar::new(frac).desired_width(220.0).desired_height(8.0)
                                        .fill(if vals[k] == max { GOOD } else { Color32::from_rgb(0x3d, 0x7b, 0xd8) }));
                                    ui.end_row();
                                }
                            });
                            });
                        });
                    }
                });
                ui.add_space(8.0);
            }
            // --- moves ---
            egui::Frame::new().fill(CARD_HI).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.label(RichText::new(t(l, "Moves", "Attacken")).strong());
                egui::Grid::new(("moves_detail", i)).striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
                    for h in [t(l, "Move", "Attacke"), t(l, "Type", "Typ"), t(l, "Class", "Kategorie"), t(l, "Power", "Stärke"), t(l, "Accuracy", "Genauigkeit"), t(l, "PP", "AP"), ""] {
                        ui.label(RichText::new(h).color(MUTED));
                    }
                    ui.end_row();
                    for (k, &id) in m.moves.iter().enumerate() {
                        let Some(mv) = self.dex.moves.get(&id) else { continue };
                        let (ty, p) = self.dex.move_stats(mv, vg);
                        let class = if mv.class == 1 || p.is_none() { 0 } else if gn <= 3 { if matches!(ty, 10..=17) { 2 } else { 1 } } else if mv.class == 3 { 2 } else { 1 };
                        ui.label(RichText::new(self.mname(id)).strong());
                        self.type_badge(ui, ty);
                        ui.label([t(l, "Status", "Status"), t(l, "Physical", "Physisch"), t(l, "Special", "Speziell")][class]);
                        ui.label(p.map_or("–".into(), |p| p.to_string()));
                        ui.label(mv.accuracy.map_or("–".into(), |a| format!("{a}%")));
                        let max_pp = mv.pp as u32 * (5 + d.map_or(0, |d| d.pp_ups[k.min(3)] as u32)) / 5;
                        ui.label(d.map_or(max_pp.to_string(), |d| format!("{} / {max_pp}", d.pp[k.min(3)])));
                        if class != 0 && types.contains(&ty) { pill(ui, "STAB", GOOD); } else { ui.label(""); }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);
            // --- growth to Lv 100 ---
            egui::Frame::new().fill(CARD_HI).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.label(RichText::new(t(l, "Stat growth to Lv 100 (level evolutions included)", "Wertentwicklung bis Lv 100 (mit Level-Entwicklungen)")).strong());
                let (ivs, evs, nature) = d.map_or(([15; 6], [0; 6], 0), |d| (d.ivs, d.evs, d.nature));
                let mut lines: Vec<Vec<(f32, f32)>> = vec![vec![]; 6];
                let mut marks = vec![];
                let mut sp = m.species;
                for lv in m.level..=100 {
                    if let Some(&(to, _)) = self.learn.evolutions.get(&sp).and_then(|v| v.iter().find(|e| matches!(e.1, EvoHow::Level(x) if x <= lv))) {
                        sp = to;
                        marks.push((lv as f32, self.sname(to)));
                    }
                    let base = self.dex.species.get(&sp).map_or(s.base_stats, |x| x.base_stats);
                    let st = analysis::real_stats(&base, &ivs, &evs, nature, lv);
                    for k in 0..6 { lines[k].push((lv as f32, st[k] as f32)); }
                }
                let series: Vec<(Color32, Vec<(f32, f32)>)> = lines.into_iter().enumerate().map(|(k, v)| (STAT_COLORS[k], v)).collect();
                line_chart(ui, ui.available_width().min(1100.0), 260.0, (m.level as f32, 100.0), &series, &marks);
                ui.horizontal_wrapped(|ui| {
                    for k in 0..6 { swatch(ui, STAT_COLORS[k]); ui.label(RichText::new(stat_short(l)[k]).small()); }
                    if d.is_none() { ui.label(RichText::new(t(l, "(average IVs, no EVs)", "(durchschnittliche DVs, ohne Fleißpunkte)")).small().color(MUTED)); }
                });
            });
        });
    }

    /// Base stat total of the last level-evolution stage (or the first branch otherwise).
    fn final_bst(&self, species: u16) -> u16 {
        let mut sp = species;
        for _ in 0..3 {
            match self.learn.evolutions.get(&sp).and_then(|v| v.first()) { Some(&(to, _)) => sp = to, None => break }
        }
        self.dex.species.get(&sp).map_or(0, |s| s.base_stats.iter().sum())
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
        if ui.available_width() > 1150.0 {
            ui.horizontal_top(|ui| {
                self.defense_table(ui, r);
                ui.add_space(8.0);
                self.defense_summary(ui, r, 380.0);
            });
        } else {
            self.defense_summary(ui, r, 0.0);
            ui.add_space(8.0);
            self.defense_table(ui, r);
        }
    }

    fn defense_summary(&self, ui: &mut Ui, r: &Report, width: f32) {
        let l = self.lang;
        card(ui, |ui| {
            if width > 0.0 { ui.set_width(width); }
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
    }

    fn defense_table(&self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
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
            ui.label(format!("{}: {} » {}/100", t(l, "Team rating", "Team-Bewertung"), r.rating_now, p.rating_after));
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
                        EvoHow::Trade => t(l, "by trade", "durch Tausch").to_string(),
                        EvoHow::Item => t(l, "with an item", "mit Item").to_string(),
                        EvoHow::Friendship(w) => match &m.details {
                            Some(d) => format!("{} {}", t(l, "friendship", "Freundschaft"), self.friendship_progress(d.friendship, w, vg.gn)),
                            None => format!("{} {}", t(l, "with friendship", "durch Freundschaft"), daytime(w, l)),
                        },
                        _ => t(l, "special condition", "besondere Bedingung").to_string(),
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

    fn tab_catch(&mut self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        ui.horizontal(|ui| {
            ui.label(RichText::new(t(l, "Pokémon from", "Pokémon aus")).color(MUTED));
            let has_regional = !self.learn.regional.is_empty();
            for (national, en, de) in [(false, "this game's Pokédex", "dem Pokédex dieses Spiels"), (true, "the national Pokédex", "dem Nationalen Pokédex")] {
                if national == false && !has_regional { continue; }
                let on = self.national_dex == national || (!has_regional && national);
                let b = egui::Button::new(RichText::new(t(l, en, de)).color(if on { Color32::WHITE } else { MUTED }))
                    .fill(if on { ACCENT } else { CARD }).corner_radius(CornerRadius::same(8));
                if ui.add(b).clicked() && self.national_dex != national {
                    self.national_dex = national;
                    self.dirty = true;
                }
            }
        });
        ui.add_space(4.0);
        if r.catch.is_empty() {
            card(ui, |ui| ui.label(t(l, "No single typing improves the team; work on moves and levels.", "Kein Typ verbessert das Team; arbeite an Attacken und Leveln.")));
            return;
        }
        let note = if !self.national_dex && !self.learn.wild.is_empty() {
            t(l, "Only Pokémon you can catch in the wild in your ROM, or evolve one into. Place and level come from the ROM; when in the story (badges) is a rough guide from walkthroughs. What you can get soon comes first.",
                "Nur Pokémon, die du in deiner ROM wild fangen kannst oder durch Entwicklung bekommst. Ort und Level stammen aus der ROM; ab wie vielen Orden ist ein grober Richtwert aus Komplettlösungen. Was du bald bekommst, steht vorn.")
        } else if self.national_dex || self.learn.regional.is_empty() {
            t(l, "Examples come from every Pokémon up to this generation; not all of them can be caught in your game.",
                "Beispiele aus allen Pokémon bis zu dieser Generation; nicht alle sind in deinem Spiel fangbar.")
        } else {
            t(l, "Only Pokémon from this game's own Pokédex (including ones you get by evolving). A ROM hack may place others.",
                "Nur Pokémon aus dem Pokédex dieses Spiels (auch solche, die man durch Entwicklung bekommt). Ein ROM-Hack kann andere verteilen.")
        };
        ui.label(RichText::new(note).color(MUTED));
        // For a free slot: the next good Pokémon you can get, and the strongest one if that is a different one.
        let slot = self.team.len() + 1;
        let strongest = r.catch.iter().find(|c| c.replacing.is_none());
        ui.horizontal_wrapped(|ui| {
            if let Some(p) = &r.next_pick {
                let detail = match (&p.spot, p.dex_no) {
                    (Some((src, w)), _) => self.spot_text(*src, p.species, w),
                    (None, Some(n)) => format!("{} {n}{}", t(l, "Pokédex no.", "Pokédex Nr."),
                        t(l, " — rough order; load the ROM for places and levels", " – grobe Reihenfolge; lade die ROM für Fundorte und Level")),
                    _ => String::new(),
                };
                self.pick_card(ui, &format!("{} {slot}", t(l, "Next for slot", "Als Nächstes für Platz")), p.species, &p.types, p.gain, &detail);
            }
            if let Some(c) = strongest.filter(|c| r.next_pick.as_ref().is_none_or(|p| p.species != c.examples[0])) {
                let detail = match c.spots.first() {
                    Some(Some((src, w))) => self.spot_text(*src, c.examples[0], w),
                    _ => String::new(),
                };
                let title = if r.next_pick.is_some() { t(l, "Strongest choice", "Stärkste Wahl").to_string() } else { format!("{} {slot}", t(l, "Pick for slot", "Empfehlung für Platz")) };
                self.pick_card(ui, &title, c.examples[0], &c.types, c.gain, &detail);
            }
        });
        ui.add_space(6.0);
        let items: Vec<_> = r.catch.iter().collect();
        card_rows(ui, &items, 540.0, |ui, &c| {
            card(ui, |ui| {
                ui.set_width(540.0);
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
                    for (k, &id) in c.examples.iter().enumerate() {
                        ui.vertical(|ui| {
                            self.species_sprite(ui, id, 64.0);
                            ui.label(RichText::new(self.sname(id)).small());
                            if let Some(Some((_, w))) = c.spots.get(k) {
                                ui.label(RichText::new(format!("{} · Lv {}{}", w.place, w.level,
                                    if w.late() || w.badges <= self.badge_count() { String::new() } else { format!(" · {} {}", w.badges, t(self.lang, "badges", "Orden")) })).small().color(if w.late() { BAD } else if w.badges > self.badge_count() { OKAY } else { MUTED }))
                                    .on_hover_text(self.spot_text(c.spots[k].as_ref().unwrap().0, id, w));
                            }
                        });
                    }
                });
            });
        });
    }

    fn tab_bosses(&mut self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        self.spoiler_bar(ui);
        if !r.beaten_bosses.is_empty() {
            let label = format!("{} ({})", t(l, "Show beaten bosses", "Besiegte Bosse zeigen"), r.beaten_bosses.len());
            ui.checkbox(&mut self.show_beaten_bosses, label);
            ui.add_space(4.0);
        }
        if r.bosses.is_empty() && !(self.show_beaten_bosses && !r.beaten_bosses.is_empty()) {
            card(ui, |ui| ui.label(t(l, "No bosses left (or no boss data for this game).", "Keine Bosse mehr übrig (oder keine Boss-Daten für dieses Spiel).")));
            return;
        }
        let mut items: Vec<(&analysis::BossView, bool)> = vec![];
        if self.show_beaten_bosses { items.extend(r.beaten_bosses.iter().map(|b| (b, false))); }
        items.extend(r.bosses.iter().enumerate().map(|(k, b)| (b, k == 0)));
        card_rows(ui, &items, 540.0, |ui, &(bv, next)| {
            self.fight_card(ui, bv, &self.bosses[bv.boss], next, next, "boss");
        });
    }

    fn tab_trainers(&mut self, ui: &mut Ui, r: &Report) {
        let l = self.lang;
        if r.trainers.is_empty() {
            card(ui, |ui| ui.label(t(l,
                "Load your HeartGold/SoulSilver ROM (left) to see every trainer and your rival, read from the ROM itself.",
                "Lade deine HeartGold/SoulSilver-ROM (links), dann siehst du jeden Trainer und deinen Rivalen, direkt aus der ROM gelesen.")));
            return;
        }
        let beaten = r.trainers.iter().filter(|v| v.beaten).count();
        ui.horizontal(|ui| {
            for (k, en, de, n) in [(0u8, "Upcoming", "Kommend", r.trainers.len() - beaten), (1, "Beaten", "Besiegt", beaten), (2, "All", "Alle", r.trainers.len())] {
                let on = self.trainer_filter == k;
                let b = egui::Button::new(RichText::new(format!("{} ({n})", t(l, en, de))).color(if on { Color32::WHITE } else { MUTED }))
                    .fill(if on { ACCENT } else { CARD }).corner_radius(CornerRadius::same(8));
                if ui.add(b).clicked() { self.trainer_filter = k; self.trainer_limit = 30; }
            }
            ui.add_space(12.0);
            if ui.add(egui::TextEdit::singleline(&mut self.trainer_search)
                .hint_text(t(l, "Search name, class or Pokémon", "Name, Klasse oder Pokémon suchen")).desired_width(260.0)).changed()
            {
                self.trainer_limit = 30;
            }
        });
        self.spoiler_bar(ui);
        if !self.spoilers_shown && self.trainer_filter != 1 {
            card(ui, |ui| ui.label(t(l,
                "Upcoming trainers are hidden by the spoiler protection. Your beaten fights are under \"Beaten\".",
                "Kommende Trainer sind durch den Spoiler-Schutz verborgen. Deine geschlagenen Kämpfe stehen unter \"Besiegt\".")));
            return;
        }
        ui.label(RichText::new(t(l,
            "Sorted by their strongest Pokémon's level. Each fight is judged with your team raised to that level. Beaten = marked in your save.",
            "Sortiert nach dem Level ihres stärksten Pokémon. Jeder Kampf wird mit deinem Team auf diesem Level bewertet. Besiegt = laut deinem Spielstand.")).color(MUTED));
        ui.add_space(4.0);
        let q = crate::data::norm(&self.trainer_search);
        let list: Vec<&analysis::BossView> = r.trainers.iter()
            .filter(|v| match self.trainer_filter { 0 => !v.beaten, 1 => v.beaten, _ => true })
            .filter(|v| {
                if q.is_empty() { return true; }
                let b = &self.trainers[v.boss];
                crate::data::norm(&b.name_de).contains(&q) || v.mons.iter().any(|m| crate::data::norm(&self.sname(m.species)).contains(&q))
            })
            .collect();
        let shown: Vec<(&analysis::BossView, bool)> = list.iter().take(self.trainer_limit).map(|&v| (v, false)).collect();
        card_rows(ui, &shown, 540.0, |ui, &(bv, _)| {
            self.fight_card(ui, bv, &self.trainers[bv.boss], false, false, "trainer");
        });
        if list.len() > self.trainer_limit
            && ui.add_sized([240.0, 32.0], egui::Button::new(format!("{} ({} {})", t(l, "Show more", "Mehr zeigen"), list.len() - self.trainer_limit, t(l, "left", "übrig")))).clicked()
        {
            self.trainer_limit += 30;
        }
    }

    fn pick_card(&self, ui: &mut Ui, title: &str, species: u16, types: &[u8], gain: i32, detail: &str) {
        card(ui, |ui| {
            ui.set_width(540.0);
            ui.horizontal(|ui| {
                self.species_sprite(ui, species, 96.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).color(MUTED));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.sname(species)).size(22.0).strong());
                        self.types_row(ui, types);
                        ui.label(RichText::new(format!("+{gain}")).size(18.0).strong().color(GOOD));
                    });
                    if !detail.is_empty() { ui.add(egui::Label::new(RichText::new(detail).size(15.0)).wrap()); }
                });
            });
        });
    }

    /// "Catch X on Route 32 (grass, from Lv 6)", naming the earlier form when you evolve it yourself.
    fn spot_text(&self, src: u16, target: u16, w: &crate::data::WildSpot) -> String {
        use crate::data::CatchHow::*;
        let l = self.lang;
        let how = match w.how {
            Grass => t(l, "grass", "Gras").to_string(),
            Surf => self.mname(57),
            RockSmash => self.mname(249),
            Rod => t(l, "fishing", "Angeln").to_string(),
        };
        let evolve = if src != target { format!(", {} {}", t(l, "evolve it into", "entwickeln zu"), self.sname(target)) } else { String::new() };
        let when = if w.late() { t(l, " – only after the Pokémon League", " – erst nach der Pokémon-Liga").to_string() }
            else if w.badges > self.badge_count() { format!(" – {} {} {}", t(l, "from", "ab"), w.badges, t(l, "badges", "Orden")) }
            else { t(l, " – reachable now", " – schon erreichbar").to_string() };
        format!("{} {} {} {} ({}, {} Lv {}){evolve}{when}", t(l, "Catch", "Fange"), self.sname(src), t(l, "at", "bei"), w.place, how, t(l, "from", "ab"), w.level)
    }

    /// "180/220" or "ready, next level-up", plus day/night when it matters.
    fn friendship_progress(&self, now: u8, when: Daytime, gn: u8) -> String {
        let l = self.lang;
        let need = crate::data::friendship_needed(gn);
        let state = if now >= need { t(l, "ready: evolves at the next level-up", "bereit: entwickelt sich beim nächsten Level-Up").to_string() }
            else { format!("{now}/{need}, {} {}", t(l, "missing", "fehlen"), need - now) };
        format!("{state}{}", daytime(when, l))
    }

    fn badge_count(&self) -> u8 {
        self.badges.iter().filter(|&&b| b).count() as u8
    }

    /// Spoiler protection status, with a confirm dialog before revealing upcoming teams.
    fn spoiler_bar(&mut self, ui: &mut Ui) {
        let l = self.lang;
        ui.horizontal(|ui| {
            if self.spoilers_shown {
                ui.label(RichText::new(t(l, "Spoilers shown: upcoming teams and their moves are visible.",
                    "Spoiler sichtbar: kommende Teams und ihre Attacken werden angezeigt.")).color(OKAY));
                if ui.button(t(l, "Hide again", "Wieder verbergen")).clicked() { self.spoilers_shown = false; }
            } else {
                ui.label(RichText::new(t(l, "Spoiler protection: teams and moves of fights ahead are hidden. The verdict stays visible.",
                    "Spoiler-Schutz: Teams und Attacken kommender Kämpfe sind verborgen. Die Einschätzung bleibt sichtbar.")).color(MUTED));
                if ui.button(t(l, "Show spoilers …", "Spoiler anzeigen …")).clicked() { self.spoiler_ask = true; }
            }
        });
        ui.add_space(4.0);
        if self.spoiler_ask {
            let ctx = ui.ctx().clone();
            let resp = egui::Modal::new(egui::Id::unique("spoiler_ask")).show(&ctx, |ui| {
                ui.set_width(400.0);
                ui.label(RichText::new(t(l, "Show spoilers?", "Spoiler anzeigen?")).size(18.0).strong());
                ui.add_space(4.0);
                ui.label(t(l, "This shows the teams and moves of every upcoming boss, rival and trainer. Do you really want to see them?",
                    "Damit siehst du Teams und Attacken aller kommenden Bosse, Rivalen und Trainer. Willst du sie wirklich sehen?"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(t(l, "Yes, show them", "Ja, anzeigen")).clicked() { self.spoilers_shown = true; self.spoiler_ask = false; }
                    if ui.button(t(l, "No", "Nein")).clicked() { self.spoiler_ask = false; }
                });
            });
            if resp.should_close() { self.spoiler_ask = false; }
        }
    }

    /// One fight (boss or trainer): who, verdict, levels, and their team with your answer to each Pokémon.
    fn fight_card(&self, ui: &mut Ui, bv: &analysis::BossView, b: &Boss, next: bool, open: bool, salt: &str) {
        let l = self.lang;
        let our_max = self.team.iter().map(|m| m.level).max().unwrap_or(1);
        let hidden = !self.spoilers_shown && !bv.beaten;
        card(ui, |ui| {
            ui.set_width(540.0);
            if bv.beaten { ui.multiply_opacity(0.6); }
            ui.horizontal(|ui| {
                match b.kind.as_str() {
                    // Route trainers have no portrait source; their ace stands in.
                    "trainer" if hidden => mystery(ui, 72.0),
                    "trainer" => self.species_sprite(ui, b.team.iter().max_by_key(|m| m.level).map_or(0, |m| m.dex), 72.0),
                    "rival" => sprite(ui, self.sprites.trainer("silver", &self.game_id), 72.0),
                    _ => sprite(ui, self.sprites.trainer(&b.name_en, &self.game_id), 72.0),
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(l.pick(&b.name_en, &b.name_de)).size(17.0).strong());
                        if let Some(ty) = b.type_.as_deref().and_then(|x| self.dex.type_by_ident(x)) { self.type_badge(ui, ty); }
                        if b.kind == "rival" { pill(ui, t(l, "Rival", "Rivale"), Color32::from_rgb(0x8a, 0x5c, 0xd6)); }
                        self.verdict_pill(ui, bv.verdict);
                        if bv.beaten { pill(ui, t(l, "Beaten", "Besiegt"), CARD_HI); }
                        if next { pill(ui, t(l, "next", "als Nächstes"), ACCENT); }
                    });
                    let place = l.pick(b.location_en.as_deref().unwrap_or(""), b.location_de.as_deref().unwrap_or(""));
                    let var = b.variant.as_ref().map(|v| format!("  ({v})")).unwrap_or_default();
                    if !place.is_empty() || !var.is_empty() { ui.label(RichText::new(format!("{place}{var}")).color(MUTED)); }
                    let gap = bv.their_max as i32 - our_max as i32;
                    let col = if bv.beaten { MUTED } else if gap > 3 { BAD } else if gap > 0 { OKAY } else { GOOD };
                    ui.label(RichText::new(format!("{} {}  ·  {} {}", t(l, "Their top level", "Ihr höchstes Level"), bv.their_max, t(l, "yours now", "deins jetzt"), our_max)).color(col));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t(l, "Judged with your team at their level:", "Bewertet mit deinem Team auf ihrem Level:")).color(MUTED).small());
                        for &(sp, _) in &bv.team { self.species_sprite(ui, sp, 32.0); }
                    });
                });
            });
            if hidden {
                ui.label(RichText::new(t(l, "Team and moves hidden (spoiler protection)", "Team und Attacken verborgen (Spoiler-Schutz)")).color(MUTED).italics());
                return;
            }
            egui::CollapsingHeader::new(t(l, "Their team and your answers", "Ihr Team und deine Konter"))
                .id_salt((salt, bv.boss)).default_open(open).show(ui, |ui| {
                egui::Grid::new((salt, "grid", bv.boss)).spacing([14.0, 6.0]).show(ui, |ui| {
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
        if self.game_id.starts_with(crate::data::POKEMMO) {
            if !crate::pokemmo::ready() { crate::pokemmo::fetch(ctx.clone()); }
            else if !self.data_key.ends_with("|mmo") { self.dirty = true; } // download just finished
        }
        if self.dirty { self.recompute(); }
        let l = self.lang;
        // Text size: big monitors start larger; Ctrl +/- and the A buttons change it, and it is remembered.
        if self.zoom == 0.0 {
            if let Some(m) = ctx.input(|i| i.viewport().monitor_size) {
                self.zoom = ((m.x / 1600.0).clamp(1.0, 1.6) * 10.0).round() / 10.0;
                ctx.set_zoom_factor(self.zoom);
            }
        } else if (ctx.zoom_factor() - self.zoom).abs() > 0.001 && ctx.cumulative_frame_nr() < 2 {
            ctx.set_zoom_factor(self.zoom);
        } else {
            self.zoom = ctx.zoom_factor();
        }

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
                    let tip = t(l, "Text size (also Ctrl + / Ctrl -)", "Textgröße (auch Strg + / Strg -)");
                    if ui.button("A+").on_hover_text(tip).clicked() { ctx.set_zoom_factor((ctx.zoom_factor() + 0.1).min(2.5)); }
                    if ui.button("A−").on_hover_text(tip).clicked() { ctx.set_zoom_factor((ctx.zoom_factor() - 0.1).max(0.7)); }
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
                    (Tab::Team, "Team", "Team"), (Tab::Stats, "Stats", "Werte"), (Tab::Defense, "Weaknesses", "Schwächen"), (Tab::Offense, "Attack", "Angriff"),
                    (Tab::Projection, "Projection", "Projektion"), (Tab::Catch, "Catch next", "Fangen"), (Tab::Bosses, "Bosses", "Bosse"), (Tab::Trainers, "Trainers", "Trainer"),
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
                Tab::Stats => self.tab_stats(ui, &r),
                Tab::Defense => self.tab_defense(ui, &r),
                Tab::Offense => self.tab_offense(ui, &r),
                Tab::Projection => self.tab_projection(ui, &r),
                Tab::Catch => self.tab_catch(ui, &r),
                Tab::Bosses => self.tab_bosses(ui, &r),
                Tab::Trainers => self.tab_trainers(ui, &r),
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
            national_dex: self.national_dex,
            zoom: self.zoom,
        };
        eframe::set_value(storage, eframe::APP_KEY, &saved);
    }
}

/// Short stat names as the German SoulSilver summary screen writes them (ROM text a/0/2/7, files 31 and 302).
fn stat_short(l: Lang) -> [&'static str; 6] {
    if l == Lang::De { ["KP", "Angriff", "Vert.", "Sp. Ang.", "Sp. Ver.", "Init."] } else { ["HP", "Atk", "Def", "SpA", "SpD", "Spe"] }
}

const MEMBER_COLORS: [Color32; 6] = [
    Color32::from_rgb(0xe8, 0x4a, 0x4a), Color32::from_rgb(0x3d, 0x9b, 0xe8), Color32::from_rgb(0x4c, 0xc2, 0x7a),
    Color32::from_rgb(0xf0, 0xb9, 0x3a), Color32::from_rgb(0xb0, 0x6a, 0xe0), Color32::from_rgb(0x3a, 0xd0, 0xc8),
];

/// German nature names from the ROM (a/0/2/7 file 34), with the English ones.
const NATURES: [(&str, &str); 25] = [
    ("Hardy", "Robust"), ("Lonely", "Solo"), ("Brave", "Mutig"), ("Adamant", "Hart"), ("Naughty", "Frech"),
    ("Bold", "Kühn"), ("Docile", "Sanft"), ("Relaxed", "Locker"), ("Impish", "Pfiffig"), ("Lax", "Lasch"),
    ("Timid", "Scheu"), ("Hasty", "Hastig"), ("Serious", "Ernst"), ("Jolly", "Froh"), ("Naive", "Naiv"),
    ("Modest", "Mäßig"), ("Mild", "Mild"), ("Quiet", "Ruhig"), ("Bashful", "Zaghaft"), ("Rash", "Hitzig"),
    ("Calm", "Still"), ("Gentle", "Zart"), ("Sassy", "Forsch"), ("Careful", "Sacht"), ("Quirky", "Kauzig"),
];

const STAT_COLORS: [Color32; 6] = [
    Color32::from_rgb(0x4c, 0xc2, 0x7a), Color32::from_rgb(0xe8, 0x5a, 0x5a), Color32::from_rgb(0xf0, 0xb9, 0x3a),
    Color32::from_rgb(0x3d, 0x9b, 0xe8), Color32::from_rgb(0x6a, 0xd0, 0xc8), Color32::from_rgb(0xe0, 0x6a, 0xd0),
];

/// Football-manager colours for a 1-20 attribute.
fn attr_color(v: u8) -> Color32 {
    match v {
        16.. => Color32::from_rgb(0x4c, 0xd2, 0x7a),
        11..=15 => Color32::from_rgb(0xa8, 0xd8, 0x6a),
        6..=10 => Color32::from_rgb(0xf0, 0xb9, 0x3a),
        _ => Color32::from_rgb(0xe8, 0x5a, 0x5a),
    }
}

/// Simple line chart: x range, y from 0 to the highest value, optional vertical markers with labels.
fn line_chart(ui: &mut Ui, w: f32, h: f32, x: (f32, f32), series: &[(Color32, Vec<(f32, f32)>)], marks: &[(f32, String)]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let plot = egui::Rect::from_min_max(rect.min + egui::vec2(44.0, 8.0), rect.max - egui::vec2(10.0, 22.0));
    let ymax = series.iter().flat_map(|s| s.1.iter().map(|q| q.1)).fold(1.0f32, f32::max) * 1.05;
    let span = (x.1 - x.0).max(1.0);
    let at = |px: f32, py: f32| egui::pos2(plot.left() + (px - x.0) / span * plot.width(), plot.bottom() - py / ymax * plot.height());
    let grid = Stroke::new(1.0, CARD);
    for k in 0..=4 {
        let v = ymax / 4.0 * k as f32;
        let y = at(x.0, v).y;
        p.line_segment([egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)], grid);
        p.text(egui::pos2(plot.left() - 6.0, y), egui::Align2::RIGHT_CENTER, format!("{}", v.round()), egui::FontId::proportional(11.0), MUTED);
    }
    let mut lv = (x.0 / 10.0).ceil() * 10.0;
    while lv <= x.1 {
        let px = at(lv, 0.0).x;
        p.line_segment([egui::pos2(px, plot.top()), egui::pos2(px, plot.bottom())], grid);
        p.text(egui::pos2(px, plot.bottom() + 4.0), egui::Align2::CENTER_TOP, format!("Lv {lv}"), egui::FontId::proportional(11.0), MUTED);
        lv += 10.0;
    }
    for (mx, name) in marks {
        let px = at(*mx, 0.0).x;
        p.line_segment([egui::pos2(px, plot.top()), egui::pos2(px, plot.bottom())], Stroke::new(1.5, Color32::from_rgb(0xff, 0xd0, 0x60)));
        p.text(egui::pos2(px + 4.0, plot.top() + 2.0), egui::Align2::LEFT_TOP, format!("{name} (Lv {mx})"), egui::FontId::proportional(12.0), Color32::from_rgb(0xff, 0xd0, 0x60));
    }
    for (col, pts) in series {
        let line: Vec<egui::Pos2> = pts.iter().map(|&(a, b)| at(a, b)).collect();
        p.add(egui::Shape::line(line, Stroke::new(2.0, *col)));
    }
}

/// Small colour square for chart legends.
fn swatch(ui: &mut Ui, col: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect.shrink(1.0), 2.0, col);
}

/// A rough role from the base stats (the tool's own wording, not a game term).
fn role(b: &[u16; 6], l: Lang) -> &'static str {
    let (atk, def, spa, spd, spe, hp) = (b[1] as f32, b[2] as f32, b[3] as f32, b[4] as f32, b[5] as f32, b[0] as f32);
    if hp + def + spd > 1.15 * (atk + spa + spe) { return t(l, "Wall", "Verteidiger"); }
    let kind = if atk > spa * 1.2 { 1 } else if spa > atk * 1.2 { 2 } else { 0 };
    match (spe >= 90.0 && atk.max(spa) >= 80.0, kind) {
        (true, 1) => t(l, "Fast physical attacker", "Schneller physischer Angreifer"),
        (true, 2) => t(l, "Fast special attacker", "Schneller spezieller Angreifer"),
        (true, _) => t(l, "Fast all-rounder", "Schneller Allrounder"),
        (false, 1) => t(l, "Physical attacker", "Physischer Angreifer"),
        (false, 2) => t(l, "Special attacker", "Spezieller Angreifer"),
        _ => t(l, "All-rounder", "Allrounder"),
    }
}

/// Spider-web chart: one spoke per label, values 0..1, each series as a filled outline.
fn radar(ui: &mut Ui, size: f32, labels: &[String], series: &[(Color32, Vec<f32>)]) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size * 1.7, size * 1.15), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let (c, r, n) = (rect.center(), size * 0.4, labels.len());
    let pt = |i: usize, v: f32| {
        let a = -FRAC_PI_2 + TAU * i as f32 / n as f32;
        c + egui::vec2(a.cos(), a.sin()) * r * v
    };
    let grid = Stroke::new(1.0, CARD_HI);
    for ring in [0.25, 0.5, 0.75, 1.0] {
        p.add(egui::Shape::closed_line((0..n).map(|i| pt(i, ring)).collect(), grid));
    }
    for (i, label) in labels.iter().enumerate() {
        p.line_segment([c, pt(i, 1.0)], grid);
        let lp = pt(i, 1.1);
        let align = if (lp.x - c.x).abs() < 6.0 { egui::Align2::CENTER_CENTER } else if lp.x > c.x { egui::Align2::LEFT_CENTER } else { egui::Align2::RIGHT_CENTER };
        p.text(lp, align, label, egui::FontId::proportional(13.0), MUTED);
    }
    for (col, vals) in series {
        let pts: Vec<egui::Pos2> = vals.iter().enumerate().map(|(i, &v)| pt(i, v.clamp(0.0, 1.0))).collect();
        // Fan from the centre: fills the (often non-convex) web shape correctly.
        let mut mesh = egui::Mesh::default();
        let fill = col.gamma_multiply(0.22);
        mesh.colored_vertex(c, fill);
        for q in &pts { mesh.colored_vertex(*q, fill); }
        for i in 0..n { mesh.add_triangle(0, 1 + i as u32, 1 + ((i + 1) % n) as u32); }
        p.add(egui::Shape::mesh(mesh));
        p.add(egui::Shape::closed_line(pts.clone(), Stroke::new(2.0, *col)));
        for q in pts { p.circle_filled(q, 3.0, *col); }
    }
}

/// Placeholder portrait for a hidden trainer: a dark disc with a question mark.
fn mystery(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter();
    p.circle_filled(rect.center(), size * 0.38, CARD_HI);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(size * 0.4), MUTED);
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
    let names = stat_short(l);
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
