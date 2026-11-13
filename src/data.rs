//! Reference data: PokeAPI CSVs and boss JSON, embedded at compile time so the tool is one file.
use serde::Deserialize;
use std::collections::HashMap;

pub const LANG_DE: u16 = 6;
pub const LANG_EN: u16 = 9;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    pub fn pick<'a>(self, en: &'a str, de: &'a str) -> &'a str {
        if self == Lang::De && !de.is_empty() { de } else { en }
    }
}

pub struct Species {
    pub id: u16,
    pub gn: u8,
    pub name_en: String,
    pub name_de: String,
    types: Vec<u8>,
    types_past: Vec<(u8, Vec<u8>)>, // (last gn these types applied, types)
    pub base_stats: [u16; 6],
}

pub struct Move {
    pub id: u16,
    pub ident: String,
    pub name_en: String,
    pub name_de: String,
    type_id: u8,
    power: Option<u16>,
    /// None = never misses
    pub accuracy: Option<u8>,
    pub class: u8, // 1 status, 2 physical, 3 special
    changes: Vec<(u16, Option<u8>, Option<u16>)>, // (changed-in version group order, old type, old power)
}

pub struct TypeInfo {
    pub id: u8,
    pub gn: u8,
    pub name_en: String,
    pub name_de: String,
}

pub struct VersionGroup {
    pub id: u16,
    /// e.g. ("HeartGold / SoulSilver", "HeartGold / SoulSilver")
    pub name_en: String,
    pub name_de: String,
    pub ident: String,
    pub gn: u8,
    pub order: u16,
}

#[derive(Deserialize, Clone)]
pub struct BossMon {
    pub dex: u16,
    pub level: u8,
    #[serde(default)]
    pub moves: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct Boss {
    pub kind: String,
    pub name_en: String,
    pub name_de: String,
    #[serde(rename = "type", default)]
    pub type_: Option<String>,
    #[serde(default)]
    pub location_en: Option<String>,
    #[serde(default)]
    pub location_de: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub team: Vec<BossMon>,
}

#[derive(Deserialize)]
pub struct GameBosses {
    pub id: String,
    pub bosses: Vec<Boss>,
}

#[derive(Deserialize)]
struct BossFile {
    games: Vec<GameBosses>,
}

pub struct Dex {
    pub species: HashMap<u16, Species>,
    pub moves: HashMap<u16, Move>,
    pub types: Vec<TypeInfo>,
    efficacy: HashMap<(u8, u8), u8>,
    efficacy_past: HashMap<(u8, u8), Vec<(u8, u8)>>, // (last gn, factor)
    pub version_groups: Vec<VersionGroup>,
    pub bosses: Vec<GameBosses>,
    move_by_ident: HashMap<String, u16>,
}

macro_rules! csv_file {
    ($name:literal) => {
        include_str!(concat!("../data/pokeapi/", $name, ".csv"))
    };
}

fn rows(text: &str) -> impl Iterator<Item = HashMap<String, String>> + '_ {
    let mut r = csv::Reader::from_reader(text.as_bytes());
    let headers: Vec<String> = r.headers().expect("csv header").iter().map(String::from).collect();
    r.into_records().filter_map(Result::ok).map(move |rec| {
        headers.iter().cloned().zip(rec.iter().map(String::from)).collect()
    })
}

fn num<T: std::str::FromStr>(row: &HashMap<String, String>, col: &str) -> Option<T> {
    row.get(col).and_then(|v| v.parse().ok())
}

/// Lower-case and drop everything but letters and digits, so "Mr. Mime" == "mrmime".
pub fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

impl Dex {
    pub fn load() -> Dex {
        let mut names: HashMap<(u16, u16), String> = HashMap::new();
        for r in rows(csv_file!("pokemon_species_names")) {
            if let (Some(id), Some(l)) = (num(&r, "pokemon_species_id"), num(&r, "local_language_id")) {
                names.insert((id, l), r["name"].clone());
            }
        }
        let mut species: HashMap<u16, Species> = HashMap::new();
        for r in rows(csv_file!("pokemon_species")) {
            let id: u16 = num(&r, "id").unwrap();
            species.insert(id, Species {
                id,
                gn: num(&r, "generation_id").unwrap_or(1),
                name_en: names.get(&(id, LANG_EN)).cloned().unwrap_or_else(|| r["identifier"].clone()),
                name_de: names.get(&(id, LANG_DE)).cloned().unwrap_or_default(),
                types: vec![],
                types_past: vec![],
                base_stats: [0; 6],
            });
        }
        // Default form of species N is pokemon id N, so these ids line up for every species.
        for r in rows(csv_file!("pokemon_types")) {
            if let Some(s) = species.get_mut(&num(&r, "pokemon_id").unwrap()) {
                s.types.push(num(&r, "type_id").unwrap());
            }
        }
        for r in rows(csv_file!("pokemon_types_past")) {
            if let Some(s) = species.get_mut(&num(&r, "pokemon_id").unwrap()) {
                let g: u8 = num(&r, "generation_id").unwrap();
                let t: u8 = num(&r, "type_id").unwrap();
                match s.types_past.iter_mut().find(|(pg, _)| *pg == g) {
                    Some((_, v)) => v.push(t),
                    None => s.types_past.push((g, vec![t])),
                }
            }
        }
        for r in rows(csv_file!("pokemon_stats")) {
            if let Some(s) = species.get_mut(&num(&r, "pokemon_id").unwrap()) {
                let stat: usize = num(&r, "stat_id").unwrap();
                if (1..=6).contains(&stat) {
                    s.base_stats[stat - 1] = num(&r, "base_stat").unwrap();
                }
            }
        }

        let mut type_names: HashMap<(u8, u16), String> = HashMap::new();
        for r in rows(csv_file!("type_names")) {
            if let (Some(id), Some(l)) = (num(&r, "type_id"), num(&r, "local_language_id")) {
                type_names.insert((id, l), r["name"].clone());
            }
        }
        let types = rows(csv_file!("types"))
            .filter_map(|r| {
                let id: u16 = num(&r, "id")?;
                // 19 = Stellar (Tera only), 10001+ = unknown/shadow: not real battle types.
                if id > 18 { return None; }
                let id = id as u8;
                Some(TypeInfo {
                    id,
                    gn: num(&r, "generation_id").unwrap_or(1),
                    name_en: type_names.get(&(id, LANG_EN)).cloned().unwrap_or_else(|| r["identifier"].clone()),
                    name_de: type_names.get(&(id, LANG_DE)).cloned().unwrap_or_default(),
                })
            })
            .collect();
        let efficacy = rows(csv_file!("type_efficacy"))
            .map(|r| ((num(&r, "damage_type_id").unwrap(), num(&r, "target_type_id").unwrap()), num(&r, "damage_factor").unwrap()))
            .collect();
        let mut efficacy_past: HashMap<(u8, u8), Vec<(u8, u8)>> = HashMap::new();
        for r in rows(csv_file!("type_efficacy_past")) {
            efficacy_past
                .entry((num(&r, "damage_type_id").unwrap(), num(&r, "target_type_id").unwrap()))
                .or_default()
                .push((num(&r, "generation_id").unwrap(), num(&r, "damage_factor").unwrap()));
        }

        let mut version_names: HashMap<(u16, u16), String> = HashMap::new();
        for r in rows(csv_file!("version_names")) {
            if let (Some(id), Some(l)) = (num(&r, "version_id"), num(&r, "local_language_id")) {
                version_names.insert((id, l), r["name"].clone());
            }
        }
        let mut vg_versions: HashMap<u16, Vec<u16>> = HashMap::new();
        for r in rows(csv_file!("versions")) {
            vg_versions.entry(num(&r, "version_group_id").unwrap()).or_default().push(num(&r, "id").unwrap());
        }
        let vg_name = |vg: u16, lang: u16| -> String {
            vg_versions.get(&vg).into_iter().flatten()
                .filter_map(|v| version_names.get(&(*v, lang)).or(version_names.get(&(*v, LANG_EN))))
                .cloned().collect::<Vec<_>>().join(" / ")
        };
        let version_groups: Vec<VersionGroup> = rows(csv_file!("version_groups"))
            .map(|r| VersionGroup {
                id: num(&r, "id").unwrap(),
                name_en: vg_name(num(&r, "id").unwrap(), LANG_EN),
                name_de: vg_name(num(&r, "id").unwrap(), LANG_DE),
                ident: r["identifier"].clone(),
                gn: num(&r, "generation_id").unwrap(),
                order: num(&r, "order").unwrap(),
            })
            .collect();
        let vg_order: HashMap<u16, u16> = version_groups.iter().map(|v| (v.id, v.order)).collect();

        let mut move_names: HashMap<(u16, u16), String> = HashMap::new();
        for r in rows(csv_file!("move_names")) {
            if let (Some(id), Some(l)) = (num(&r, "move_id"), num(&r, "local_language_id")) {
                move_names.insert((id, l), r["name"].clone());
            }
        }
        let mut moves: HashMap<u16, Move> = HashMap::new();
        let mut move_by_ident = HashMap::new();
        for r in rows(csv_file!("moves")) {
            let id: u16 = num(&r, "id").unwrap();
            if id >= 10000 { continue; } // shadow moves (Colosseum/XD)
            move_by_ident.insert(r["identifier"].clone(), id);
            moves.insert(id, Move {
                id,
                ident: r["identifier"].clone(),
                name_en: move_names.get(&(id, LANG_EN)).cloned().unwrap_or_else(|| r["identifier"].clone()),
                name_de: move_names.get(&(id, LANG_DE)).cloned().unwrap_or_default(),
                type_id: num(&r, "type_id").unwrap_or(1),
                power: num(&r, "power"),
                accuracy: num(&r, "accuracy"),
                class: num(&r, "damage_class_id").unwrap_or(1),
                changes: vec![],
            });
        }
        // Each changelog row holds the values a move had BEFORE the given version group.
        for r in rows(csv_file!("move_changelog")) {
            if let Some(m) = moves.get_mut(&num(&r, "move_id").unwrap()) {
                let vg: u16 = num(&r, "changed_in_version_group_id").unwrap();
                m.changes.push((vg_order[&vg], num(&r, "type_id"), num(&r, "power")));
            }
        }
        for m in moves.values_mut() {
            m.changes.sort_by_key(|c| c.0);
        }

        let mut bosses = vec![];
        for text in [include_str!("../data/bosses/gen1-4.json"), include_str!("../data/bosses/gen5-9.json")] {
            let f: BossFile = serde_json::from_str(text).expect("boss json");
            bosses.extend(f.games);
        }

        Dex { species, moves, types, efficacy, efficacy_past, version_groups, bosses, move_by_ident }
    }

    pub fn vg(&self, ident: &str) -> Option<&VersionGroup> {
        self.version_groups.iter().find(|v| v.ident == ident)
    }

    pub fn types_in_gen(&self, gn: u8) -> Vec<u8> {
        self.types.iter().filter(|t| t.gn <= gn).map(|t| t.id).collect()
    }

    pub fn type_name(&self, id: u8, lang: Lang) -> &str {
        self.types.iter().find(|t| t.id == id).map(|t| lang.pick(&t.name_en, &t.name_de)).unwrap_or("?")
    }

    pub fn type_by_ident(&self, s: &str) -> Option<u8> {
        let n = norm(s);
        self.types.iter().find(|t| norm(&t.name_en) == n || norm(&t.name_de) == n).map(|t| t.id)
    }

    /// Damage factor in percent (0, 50, 100, 200) for one attacking vs one defending type in `gn`.
    pub fn factor(&self, atk: u8, def: u8, gn: u8) -> u8 {
        if let Some(past) = self.efficacy_past.get(&(atk, def)) {
            if let Some((_, f)) = past.iter().filter(|(g, _)| *g >= gn).min_by_key(|(g, _)| *g) {
                return *f;
            }
        }
        *self.efficacy.get(&(atk, def)).unwrap_or(&100)
    }

    /// Multiplier against a (possibly dual) typing.
    pub fn eff(&self, atk: u8, def: &[u8], gn: u8) -> f32 {
        def.iter().map(|&d| self.factor(atk, d, gn) as f32 / 100.0).product()
    }

    pub fn species_types(&self, s: &Species, gn: u8) -> Vec<u8> {
        s.types_past
            .iter()
            .filter(|(g, _)| *g >= gn)
            .min_by_key(|(g, _)| *g)
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| s.types.clone())
    }

    /// (type, power) of a move as it was in the given version group.
    pub fn move_stats(&self, m: &Move, vg: &VersionGroup) -> (u8, Option<u16>) {
        let later = || m.changes.iter().filter(|c| c.0 > vg.order);
        let t = later().find_map(|c| c.1).unwrap_or(m.type_id);
        let p = later().find_map(|c| c.2).or(m.power);
        (t, p)
    }

    #[cfg(test)]
    pub fn find_species(&self, name: &str) -> Option<&Species> {
        let n = norm(name);
        if let Ok(id) = name.trim().parse::<u16>() {
            return self.species.get(&id);
        }
        self.species.values().find(|s| norm(&s.name_en) == n || norm(&s.name_de) == n)
    }

    #[cfg(test)]
    pub fn find_move(&self, name: &str) -> Option<&Move> {
        let n = norm(name);
        self.move_by_ident
            .get(name.trim())
            .and_then(|id| self.moves.get(id))
            .or_else(|| self.moves.values().find(|m| norm(&m.name_en) == n || norm(&m.name_de) == n))
    }

    pub fn species_name(&self, id: u16, lang: Lang) -> String {
        self.species.get(&id).map(|s| lang.pick(&s.name_en, &s.name_de).to_string()).unwrap_or(format!("#{id}"))
    }

    pub fn move_name(&self, id: u16, lang: Lang) -> String {
        self.moves.get(&id).map(|m| lang.pick(&m.name_en, &m.name_de).to_string()).unwrap_or(format!("move {id}"))
    }

    pub fn move_id(&self, ident: &str) -> Option<u16> {
        self.move_by_ident.get(ident).copied()
    }
}

/// What each species can learn in one version group.
#[derive(Default)]
pub struct Learnsets {
    pub level_up: HashMap<u16, Vec<(u8, u16)>>,
    pub machine: HashMap<u16, Vec<u16>>,
    /// move id -> machine number as PokeAPI numbers it
    pub machine_no: HashMap<u16, u16>,
    /// species -> what it can evolve into, and how
    pub evolutions: HashMap<u16, Vec<(u16, EvoHow)>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EvoHow {
    Level(u8),
    Friendship,
    Item,
    Trade,
    Other,
}

/// Minimal CSV scan for the big learnset file (the generic `rows` is too slow for ~600k lines).
fn scan<'a>(text: &'a str, cols: &[&str]) -> impl Iterator<Item = Vec<&'a str>> + 'a {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").trim_end().split(',').collect();
    let idx: Vec<usize> = cols.iter().map(|c| head.iter().position(|h| h == c).expect("csv column")).collect();
    lines.map(move |l| {
        let f: Vec<&str> = l.trim_end().split(',').collect();
        idx.iter().map(|&i| *f.get(i).unwrap_or(&"")).collect()
    })
}

impl Dex {
    pub fn learnsets(&self, vg: &VersionGroup) -> Learnsets {
        let vg_id = vg.id.to_string();
        let mut ls = Learnsets::default();
        for f in scan(csv_file!("pokemon_moves"), &["pokemon_id", "version_group_id", "move_id", "pokemon_move_method_id", "level"]) {
            if f[1] != vg_id { continue; }
            let (Ok(sp), Ok(mv)) = (f[0].parse::<u16>(), f[2].parse::<u16>()) else { continue };
            match f[3] {
                "1" => ls.level_up.entry(sp).or_default().push((f[4].parse().unwrap_or(0), mv)),
                "4" => ls.machine.entry(sp).or_default().push(mv),
                _ => {}
            }
        }
        for f in scan(csv_file!("machines"), &["machine_number", "version_group_id", "move_id"]) {
            if f[1] == vg_id {
                if let (Ok(n), Ok(mv)) = (f[0].parse(), f[2].parse()) { ls.machine_no.insert(mv, n); }
            }
        }
        let from: HashMap<u16, u16> = scan(csv_file!("pokemon_species"), &["id", "evolves_from_species_id"])
            .filter_map(|f| Some((f[0].parse().ok()?, f[1].parse().ok()?)))
            .collect();
        let cols = ["evolved_species_id", "evolution_trigger_id", "minimum_level", "required_pokemon_form_id"];
        for f in scan(csv_file!("pokemon_evolution"), &cols) {
            // Regional forms (form ids 10000+) evolve differently; the plain species is what older games have.
            if f[3].parse::<u32>().is_ok_and(|x| x >= 10000) { continue; }
            let Ok(to) = f[0].parse::<u16>() else { continue };
            let Some(&pre) = from.get(&to) else { continue };
            if self.species.get(&to).is_none_or(|s| s.gn > vg.gn) { continue; }
            let how = match (f[1], f[2].parse::<u8>()) {
                ("1", Ok(l)) => EvoHow::Level(l),
                ("1", _) => EvoHow::Friendship, // shortcut: level-up without a level is mostly friendship; also time/place ones
                ("2", _) => EvoHow::Trade,
                ("3", _) => EvoHow::Item,
                _ => EvoHow::Other,
            };
            let list = ls.evolutions.entry(pre).or_default();
            if !list.iter().any(|e| e.0 == to) { list.push((to, how)); }
        }
        ls
    }
}
