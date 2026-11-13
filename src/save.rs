//! Emulator save reading, Gen 1-7. Offsets: research/save-formats.md (from PKHeX, pret).
use crate::analysis::Mon;
use crate::data::Dex;
use std::collections::HashMap;
use std::sync::OnceLock;

pub struct SaveInfo {
    pub game: Option<String>,
    pub trainer: Option<String>,
    /// Badges in boss-list gym order (Johto then Kanto for Gen 2/HGSS).
    pub badges: Vec<bool>,
    pub team: Vec<Mon>,
    /// TM/HM numbers in the bag (PokeAPI numbering, HM01 = 101); None = not read for this gen yet.
    pub machines: Option<Vec<u16>>,
    /// Trainer ids (ROM numbering) already beaten; None = not read for this game.
    pub beaten_trainers: Option<Vec<u16>>,
}

fn u16le(d: &[u8], o: usize) -> u16 { u16::from_le_bytes([d[o], d[o + 1]]) }
fn u32le(d: &[u8], o: usize) -> u32 { u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) }
fn bits(b: u8) -> impl Iterator<Item = bool> { (0..8).map(move |i| b >> i & 1 == 1) }

const DESMUME_COOKIE: &[u8] = b"|-DESMUME SAVE-|";
const NOCASH: &[u8] = b"NocashGbaBackupMediaSavDataFile";
const RAW_SIZES: [usize; 8] = [0x8000, 0x10000, 0x20000, 0x80000, 0x65600, 0x76000, 0x6BE00, 0x6CC00];

/// Strip emulator headers/footers down to the raw cartridge save.
fn unwrap_container(f: &[u8]) -> Result<Vec<u8>, String> {
    if f.ends_with(DESMUME_COOKIE) && f.len() > 0x7A {
        return Ok(f[..f.len() - 0x7A].to_vec());
    }
    if f.starts_with(NOCASH) && f.len() >= 0x50 {
        return match u32le(f, 0x44) {
            0 => {
                let n = u32le(f, 0x48) as usize;
                f.get(0x4C..0x4C + n).map(<[u8]>::to_vec).ok_or("truncated No$GBA save".into())
            }
            _ => unrle(&f[0x50..], u32le(f, 0x4C) as usize),
        };
    }
    if RAW_SIZES.contains(&f.len()) {
        return Ok(f.to_vec());
    }
    // RTC footers (Gen 2 / Gen 3 emulators) are at most 0x30 bytes; anything a bit larger than a raw size is that.
    RAW_SIZES
        .iter()
        .find(|&&s| f.len() > s && f.len() <= s + 0x40)
        .map(|&s| f[..s].to_vec())
        .ok_or(format!("unknown save size {} bytes (Switch saves, Gen 8/9, are not supported yet)", f.len()))
}

fn unrle(mut p: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(size);
    let bad = || "broken No$GBA compressed save".to_string();
    while let Some((&c, rest)) = p.split_first() {
        p = rest;
        match c {
            0 => break,
            1..=0x7F => {
                let n = c as usize;
                out.extend_from_slice(p.get(..n).ok_or_else(bad)?);
                p = &p[n..];
            }
            0x80 => {
                let (&v, r) = p.split_first().ok_or_else(bad)?;
                let n = u16le(r, 0) as usize;
                out.extend(std::iter::repeat_n(v, n));
                p = &r[2..];
            }
            _ => {
                let (&v, r) = p.split_first().ok_or_else(bad)?;
                out.extend(std::iter::repeat_n(v, (c - 0x80) as usize));
                p = r;
            }
        }
    }
    if out.len() != size { return Err(bad()); }
    Ok(out)
}

pub fn read(file: &[u8], dex: &Dex) -> Result<SaveInfo, String> {
    let d = unwrap_container(file)?;
    let info = match d.len() {
        0x8000 | 0x10000 if gen3(&d).is_none() => gen12(&d),
        0x10000 | 0x20000 => gen3(&d),
        0x80000 => gen4(&d).or_else(|| gen5(&d)),
        0x65600 | 0x76000 | 0x6BE00 | 0x6CC00 => gen67(&d),
        _ => None,
    };
    let mut info = info.ok_or("could not recognise this save (no valid Gen 1-7 layout found)")?;
    info.team.retain(|m| dex.species.contains_key(&m.species) && m.species != 0);
    Ok(info)
}

// ---------------- Gen 1 / 2 ----------------

fn has_list(d: &[u8], off: usize, max: u8) -> bool {
    let c = d[off];
    c <= max && d[off + 1 + c as usize] == 0xFF
}

fn gb_text(b: &[u8]) -> String {
    b.iter()
        .take_while(|&&c| c != 0x50)
        .map(|&c| match c {
            0x80..=0x99 => (b'A' + c - 0x80) as char,
            0xA0..=0xB9 => (b'a' + c - 0xA0) as char,
            0xF6..=0xFF => (b'0' + c - 0xF6) as char,
            0xC0 => 'Ä', 0xC1 => 'Ö', 0xC2 => 'Ü', 0xC3 => 'ä', 0xC4 => 'ö', 0xC5 => 'ü',
            0x7F => ' ',
            _ => '?',
        })
        .collect()
}

const GEN1_TO_NATIONAL: [u8; 191] = [
    0, 112, 115, 32, 35, 21, 100, 34, 80, 2, 103, 108, 102, 88, 94, 29, 31, 104, 111, 131, 59, 151, 130, 90, 72, 92, 123, 120, 9, 127, 114, 0, 0, 58, 95, 22, 16, 79, 64, 75, 113, 67, 122, 106, 107, 24, 47, 54, 96, 76, 0, 126, 0, 125, 82, 109, 0, 56, 86, 50, 128, 0, 0, 0, 83, 48, 149, 0, 0, 0, 84, 60, 124, 146, 144, 145, 132, 52, 98, 0, 0, 0, 37, 38, 25, 26, 0, 0, 147, 148, 140, 141, 116, 117, 0, 0, 27, 28, 138, 139, 39, 40, 133, 136, 135, 134, 66, 41, 23, 46, 61, 62, 13, 14, 15, 0, 85, 57, 51, 49, 87, 0, 0, 10, 11, 12, 68, 0, 55, 97, 42, 150, 143, 129, 0, 0, 89, 0, 99, 91, 0, 101, 36, 110, 53, 105, 0, 93, 63, 65, 17, 18, 121, 1, 3, 73, 0, 118, 119, 0, 0, 0, 0, 77, 78, 19, 20, 33, 30, 74, 137, 142, 0, 81, 0, 0, 4, 7, 5, 8, 6, 0, 0, 0, 0, 43, 44, 45, 69, 70, 71,
];

// shortcut: international (non-Japanese/Korean) Gen 1/2 saves only; add JP offsets if someone needs them.
fn gen12(d: &[u8]) -> Option<SaveInfo> {
    // (party offset, johto badge byte, kanto badge byte or none, game)
    let (party, mon_size, badges, game) = if has_list(d, 0x2865, 6) && has_list(d, 0x2D10, 20) {
        (0x2865, 48, [Some(0x23E5), Some(0x23E6)], "crystal")
    } else if has_list(d, 0x288A, 6) && has_list(d, 0x2D6C, 20) {
        (0x288A, 48, [Some(0x23E4), Some(0x23E5)], "gold-silver")
    } else if has_list(d, 0x2F2C, 6) && has_list(d, 0x30C0, 20) {
        let yellow = if d[0x29C3] != 0 { d[0x29C3] == 0x54 } else { d[0x271C] != 0 };
        (0x2F2C, 44, [Some(0x2602), None], if yellow { "yellow" } else { "red-blue" })
    } else {
        return None;
    };
    let gen1 = mon_size == 44;
    let count = d[party] as usize;
    let team = (0..count)
        .filter(|&i| d[party + 1 + i] != 0xFD) // 0xFD = egg (Gen 2)
        .map(|i| {
            let m = party + 8 + mon_size * i;
            let species = if gen1 { *GEN1_TO_NATIONAL.get(d[m] as usize).unwrap_or(&0) as u16 } else { d[m] as u16 };
            let (moves_at, level_at) = if gen1 { (8, 0x21) } else { (2, 0x1F) };
            let nick_at = party + 8 + mon_size * 6 + 66 + 11 * i;
            Mon {
                species,
                level: d[m + level_at],
                moves: d[m + moves_at..m + moves_at + 4].iter().filter(|&&x| x != 0).map(|&x| x as u16).collect(),
                nickname: Some(gb_text(&d[nick_at..nick_at + 11])),
                details: None,
            }
        })
        .collect();
    let trainer = Some(gb_text(&d[if gen1 { 0x2598 } else { 0x200B }..][..11]));
    let badges = badges.iter().flatten().flat_map(|&o| bits(d[o])).collect();
    Some(SaveInfo { game: Some(game.into()), trainer, badges, team, machines: None, beaten_trainers: None })
}

// ---------------- Gen 3 ----------------

const BLOCK_POSITION: [[usize; 4]; 24] = [
    [0, 1, 2, 3], [0, 1, 3, 2], [0, 2, 1, 3], [0, 3, 1, 2], [0, 2, 3, 1], [0, 3, 2, 1],
    [1, 0, 2, 3], [1, 0, 3, 2], [2, 0, 1, 3], [3, 0, 1, 2], [2, 0, 3, 1], [3, 0, 2, 1],
    [1, 2, 0, 3], [1, 3, 0, 2], [2, 1, 0, 3], [3, 1, 0, 2], [2, 3, 0, 1], [3, 2, 0, 1],
    [1, 2, 3, 0], [1, 3, 2, 0], [2, 1, 3, 0], [3, 1, 2, 0], [2, 3, 1, 0], [3, 2, 1, 0],
];

fn unshuffle(data: &mut [u8], start: usize, block: usize, sv: usize) {
    let enc = data[start..start + 4 * block].to_vec();
    for (i, &src) in BLOCK_POSITION[sv].iter().enumerate() {
        data[start + i * block..start + (i + 1) * block].copy_from_slice(&enc[src * block..(src + 1) * block]);
    }
}

const GEN3_HOENN: [u16; 135] = [
    252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 290, 291, 292, 276, 277, 285, 286, 327, 278, 279, 283, 284, 320, 321, 300, 301, 352, 343, 344, 299, 324, 302, 339, 340, 370, 341, 342, 349, 350, 318, 319, 328, 329, 330, 296, 297, 309, 310, 322, 323, 363, 364, 365, 331, 332, 361, 362, 337, 338, 298, 325, 326, 311, 312, 303, 307, 308, 333, 334, 360, 355, 356, 315, 287, 288, 289, 316, 317, 357, 293, 294, 295, 366, 367, 368, 359, 353, 354, 336, 335, 369, 304, 305, 306, 351, 313, 314, 345, 346, 347, 348, 280, 281, 282, 371, 372, 373, 374, 375, 376, 377, 378, 379, 382, 383, 384, 380, 381, 385, 386, 358,
];

fn gen3_text(b: &[u8]) -> String {
    b.iter()
        .take_while(|&&c| c != 0xFF)
        .map(|&c| match c {
            0xBB..=0xD4 => (b'A' + c - 0xBB) as char,
            0xD5..=0xEE => (b'a' + c - 0xD5) as char,
            0xA1..=0xAA => (b'0' + c - 0xA1) as char,
            0xF1 => 'Ä', 0xF2 => 'Ö', 0xF3 => 'Ü', 0xF4 => 'ä', 0xF5 => 'ö', 0xF6 => 'ü',
            0x00 => ' ',
            _ => '?',
        })
        .collect()
}

fn gen3(d: &[u8]) -> Option<SaveInfo> {
    // Each slot: 14 sectors in rotated order; section id at 0xFF4, signature 0xFF8, save index 0xFFC.
    let slot = |base: usize| -> Option<(u32, [usize; 14])> {
        let mut map = [usize::MAX; 14];
        let mut index = 0;
        for s in 0..14 {
            let o = base + s * 0x1000;
            if o + 0x1000 > d.len() || u32le(d, o + 0xFF8) != 0x0801_2025 { return None; }
            let id = u16le(d, o + 0xFF4) as usize;
            *map.get_mut(id)? = o;
            index = u32le(d, o + 0xFFC);
        }
        map.iter().all(|&o| o != usize::MAX).then_some((index, map))
    };
    let (_, map) = [slot(0), if d.len() >= 0x1C000 { slot(0xE000) } else { None }]
        .into_iter()
        .flatten()
        .filter(|(i, _)| *i != u32::MAX)
        .max_by_key(|(i, _)| *i)?;
    let small = &d[map[0]..map[0] + 0xF80];
    let large: Vec<u8> = (1..=4).flat_map(|id| d[map[id]..map[id] + 0xF80].to_vec()).collect();
    let (game, party, flags, first_badge) = match u32le(small, 0xAC) {
        1 => ("firered-leafgreen", 0x34, 0xEE0, 0x820),
        k if k != 0 && small[0x890..0xF2C].iter().any(|&b| b != 0) => ("emerald", 0x234, 0x1270, 0x867),
        _ => ("ruby-sapphire", 0x234, 0x1220, 0x807),
    };
    let count = (large[party] as usize).min(6);
    let team = (0..count)
        .filter_map(|i| {
            let mut m = large[party + 4 + 100 * i..party + 4 + 100 * (i + 1)].to_vec();
            let pid = u32le(&m, 0);
            let key = pid ^ u32le(&m, 4);
            for o in (0x20..0x50).step_by(4) {
                let v = u32le(&m, o) ^ key;
                m[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
            unshuffle(&mut m, 0x20, 12, (pid % 24) as usize);
            if u32le(&m, 0x48) >> 30 & 1 == 1 { return None; } // egg
            let internal = u16le(&m, 0x20);
            let species = match internal {
                1..=251 => internal,
                277..=411 => GEN3_HOENN[internal as usize - 277],
                _ => 0,
            };
            Some(Mon {
                species,
                level: m[0x54],
                moves: (0..4).map(|k| u16le(&m, 0x2C + 2 * k)).filter(|&x| x != 0).collect(),
                nickname: Some(gen3_text(&m[8..18])),
                details: None,
            })
        })
        .collect();
    let badges = (0..8).map(|n| {
        let f = first_badge + n;
        large[flags + (f >> 3)] >> (f & 7) & 1 == 1
    }).collect();
    Some(SaveInfo { game: Some(game.into()), trainer: Some(gen3_text(&small[..8])), badges, team, machines: None, beaten_trainers: None })
}

// ---------------- Gen 4 / 5 (PK4/PK5) and 6 / 7 (PK6/PK7) ----------------

fn lcrng_crypt(m: &mut [u8], range: std::ops::Range<usize>, seed: u32) {
    let mut x = seed;
    for o in range.step_by(2) {
        x = x.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        let w = u16le(m, o) ^ (x >> 16) as u16;
        m[o..o + 2].copy_from_slice(&w.to_le_bytes());
    }
}

/// Decrypt a Gen 4-7 party Pokémon. `block` = 32 (Gen 4/5) or 56 (Gen 6/7).
fn decrypt_pk(raw: &[u8], block: usize) -> Vec<u8> {
    let mut m = raw.to_vec();
    let pid = u32le(&m, 0);
    let end = 8 + 4 * block;
    let seed = if block == 32 { u16le(&m, 6) as u32 } else { pid }; // Gen 4/5 seed the boxed part with the checksum
    lcrng_crypt(&mut m, 8..end, seed);
    lcrng_crypt(&mut m, end..raw.len(), pid);
    unshuffle(&mut m, 8, block, ((pid >> 13 & 31) % 24) as usize);
    m
}

fn gen4_charmap() -> &'static HashMap<u16, char> {
    static MAP: OnceLock<HashMap<u16, char>> = OnceLock::new();
    MAP.get_or_init(|| {
        let raw: HashMap<String, String> = serde_json::from_str(include_str!("../data/gen4-charmap.json")).unwrap();
        raw.iter()
            .filter_map(|(k, v)| Some((u16::from_str_radix(k, 16).ok()?, v.chars().next()?)))
            .collect()
    })
}

pub fn gen4_text(units: impl IntoIterator<Item = u16>) -> String {
    let map = gen4_charmap();
    units.into_iter()
        .take_while(|&c| c != 0xFFFF && c != 0)
        .map(|c| match c { 0x1DE => ' ', 0x1E1 | 0x1E2 => '\u{0}', _ => *map.get(&c).unwrap_or(&'?') })
        .filter(|&c| c != '\u{0}')
        .collect()
}

fn u16s(d: &[u8], o: usize, n: usize) -> Vec<u16> {
    (0..n).map(|i| u16le(d, o + 2 * i)).collect()
}

fn utf16(d: &[u8], o: usize, n: usize) -> String {
    let u: Vec<u16> = u16s(d, o, n).into_iter().take_while(|&c| c != 0 && c != 0xFFFF).collect();
    String::from_utf16_lossy(&u).replace('\u{E08E}', "♂").replace('\u{E08F}', "♀")
}

/// Party mon from a decrypted PK4-PK7 record (field offsets differ between the two layouts).
fn pk_mon(m: &[u8], gen6: bool) -> Option<Mon> {
    let (moves_at, iv_at, level_at) = if gen6 { (0x5A, 0x74, 0xEC) } else { (0x28, 0x38, 0x8C) };
    if u32le(m, iv_at) >> 30 & 1 == 1 { return None; } // egg
    Some(Mon {
        species: u16le(m, 8),
        level: m[level_at],
        moves: (0..4).map(|k| u16le(m, moves_at + 2 * k)).filter(|&x| x != 0).collect(),
        nickname: None,
        details: None,
    })
}

/// Nature, DVs, Fleißpunkte, stats and more from a decrypted PK4 party record (PKHeX PK4 layout).
fn pk4_details(m: &[u8]) -> crate::analysis::Details {
    let pid = u32le(m, 0);
    let iv = u32le(m, 0x38);
    let ivr = |k: u32| (iv >> (5 * k) & 31) as u8;
    // The save orders stats KP, Angriff, Vert., Init., Sp. Ang., Sp. Ver.; the tool uses the base-stat order.
    let ivs = [ivr(0), ivr(1), ivr(2), ivr(4), ivr(5), ivr(3)];
    let evs = [m[0x18], m[0x19], m[0x1A], m[0x1C], m[0x1D], m[0x1B]];
    let st = |o: usize| u16le(m, o);
    let (tid, sid) = (u16le(m, 0x0C) as u32, u16le(m, 0x0E) as u32);
    let met = if u16le(m, 0x46) != 0 { u16le(m, 0x46) } else { u16le(m, 0x80) };
    crate::analysis::Details {
        nature: (pid % 25) as u8,
        ivs,
        evs,
        stats: [st(0x90), st(0x92), st(0x94), st(0x98), st(0x9A), st(0x96)],
        hp_now: st(0x8E),
        ability: m[0x15] as u16,
        item: u16le(m, 0x0A),
        ball: m[0x83].max(m[0x86]) as u16, // DPPt ball / HGSS ball (Aprikoko balls only in the second)
        friendship: m[0x14],
        exp: u32le(m, 0x10),
        met_level: m[0x84] & 0x7F,
        met_place: met,
        shiny: (tid ^ sid ^ (pid >> 16) ^ (pid & 0xFFFF)) < 8,
        pokerus: m[0x82],
        pp: [m[0x30], m[0x31], m[0x32], m[0x33]],
        pp_ups: [m[0x34], m[0x35], m[0x36], m[0x37]],
    }
}

fn gen4(d: &[u8]) -> Option<SaveInfo> {
    // (general block size, trainer block, party count offset, TM/HM pocket, game id)
    let (size, trainer, party, tm_pocket, game) = [
        (0xC100, 0x64, 0x94, 0x980, "diamond-pearl"),
        (0xCF2C, 0x68, 0x9C, 0x98C, "platinum"),
        (0xF628, 0x64, 0x94, 0x9A0, "heartgold-soulsilver"),
    ]
        .into_iter()
        .find(|&(s, ..)| {
            let e = 0x40000 + s;
            u32le(d, e - 0xC) == s as u32 && matches!(u32le(d, e - 8), 0x2006_0623 | 0x2007_0903)
        })?;
    // Newer copy = bigger save counter; 0xFFFFFFFF means never written. The HGSS footer is 0x10 bytes
    // (counter at end-0x10, checked on Max's save); DP/Pt have 0x14 with an extra counter in front (PKHeX).
    let counter = |base: usize| {
        let e = base + size;
        let c = if game == "heartgold-soulsilver" { (0, u32le(d, e - 0x10)) } else { (u32le(d, e - 0x14), u32le(d, e - 0x10)) };
        if c.0 == u32::MAX || c.1 == u32::MAX { (0, 0) } else { c }
    };
    let g = if counter(0x40000) > counter(0) { 0x40000 } else { 0 };
    let gd = &d[g..g + size];
    let count = (gd[party] as usize).min(6);
    let team = (0..count)
        .filter_map(|i| {
            let o = party + 4 + 236 * i;
            let m = decrypt_pk(&gd[o..o + 236], 32);
            let mut mon = pk_mon(&m, false)?;
            mon.nickname = Some(gen4_text(u16s(&m, 0x48, 11)));
            mon.details = Some(pk4_details(&m));
            Some(mon)
        })
        .collect();
    let mut badges: Vec<bool> = bits(gd[trainer + 0x1A]).collect();
    if game == "heartgold-soulsilver" {
        badges.extend(bits(gd[trainer + 0x1F]));
    }
    // TM/HM pocket: 100 slots of (u16 item, u16 count); items 328..419 = TM01..TM92, 420..427 = HM01..HM08.
    let machines = (0..100)
        .map(|i| u16le(gd, tm_pocket + 4 * i))
        .filter(|item| (328..=427).contains(item))
        .map(|item| if item >= 420 { item - 419 + 100 } else { item - 327 })
        .collect();
    // HGSS trainer flags: event flags (at 0x10C4, LSB first) number 0x550 + trainer id (PKHeX SAV4HGSS, pret flags.h;
    // checked on Max's save: the route trainers and Sprout Tower Sages he beat, not Falkner).
    let beaten_trainers = (game == "heartgold-soulsilver").then(|| {
        (1..738u16).filter(|&id| {
            let f = 0x550 + id as usize;
            gd[0x10C4 + f / 8] >> (f % 8) & 1 == 1
        }).collect()
    });
    Some(SaveInfo { game: Some(game.into()), trainer: Some(gen4_text(u16s(gd, trainer, 8))), badges, team, machines: Some(machines), beaten_trainers })
}

fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { crc << 1 ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

fn gen5(d: &[u8]) -> Option<SaveInfo> {
    let (game, misc) = [(0x24000, 0x8C, "black-white", 0x21200), (0x26000, 0x94, "black-2-white-2", 0x21100)]
        .into_iter()
        .find(|&(main, info, ..)| {
            let f = main - 0x100;
            crc16_ccitt(&d[f..f + info]) == u16le(d, f + info + 0x10 - 2)
        })
        .map(|(_, _, g, m)| (g, m))?;
    let count = (d[0x18E04] as usize).min(6);
    let team = (0..count)
        .filter_map(|i| {
            let o = 0x18E08 + 220 * i;
            let m = decrypt_pk(&d[o..o + 220], 32);
            let mut mon = pk_mon(&m, false)?;
            mon.nickname = Some(utf16(&m, 0x48, 11));
            Some(mon)
        })
        .collect();
    Some(SaveInfo { game: Some(game.into()), trainer: Some(utf16(d, 0x19404, 8)), badges: bits(d[misc + 4]).collect(), team, machines: None, beaten_trainers: None })
}

fn gen67(d: &[u8]) -> Option<SaveInfo> {
    if u32le(d, d.len() - 0x1F0) != 0x4245_4546 { return None; } // "BEEF" block table
    // (party, trainer name, badge byte, game)
    let (party, name, badge, game) = match d.len() {
        0x65600 => (0x14200, 0x14048, Some(0x420C), "x-y"),
        0x76000 => (0x14200, 0x14048, Some(0x420C), "omega-ruby-alpha-sapphire"),
        0x6BE00 => (0x1400, 0x1238, None, "sun-moon"),
        _ => (0x1600, 0x1438, None, "ultra-sun-ultra-moon"),
    };
    let count = (d[party + 6 * 0x104] as usize).min(6);
    let team = (0..count)
        .filter_map(|i| {
            let o = party + 0x104 * i;
            let m = decrypt_pk(&d[o..o + 0x104], 56);
            let mut mon = pk_mon(&m, true)?;
            mon.nickname = Some(utf16(&m, 0x40, 13));
            Some(mon)
        })
        .collect();
    // Gen 7 has no badges; its grand trials stay "ahead" in the report.
    let badges = badge.map(|b| bits(d[b]).collect()).unwrap_or_default();
    Some(SaveInfo { game: Some(game.into()), trainer: Some(utf16(d, name, 13)), badges, team, machines: None, beaten_trainers: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pk4_roundtrip() {
        // Encrypt a known PK4 by running the scheme backwards, then check decrypt_pk recovers it.
        let mut plain = vec![0u8; 236];
        let pid: u32 = 0x1234_5678;
        plain[0..4].copy_from_slice(&pid.to_le_bytes());
        plain[6..8].copy_from_slice(&0xBEEFu16.to_le_bytes());
        plain[8..10].copy_from_slice(&393u16.to_le_bytes()); // block A: species
        plain[0x28..0x2A].copy_from_slice(&145u16.to_le_bytes()); // block B: move 1
        plain[0x8C] = 17;
        let sv = ((pid >> 13 & 31) % 24) as usize;
        let mut enc = plain.clone();
        for (i, &src) in BLOCK_POSITION[sv].iter().enumerate() {
            enc[8 + src * 32..8 + (src + 1) * 32].copy_from_slice(&plain[8 + i * 32..8 + (i + 1) * 32]);
        }
        lcrng_crypt(&mut enc, 8..0x88, 0xBEEF);
        lcrng_crypt(&mut enc, 0x88..236, pid);
        let m = decrypt_pk(&enc, 32);
        let mon = pk_mon(&m, false).unwrap();
        assert_eq!((mon.species, mon.level, mon.moves.clone()), (393, 17, vec![145]));
    }

    #[test]
    fn rle() {
        assert_eq!(unrle(&[2, 7, 8, 0x83, 9, 0x80, 1, 2, 0, 0], 7).unwrap(), vec![7, 8, 9, 9, 9, 1, 1]);
    }

    /// Needs a real Gen 4 save; set PKMN_SAVE to its path, otherwise the test is skipped.
    #[test]
    fn pk4_details_match_stored_stats() {
        let Ok(path) = std::env::var("PKMN_SAVE") else { return };
        let dex = crate::data::Dex::load();
        let info = read(&std::fs::read(path).unwrap(), &dex).unwrap();
        assert!(!info.team.is_empty());
        for m in &info.team {
            let d = m.details.as_ref().unwrap();
            let base = &dex.species[&m.species].base_stats;
            assert_eq!(crate::analysis::real_stats(base, &d.ivs, &d.evs, d.nature, m.level), d.stats, "{:?} {:?}", m.nickname, d);
            assert!(d.exp >= crate::data::exp_at(dex.species[&m.species].growth, m.level));
            assert!(d.exp < crate::data::exp_at(dex.species[&m.species].growth, m.level + 1));
        }
    }
}
