//! Read real boss teams from a HeartGold/SoulSilver ROM (works for ROM hacks too).
//! Layout: research/save-formats.md part D (verified on Max's SoulSilver hard mod).
use crate::data::EvoHow;
use crate::data::{norm, Boss, BossMon, Dex};

fn u16le(d: &[u8], o: usize) -> u16 { u16::from_le_bytes([d[o], d[o + 1]]) }
fn u32le(d: &[u8], o: usize) -> usize { u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize }

/// Resolve a path like "a/0/5/5" through the NDS file name table and return the file bytes.
fn nds_file<'a>(rom: &'a [u8], path: &str) -> Option<&'a [u8]> {
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
            let name = &rom[p + 1..p + 1 + len];
            p += 1 + len;
            let is_dir = t & 0x80 != 0;
            if is_dir {
                let id = u16le(rom, p) as usize & 0xFFF;
                p += 2;
                if !last && name == part.as_bytes() { dir = id; break; }
            } else {
                if last && name == part.as_bytes() {
                    let (s, e) = (u32le(rom, fat + 8 * file_id), u32le(rom, fat + 8 * file_id + 4));
                    return rom.get(s..e);
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
            n += 1;
        }
    }
    Ok(n)
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
                1..=3 => EvoHow::Friendship,
                6 | 7 | 16..=19 => EvoHow::Item,
                5 => EvoHow::Trade,
                _ => EvoHow::Other,
            };
            Some((to, how))
        }).collect();
        (!list.is_empty()).then_some((sp, list))
    }).collect())
}
