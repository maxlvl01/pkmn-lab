//! Read real boss teams from a HeartGold/SoulSilver ROM (works for ROM hacks too).
//! Layout: research/save-formats.md part D (verified on Max's SoulSilver hard mod).
use crate::data::{CatchHow, Daytime, EvoHow, WildSpot};
use std::collections::{HashMap, HashSet};
use crate::data::{norm, Boss, BossMon, Dex};

fn u16le(d: &[u8], o: usize) -> u16 { u16::from_le_bytes([d[o], d[o + 1]]) }
fn u32le(d: &[u8], o: usize) -> usize { u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize }

/// Resolve a path like "a/0/5/5" through the NDS file name table and return the file bytes.
fn nds_file<'a>(rom: &'a [u8], path: &str) -> Option<&'a [u8]> {
    let (s, e) = nds_range(rom, path)?;
    rom.get(s..e)
}

/// Where a file sits in the ROM. `rom` only needs to reach the end of the name and allocation tables.
fn nds_range(rom: &[u8], path: &str) -> Option<(usize, usize)> {
    let (fnt, fat) = (u32le(rom, 0x40), u32le(rom, 0x48));
    let mut dir = 0usize;
    let parts: Vec<&str> = path.split('/').collect();
    for (depth, part) in parts.iter().enumerate() {
        let mut p = fnt + u32le(rom, fnt + 8 * dir);
        let mut file_id = u16le(rom, fnt + 8 * dir + 4) as usize;
        let last = depth == parts.len() - 1;
        loop {
            let t = *rom.get(p)?;
            if t == 0 { return None; }
            let len = (t & 0x7F) as usize;
            let name = rom.get(p + 1..p + 1 + len)?;
            p += 1 + len;
            let is_dir = t & 0x80 != 0;
            if is_dir {
                let id = u16le(rom, p) as usize & 0xFFF;
                p += 2;
                if !last && name == part.as_bytes() { dir = id; break; }
            } else {
                if last && name == part.as_bytes() {
                    rom.get(fat + 8 * file_id..fat + 8 * file_id + 8)?;
                    return Some((u32le(rom, fat + 8 * file_id), u32le(rom, fat + 8 * file_id + 4)));
                }
                file_id += 1;
            }
        }
    }
    None
}

/// Member `i` of a NARC archive.
fn narc(data: &[u8], i: usize) -> Option<&[u8]> {
    if !data.starts_with(b"NARC") || &data[0x10..0x14] != b"BTAF" { return None; }
    let count = u16le(data, 0x18) as usize;
    if i >= count { return None; }
    let btnf = 0x10 + u32le(data, 0x14);
    let gmif = btnf + u32le(data, btnf + 4);
    let (s, e) = (u32le(data, 0x1C + 8 * i), u32le(data, 0x20 + 8 * i));
    data.get(gmif + 8 + s..gmif + 8 + e)
}

/// HGSS trainer ids (pret pokeheartgold include/constants/trainers.h), keyed by the English name in our boss data.
const HGSS_TRAINERS: [(&str, usize); 22] = [
    ("falkner", 20), ("bugsy", 21), ("whitney", 30), ("morty", 31), ("pryce", 32), ("jasmine", 33), ("chuck", 34),
    ("clair", 35), ("will", 245), ("koga", 247), ("bruno", 418), ("karen", 246), ("lance", 244), ("brock", 253),
    ("misty", 254), ("ltsurge", 255), ("erika", 256), ("janine", 257), ("sabrina", 258), ("blaine", 259),
    ("blue", 261), ("red", 260),
];

fn trainer_team(trdata: &[u8], trpoke: &[u8], id: usize, dex: &Dex) -> Option<Vec<BossMon>> {
    let head = narc(trdata, id)?;
    let party = narc(trpoke, id)?;
    let (kind, npoke) = (head[0], head[3] as usize);
    // HGSS records end in a u16 ball-capsule field, hence 8/16/10/18 rather than DPPt's 6/14/8/16.
    let size = 8 + if kind & 1 != 0 { 8 } else { 0 } + if kind & 2 != 0 { 2 } else { 0 };
    let mut team = vec![];
    for i in 0..npoke {
        let r = party.get(i * size..(i + 1) * size)?;
        let moves_at = 6 + if kind & 2 != 0 { 2 } else { 0 };
        let moves = if kind & 1 != 0 {
            (0..4)
                .map(|k| u16le(r, moves_at + 2 * k))
                .filter(|&m| m != 0)
                .filter_map(|m| dex.moves.get(&m).map(|mv| mv.ident.clone()))
                .collect()
        } else {
            vec![] // default moves aren't stored; the analysis falls back to the species' own types
        };
        team.push(BossMon { dex: u16le(r, 4) & 0x3FF, level: u16le(r, 2).min(255) as u8, moves });
    }
    Some(team)
}

/// Replace the boss teams with the ones in the ROM. Returns how many bosses were updated.
pub fn apply_rom_teams(rom: &[u8], game_id: &str, bosses: &mut [Boss], dex: &Dex) -> Result<usize, String> {
    let code = rom.get(0x0C..0x0F).ok_or("not an NDS ROM")?;
    if game_id != "heartgold-soulsilver" || !(code == b"IPK" || code == b"IPG") {
        return Err("reading boss teams from the ROM works for HeartGold/SoulSilver only so far".into());
    }
    let trdata = nds_file(rom, "a/0/5/5").ok_or("trainer data a/0/5/5 not found in ROM")?;
    let trpoke = nds_file(rom, "a/0/5/6").ok_or("trainer teams a/0/5/6 not found in ROM")?;
    let mut n = 0;
    for b in bosses.iter_mut() {
        let Some(&(_, id)) = HGSS_TRAINERS.iter().find(|(k, _)| *k == norm(&b.name_en)) else { continue };
        if let Some(team) = trainer_team(trdata, trpoke, id, dex) {
            b.team = team;
            b.id = Some(id as u16);
            n += 1;
        }
    }
    Ok(n)
}

/// Every other trainer in an HGSS ROM (routes, rival, Team Rocket, ...), named from the ROM's own text.
pub fn hgss_trainers(rom: &[u8], dex: &Dex) -> Vec<Boss> {
    let (Some(trdata), Some(trpoke), Some(msgs)) = (nds_file(rom, "a/0/5/5"), nds_file(rom, "a/0/5/6"), nds_file(rom, "a/0/2/7")) else {
        return vec![];
    };
    // Message files 729/730: trainer names and class names (found by decoding the German ROM).
    let names = narc(msgs, 729).map(gen4_messages).unwrap_or_default();
    let classes = narc(msgs, 730).map(gen4_messages).unwrap_or_default();
    let count = u16le(trdata, 0x18) as usize;
    (1..count).filter(|id| !HGSS_TRAINERS.iter().any(|t| t.1 == *id)).filter_map(|id| {
        let head = narc(trdata, id)?;
        let team = trainer_team(trdata, trpoke, id, dex)?;
        if team.is_empty() { return None; }
        let class = classes.get(head[1] as usize).cloned().unwrap_or_default().replace('⒆', "PKMN");
        // Not story fights: the Battle Frontier brains (placeholder teams here, their real ones live elsewhere) and
        // scripted passers-by. Class names as they appear in the German ROM.
        if ["Kampfkoloss", "Saaldiva", "Fabrikleiter", "Arkaden-Star", "Kammerdiener", "Passant"].iter().any(|c| class.starts_with(c)) {
            return None;
        }
        let name = format!("{class} {}", names.get(id).map(String::as_str).unwrap_or("")).trim().to_string();
        Some(Boss {
            kind: if class == "Rivale" { "rival".into() } else { "trainer".into() },
            name_en: name.clone(),
            name_de: name,
            type_: None,
            location_en: None,
            location_de: None,
            variant: None,
            team,
            id: Some(id as u16),
        })
    }).collect()
}

/// Wild Pokémon of an HGSS ROM: species -> places (names from the ROM's text), lowest level first.
/// Grass (morning/day/night), surfing, Zertrümmerer and the three rods; radio, swarms and headbutt trees are left out.
pub fn hgss_wild(rom: &[u8]) -> HashMap<u16, Vec<WildSpot>> {
    let mut out: HashMap<u16, Vec<WildSpot>> = HashMap::new();
    // Game code IPG* = SoulSilver, IPK* = HeartGold; each version has its own encounter archive.
    let path = if rom.get(0x0C..0x0F) == Some(b"IPG".as_slice()) { "a/1/3/6" } else { "a/0/3/7" };
    let (Some(encs), Some(msgs)) = (nds_file(rom, path), nds_file(rom, "a/0/2/7")) else { return out };
    if rom.len() < 0x30 { return out; }
    let places = narc(msgs, 279).map(gen4_messages).unwrap_or_default();
    let (a9, a9n) = (u32le(rom, 0x20), u32le(rom, 0x2C));
    let Some(arm9) = rom.get(a9..a9 + a9n) else { return out };
    let Some((start, rows)) = map_headers(arm9, u16le(encs, 0x18) as usize, places.len()) else { return out };
    let mut seen = HashSet::new();
    for h in 0..rows {
        let r = &arm9[start + 24 * h..start + 24 * h + 24];
        let (enc, sec) = (r[0] as usize, r[0x12] as usize);
        if enc == 0xFF || !seen.insert((enc, sec)) { continue; }
        let (Some(e), Some(place)) = (narc(encs, enc), places.get(sec)) else { continue };
        if e.len() < 0xC4 { continue; }
        // Unknown places count as after the League, so they never look early.
        let (walk, water) = HGSS_STORY.iter().find(|p| p.0 as usize == sec).map_or((16, 16), |p| (p.1, p.2));
        let mut add = |sp: u16, level: u8, how: CatchHow, badges: u8| {
            if sp != 0 && level > 0 { out.entry(sp).or_default().push(WildSpot { place: place.clone(), level, how, badges }); }
        };
        if e[0] > 0 {
            for k in 0..12 { for time in 0..3 { add(u16le(e, 0x14 + 24 * time + 2 * k), e[8 + k], CatchHow::Grass, walk); } }
        }
        // water / rock slots: min level, max level, species. Surfer needs badge 4, Zertrümmerer 3 (research/story-order),
        // Angel badge 1 (Route 32), Profiangel 4 (Oliviana), Superangel after the League (Kanto).
        for (rate, off, n, how, need) in [(e[1], 0x64, 5, CatchHow::Surf, water.max(4)), (e[2], 0x78, 2, CatchHow::RockSmash, walk.max(3)),
            (e[3], 0x80, 5, CatchHow::Rod, water.max(1)), (e[4], 0x94, 5, CatchHow::Rod, water.max(4)), (e[5], 0xA8, 5, CatchHow::Rod, 16)] {
            if rate == 0 { continue; }
            for k in 0..n { add(u16le(e, off + 4 * k + 2), e[off + 4 * k], how, need); }
        }
    }
    for v in out.values_mut() {
        // earliest per place and method, then earliest in the story first
        v.sort_by(|a, b| (&a.place, a.how as u8, a.badges, a.level).cmp(&(&b.place, b.how as u8, b.badges, b.level)));
        v.dedup_by(|a, b| a.place == b.place && a.how == b.how);
        v.sort_by_key(|s| (s.badges, s.level));
    }
    out
}

/// Place names of an HGSS ROM (index = place id, also used as met location).
pub fn hgss_places(rom: &[u8]) -> Vec<String> {
    nds_file(rom, "a/0/2/7").and_then(|m| narc(m, 279)).map(gen4_messages).unwrap_or_default()
}

/// HGSS story order: (place id, badges when you first walk there, badges for its water / gated parts); 16 = after the League.
/// shortcut: from walkthroughs plus a Sonnet subagent's game knowledge (research/story-order.csv), not from the game's own checks.
const HGSS_STORY: &[(u8, u8, u8)] = &[(126, 0, 0), (127, 0, 0), (128, 0, 0), (130, 4, 4), (132, 4, 4), (133, 3, 3), (135, 6, 6), (136, 7, 7), (137, 16, 16), (138, 16, 16), (139, 16, 16), (141, 16, 16), (143, 16, 16), (144, 16, 16), (145, 16, 16), (146, 16, 16), (149, 16, 16), (150, 16, 16), (151, 16, 16), (152, 16, 16), (153, 16, 16), (154, 16, 16), (155, 16, 16), (156, 16, 16), (157, 16, 16), (158, 16, 16), (159, 16, 16), (160, 16, 16), (161, 16, 16), (162, 16, 16), (163, 16, 16), (164, 16, 16), (165, 16, 16), (166, 16, 16), (167, 16, 16), (168, 16, 16), (169, 16, 16), (170, 8, 8), (171, 8, 8), (172, 16, 16), (173, 16, 16), (174, 8, 8), (175, 8, 8), (176, 16, 16), (177, 0, 0), (178, 0, 0), (179, 0, 0), (180, 1, 1), (181, 1, 1), (182, 2, 2), (183, 3, 3), (184, 3, 3), (185, 3, 3), (186, 4, 4), (187, 4, 4), (188, 4, 4), (189, 4, 4), (190, 3, 3), (191, 6, 6), (192, 7, 7), (193, 7, 7), (194, 0, 0), (195, 4, 4), (196, 4, 4), (197, 16, 16), (198, 16, 16), (199, 16, 16), (200, 16, 16), (202, 4, 4), (203, 16, 16), (204, 0, 0), (205, 8, 8), (206, 3, 3), (207, 3, 3), (209, 1, 4), (210, 1, 4), (211, 1, 1), (214, 1, 2), (216, 3, 8), (217, 7, 7), (218, 4, 7), (219, 16, 16), (220, 0, 4), (221, 8, 8), (222, 7, 7), (223, 8, 8), (224, 16, 16), (228, 4, 4), (234, 4, 4)];

/// The map header table in ARM9 (24-byte rows, pret's MapHeader): the longest run of rows that look like headers.
/// shortcut: a scan instead of a known address; verified on the German SoulSilver only, other ROMs may find nothing (no wild data then).
fn map_headers(arm9: &[u8], nenc: usize, nplaces: usize) -> Option<(usize, usize)> {
    let ok = |o: usize| arm9.get(o..o + 24).is_some_and(|r| {
        ((r[0] as usize) < nenc || r[0] == 0xFF) && (r[0x12] as usize) < nplaces && u16le(r, 0x0A) < 829
    });
    let mut best = (0, 0);
    for phase in (0..24).step_by(4) {
        let (mut start, mut n) = (phase, 0);
        let mut o = phase;
        while o + 24 <= arm9.len() {
            if ok(o) {
                if n == 0 { start = o; }
                n += 1;
                if n > best.1 { best = (start, n); }
            } else {
                n = 0;
            }
            o += 24;
        }
    }
    (best.1 >= 300).then_some(best)
}

/// Level-up learnsets from an HGSS ROM (a/0/3/3): species -> [(level, move id)].
/// Each entry is a u16 `move | level << 9`, list ends with 0xFFFF.
pub fn hgss_learnsets(rom: &[u8]) -> Option<std::collections::HashMap<u16, Vec<(u8, u16)>>> {
    let narc_data = nds_file(rom, "a/0/3/3")?;
    Some((1..=493u16).filter_map(|sp| {
        let b = narc(narc_data, sp as usize)?;
        let list = b.chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&w| w != 0xFFFF)
            .map(|w| ((w >> 9) as u8, w & 0x1FF))
            .collect();
        Some((sp, list))
    }).collect())
}


/// HGSS evolutions (a/0/3/4): per species 7 entries of (method, parameter, target), 44 bytes.
/// Hacks often turn trade evolutions into level ones, so the ROM's table wins over PokeAPI's.
pub fn hgss_evolutions(rom: &[u8]) -> Option<std::collections::HashMap<u16, Vec<(u16, EvoHow)>>> {
    let narc_data = nds_file(rom, "a/0/3/4")?;
    Some((1..=493u16).filter_map(|sp| {
        let b = narc(narc_data, sp as usize)?;
        let list: Vec<(u16, EvoHow)> = b.chunks_exact(6).take(7).filter_map(|e| {
            let w = |i: usize| u16::from_le_bytes([e[i], e[i + 1]]);
            let (method, param, to) = (w(0), w(2), w(4));
            if method == 0 || to == 0 || to > 493 { return None; }
            let how = match method {
                4 | 8..=14 | 22 | 23 => EvoHow::Level(param.min(100) as u8),
                // EVO_FRIENDSHIP, _DAY, _NIGHT
                1 => EvoHow::Friendship(Daytime::Any),
                2 => EvoHow::Friendship(Daytime::Day),
                3 => EvoHow::Friendship(Daytime::Night),
                6 | 7 | 16..=19 => EvoHow::Item,
                5 => EvoHow::Trade,
                _ => EvoHow::Other,
            };
            Some((to, how))
        }).collect();
        (!list.is_empty()).then_some((sp, list))
    }).collect())
}

/// Gen 4 message file: every string is XOR-encrypted with a key that changes per character.
pub fn gen4_messages(file: &[u8]) -> Vec<String> {
    if file.len() < 4 { return vec![]; }
    let (count, seed) = (u16le(file, 0) as usize, u16le(file, 2) as u32);
    (0..count).map(|i| {
        let n = i as u32 + 1;
        let key = (seed.wrapping_mul(0x2FD).wrapping_mul(n)) & 0xFFFF;
        let key2 = key | (key << 16);
        let Some(entry) = file.get(4 + i * 8..12 + i * 8) else { return String::new() };
        let off = (u32::from_le_bytes(entry[0..4].try_into().unwrap()) ^ key2) as usize;
        let len = (u32::from_le_bytes(entry[4..8].try_into().unwrap()) ^ key2) as usize;
        let mut k = (0x91BD3u32.wrapping_mul(n)) & 0xFFFF;
        let units = (0..len.min(512)).map_while(|j| {
            let c = u16le(file.get(off..)?, 2 * j) as u32 ^ k;
            k = (k + 0x493D) & 0xFFFF;
            Some(c as u16)
        });
        crate::save::gen4_text(unpack9(units.collect()))
    }).collect()
}

/// Gen 5 message file: UTF-16 strings, each XOR-encrypted with a key that rotates per character.
fn gen5_messages(f: &[u8]) -> Option<Vec<String>> {
    let lines = u16le(f.get(..4)?, 2) as usize;
    let sec = u32le(f.get(..0x10)?, 0x0C);
    (0..lines).map(|i| {
        let e = sec + 4 + 8 * i;
        let (off, len) = (sec + u32le(f.get(..e + 4)?, e), u16le(f.get(..e + 6)?, e + 4) as usize);
        let mut key = (0x7C89u32 + 0x2983 * i as u32) & 0xFFFF;
        let mut units = vec![];
        for j in 0..len {
            let c = u16le(f.get(..off + 2 * j + 2)?, off + 2 * j) ^ key as u16;
            key = ((key << 3) | (key >> 13)) & 0xFFFF;
            if c == 0xFFFF { break; }
            if c < 0xF000 { units.push(c); } // skip control codes
        }
        Some(String::from_utf16_lossy(&units))
    }).collect()
}

/// German attack names from a German Pokémon Black/White ROM, the names PokeMMO shows.
/// Reads only the header tables and the text archive, not the whole ~256 MB file.
pub fn bw_move_names(path: &std::path::Path) -> Option<Vec<String>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; 0x200];
    file.read_exact(&mut head).ok()?;
    if !matches!(&head[0x0C..0x10], b"IRBD" | b"IRAD") { return None; } // German Black / White
    let end = (u32le(&head, 0x40) + u32le(&head, 0x44)).max(u32le(&head, 0x48) + u32le(&head, 0x4C));
    head.resize(end, 0);
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_exact(&mut head).ok()?;
    let (s, e) = nds_range(&head, "a/0/0/2")?;
    let mut narc_bytes = vec![0u8; e.checked_sub(s)?];
    file.seek(SeekFrom::Start(s as u64)).ok()?;
    file.read_exact(&mut narc_bytes).ok()?;
    gen5_messages(narc(&narc_bytes, 203)?) // file 203 = attack names, line = attack id
}

/// Names (e.g. trainer names) are stored packed: 0xF100, then 9-bit character codes in 15-bit chunks, 0x1FF = end.
fn unpack9(units: Vec<u16>) -> Vec<u16> {
    if units.first() != Some(&0xF100) { return units; }
    let (mut out, mut buf, mut bits) = (vec![], 0u32, 0);
    for &u in &units[1..] {
        buf |= ((u & 0x7FFF) as u32) << bits;
        bits += 15;
        while bits >= 9 {
            let c = (buf & 0x1FF) as u16;
            if c == 0x1FF { return out; }
            out.push(c);
            buf >>= 9;
            bits -= 9;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// Needs a German SoulSilver ROM; set PKMN_ROM to its path, otherwise the test is skipped.
    #[test]
    fn wild_pokemon_from_rom() {
        let Ok(path) = std::env::var("PKMN_ROM") else { return };
        let rom = std::fs::read(path).unwrap();
        let wild = super::hgss_wild(&rom);
        // Taubsi (16) on Route 29 from level 2; Hoothoot (163) there at night.
        assert!(wild[&16].iter().any(|w| w.place == "Route 29" && w.level == 2), "{:?}", wild.get(&16));
        assert!(wild[&163].iter().any(|w| w.place == "Route 29"));
        assert!(wild[&16].iter().any(|w| w.place == "Route 1" && w.late()));
        assert!(wild[&16].iter().any(|w| w.place == "Route 29" && w.badges == 0));
        assert!(wild.len() > 100, "only {} species", wild.len());
    }
}
