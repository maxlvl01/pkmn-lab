//! Team analysis: type matchups, coverage, rating, projection, recommendations, boss matchups.
//! Returns plain data (ids, numbers); the UI turns it into German or English text.
use crate::data::{Boss, Dex, EvoHow, Learnsets, Move, VersionGroup, WildSpot};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Mon {
    pub species: u16,
    pub level: u8,
    pub moves: Vec<u16>,
    pub nickname: Option<String>,
}

pub struct Game<'a> {
    pub vg: &'a VersionGroup,
    pub bosses: &'a [Boss],
    /// Badge flags in boss "gym" order.
    pub badges: &'a [bool],
    pub learn: &'a Learnsets,
    /// TM/HM moves the player owns (from the save); None = unknown.
    pub owned_machines: Option<&'a HashSet<u16>>,
    /// Catch tips only from the game's own (regional) Pokédex.
    pub regional: bool,
    /// all other trainers (from a ROM); empty = unknown
    pub trainers: &'a [Boss],
    /// trainer ids the save marks as beaten
    pub beaten: Option<&'a HashSet<u16>>,
}

// ---------------- report data ----------------

pub struct Report {
    pub members: Vec<MemberView>,
    pub defense: Vec<DefRow>,
    /// Biggest weakness after weighing counters; and the biggest one nobody answers (if different).
    pub worst: Option<u8>,
    pub worst_uncountered: Option<u8>,
    pub also_countered: Vec<u8>,
    pub offense: Vec<OffRow>,
    /// rating: blend of today's team and its potential (evolved, at the last boss's level, best moves)
    pub summary: Summary,
    pub rating_now: u32,
    pub potential: u32,
    /// level the potential is measured at
    pub potential_level: u8,
    pub projection: Projection,
    pub catch: Vec<CatchRec>,
    pub bosses: Vec<BossView>,
    /// bosses already beaten (badge or save flag), story order
    pub beaten_bosses: Vec<BossView>,
    /// every other trainer, sorted by their top level
    pub trainers: Vec<BossView>,
}

pub struct MemberView {
    pub types: Vec<u8>,
    pub bst: u16,
    /// (move id, type, power) as in this game
    pub moves: Vec<(u16, u8, Option<u16>)>,
    pub no_stab: bool,
    pub no_damage: bool,
}

pub struct DefRow {
    pub atk: u8,
    /// multiplier per team member
    pub mults: Vec<f32>,
    pub weak: usize,
    pub resist: usize,
    /// (member index, move id) that hit this type super effectively
    pub counters: Vec<(usize, u16)>,
    pub problem: bool,
}

pub struct OffRow {
    pub def: u8,
    pub best: f32,
    pub hitters: usize,
}

#[derive(Clone, Copy)]
pub struct Summary {
    pub rating: u32,
    pub offense_pct: u32,
    pub defense_pct: u32,
}

pub enum Source {
    Known,
    Level(u8),
    Machine(u16),
}

/// A member as it will be at the projected level.
pub struct Growth {
    pub species: u16,
    pub level: u8,
    /// (new species, at level) in order
    pub evolved: Vec<(u16, u8)>,
    /// further evolutions that need something other than a level
    pub could: Vec<(u16, EvoHow)>,
    pub stats: [u16; 6],
}

pub struct ProjMember {
    pub growth: Growth,
    pub learns: Vec<(u8, u16)>,
    pub picks: Vec<(u16, Source)>,
    /// (machine number, move id) worth getting
    pub tm_ideas: Vec<(u16, u16)>,
    pub keep_status: Vec<u16>,
}

pub struct Projection {
    pub target: u8,
    pub for_boss: Option<usize>,
    pub members: Vec<ProjMember>,
    pub rating_after: u32,
}

pub struct CatchRec {
    pub gain: i32,
    pub types: Vec<u8>,
    pub examples: Vec<u16>,
    /// per example: what to catch (itself or an earlier form) and its earliest spot; None = no wild data
    pub spots: Vec<Option<(u16, WildSpot)>>,
    pub replacing: Option<usize>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Verdict {
    Dominate,
    Even,
    Struggle,
}

pub struct BossMonView {
    pub species: u16,
    pub level: u8,
    pub types: Vec<u8>,
    pub moves: Vec<u16>,
    /// our best answer: (member index, multiplier)
    pub answer: Option<(usize, f32)>,
    /// how many of ours it hits super effectively
    pub threatens: usize,
}

pub struct BossView {
    /// index into the game's boss list
    pub boss: usize,
    pub verdict: Verdict,
    pub their_max: u8,
    /// our team as projected for this fight: (species, level) per member
    pub team: Vec<(u16, u8)>,
    pub beaten: bool,
    pub mons: Vec<BossMonView>,
}

// ---------------- maths ----------------

/// A team member reduced to what the matchup maths needs.
#[derive(Clone)]
struct Fighter {
    types: Vec<u8>,
    /// (move type, power incl. STAB). Without known damaging moves: its own types at 90 power.
    attacks: Vec<(u8, f32)>,
}

fn fighter(dex: &Dex, vg: &VersionGroup, species: u16, moves: &[u16]) -> Fighter {
    let types = dex.species.get(&species).map(|s| dex.species_types(s, vg.gn)).unwrap_or_default();
    let mut attacks: Vec<(u8, f32)> = moves
        .iter()
        .filter_map(|id| dex.moves.get(id))
        .filter(|m| m.class != 1)
        .filter_map(|m| {
            let (t, p) = dex.move_stats(m, vg);
            let p = p? as f32; // fixed-damage moves (Seismic Toss etc.) have no power
            Some((t, if types.contains(&t) { p * 1.5 } else { p }))
        })
        .collect();
    if attacks.is_empty() {
        // shortcut: no move data means we assume plain STAB attacks; good enough for type-level advice.
        attacks = types.iter().map(|&t| (t, 90.0)).collect();
    }
    Fighter { types, attacks }
}

/// (best multiplier, best multiplier*power)
fn best_hit(dex: &Dex, f: &Fighter, def: &[u8], gn: u8) -> (f32, f32) {
    f.attacks
        .iter()
        .map(|&(t, p)| {
            let e = dex.eff(t, def, gn);
            (e, e * p)
        })
        .fold((0.0, 0.0), |a, b| if b.1 > a.1 { b } else { a })
}

fn summarize(dex: &Dex, team: &[Fighter], gn: u8) -> Summary {
    let types = dex.types_in_gen(gn);
    let n = types.len() as f32;
    let hit = types.iter().filter(|&&d| team.iter().any(|f| best_hit(dex, f, &[d], gn).0 >= 2.0)).count() as f32;
    let mut ok_def = 0.0;
    let mut stack_penalty = 0.0;
    for &a in &types {
        let weak = team.iter().filter(|f| dex.eff(a, &f.types, gn) > 1.0).count();
        let resist = team.iter().filter(|f| dex.eff(a, &f.types, gn) < 1.0).count();
        // Strength counts too: each member hitting the type super effectively offsets half a weakness.
        let counters = team.iter().filter(|f| best_hit(dex, f, &[a], gn).0 >= 2.0).count();
        if resist as f32 + counters as f32 * 0.5 >= weak as f32 { ok_def += 1.0; }
        if weak >= 3 { stack_penalty += (weak - 2) as f32 * 4.0; }
    }
    let offense = hit / n;
    let defense = ok_def / n;
    // Size matters too: a 2-mon team can't be "optimised" no matter how well typed.
    let size = (team.len() as f32 / 6.0).min(1.0);
    let rating = ((offense * 45.0 + defense * 45.0 + size * 10.0) - stack_penalty).clamp(0.0, 100.0);
    Summary { rating: rating.round() as u32, offense_pct: (offense * 100.0).round() as u32, defense_pct: (defense * 100.0).round() as u32 }
}

/// Bosses not beaten yet, as indices into the boss list, in story order.
fn remaining(game: &Game) -> Vec<usize> {
    let gyms_before_e4 = game.bosses.iter().take_while(|b| b.kind != "elite4").filter(|b| b.kind == "gym").count();
    // Elite Four counts as beaten once you hold a badge from after them (HGSS/GSC Kanto).
    let post_game = game.badges.iter().skip(gyms_before_e4).any(|&b| b);
    let mut gym_index = 0;
    (0..game.bosses.len())
        .filter(|&i| {
            let b = &game.bosses[i];
            let done = match b.kind.as_str() {
                "gym" => {
                    gym_index += 1;
                    game.badges.get(gym_index - 1).copied().unwrap_or(false)
                }
                "elite4" | "champion" => post_game,
                _ => false,
            } || b.id.zip(game.beaten).is_some_and(|(id, set)| set.contains(&id));
            !done && !b.team.is_empty()
        })
        .collect()
}

pub fn analyse(dex: &Dex, game: &Game, team: &[Mon], focus: Option<usize>) -> Report {
    let vg = game.vg;
    let gn = vg.gn;
    let fighters: Vec<Fighter> = team.iter().map(|m| fighter(dex, vg, m.species, &m.moves)).collect();
    let types = dex.types_in_gen(gn);

    let members = team.iter().zip(&fighters).map(|(m, f)| {
        let moves: Vec<(u16, u8, Option<u16>)> = m.moves.iter().filter_map(|&id| {
            let (t, p) = dex.move_stats(dex.moves.get(&id)?, vg);
            Some((id, t, p))
        }).collect();
        let damaging = |id: &u16| dex.moves.get(id).is_some_and(|mv| mv.class != 1);
        MemberView {
            types: f.types.clone(),
            bst: dex.species.get(&m.species).map(|s| s.base_stats.iter().sum()).unwrap_or(0),
            no_stab: !moves.is_empty() && !moves.iter().any(|(id, t, _)| damaging(id) && f.types.contains(t)),
            no_damage: !moves.is_empty() && !moves.iter().any(|(id, ..)| damaging(id)),
            moves,
        }
    }).collect();

    // Defence, with counters: real damaging moves the members know that hit the type super effectively.
    let defense: Vec<DefRow> = types.iter().map(|&a| {
        let mults: Vec<f32> = fighters.iter().map(|f| dex.eff(a, &f.types, gn)).collect();
        let weak = mults.iter().filter(|&&m| m > 1.0).count();
        let resist = mults.iter().filter(|&&m| m < 1.0).count();
        let counters = team.iter().enumerate().filter_map(|(i, m)| {
            m.moves.iter().copied().find(|id| dex.moves.get(id).is_some_and(|mv| {
                let (mt, p) = dex.move_stats(mv, vg);
                mv.class != 1 && p.is_some() && dex.eff(mt, &[a], gn) >= 2.0
            })).map(|id| (i, id))
        }).collect();
        DefRow { atk: a, mults, weak, resist, counters, problem: weak >= 3 || (weak > resist && weak >= 2) }
    }).collect();
    // Danger = defensive hole minus the members that can hit that type back.
    let danger = |r: &DefRow| {
        let quad = r.mults.iter().filter(|&&m| m >= 4.0).count() as i32;
        (r.weak as i32 - r.resist as i32) * 10 + quad * 5 + r.weak as i32 - r.counters.len() as i32 * 8
    };
    let mut ranked: Vec<&DefRow> = defense.iter().filter(|r| r.weak > 0).collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(danger(r)));
    let worst = ranked.first().map(|r| r.atk);
    let worst_uncountered = ranked.iter().find(|r| r.weak > r.resist && r.counters.is_empty()).map(|r| r.atk).filter(|&t| Some(t) != worst);
    let also_countered = ranked.iter().skip(1).filter(|r| r.weak > r.resist && !r.counters.is_empty()).map(|r| r.atk).collect();

    let offense = types.iter().map(|&d| OffRow {
        def: d,
        best: fighters.iter().map(|f| best_hit(dex, f, &[d], gn).0).fold(0.0, f32::max),
        hitters: fighters.iter().filter(|f| best_hit(dex, f, &[d], gn).0 >= 2.0).count(),
    }).collect();

    let now = summarize(dex, &fighters, gn);
    let ahead = remaining(game);
    let done: Vec<usize> = (0..game.bosses.len()).filter(|i| !ahead.contains(i) && !game.bosses[*i].team.is_empty()).collect();
    // Future potential: the team as it can be by the last boss ahead (evolved, its best moves by then).
    let later = project(dex, game, team, &ahead, ahead.last().copied());
    let summary = Summary { rating: (now.rating as f32 * 0.6 + later.rating_after as f32 * 0.4).round() as u32, ..now };
    Report {
        members,
        defense,
        worst,
        worst_uncountered,
        also_countered,
        offense,
        summary,
        rating_now: now.rating,
        potential: later.rating_after,
        potential_level: later.target,
        projection: project(dex, game, team, &ahead, focus),
        catch: recommend(dex, gn, &fighters, game, team.iter().map(|m| m.level).max().unwrap_or(1)),
        bosses: bosses(dex, game, team, &ahead, false),
        beaten_bosses: bosses(dex, game, team, &done, true),
        trainers: trainers(dex, game, team),
    }
}

/// Try every typing that exists in this gen as a new member (or as a swap if the team is full).
/// Regional mode: only Pokémon you can catch in the wild (from the ROM) or evolve them into, else the regional Pokédex.
/// Examples you can catch soon (spot level up to your top level + 10) come first, then the strongest.
fn recommend(dex: &Dex, gn: u8, fighters: &[Fighter], game: &Game, our_max: u8) -> Vec<CatchRec> {
    let learn = game.learn;
    let parent: HashMap<u16, u16> = learn.evolutions.iter().flat_map(|(&from, tos)| tos.iter().map(move |&(to, _)| (to, from))).collect();
    // Earliest wild spot of a species or any earlier form of it.
    let spot = |sp: u16| -> Option<(u16, WildSpot)> {
        let mut best: Option<(u16, WildSpot)> = None;
        let mut cur = Some(sp);
        for _ in 0..4 {
            let Some(s) = cur else { break };
            if let Some(w) = learn.wild.get(&s).and_then(|v| v.first())
                && best.as_ref().is_none_or(|b| (w.late, w.level) < (b.1.late, b.1.level)) { best = Some((s, w.clone())); }
            cur = parent.get(&s).copied();
        }
        best
    };
    let pool: Option<HashSet<u16>> = if !game.regional { None }
        else if !learn.wild.is_empty() { Some(dex.species.keys().copied().filter(|&s| spot(s).is_some()).collect()) }
        else if !learn.regional.is_empty() { Some(learn.regional.clone()) }
        else { None };
    let pool = pool.as_ref();
    let base = summarize(dex, fighters, gn).rating as i32;
    let mut typings: BTreeMap<Vec<u8>, Vec<u16>> = BTreeMap::new();
    for s in dex.species.values().filter(|s| s.gn <= gn && pool.is_none_or(|p| p.contains(&s.id))) {
        let mut ts = dex.species_types(s, gn);
        ts.sort();
        typings.entry(ts).or_default().push(s.id);
    }
    let mut scored: Vec<(i32, Vec<u8>, Option<usize>)> = vec![];
    for ts in typings.keys() {
        let cand = Fighter { types: ts.clone(), attacks: ts.iter().map(|&x| (x, 90.0)).collect() };
        let slots: Vec<Option<usize>> = if fighters.len() < 6 { vec![None] } else { (0..fighters.len()).map(Some).collect() };
        for slot in slots {
            let mut v = fighters.to_vec();
            match slot { Some(i) => v[i] = cand.clone(), None => v.push(cand.clone()) }
            scored.push((summarize(dex, &v, gn).rating as i32 - base, ts.clone(), slot));
        }
    }
    scored.sort_by_key(|s| std::cmp::Reverse(s.0));
    let mut out: Vec<CatchRec> = vec![];
    for (gain, ts, replacing) in scored {
        if gain <= 0 || out.len() == 5 || out.iter().any(|c| c.types == ts) { continue; }
        let mut examples = typings[&ts].clone();
        let soon = |id: &u16| spot(*id).is_none_or(|s| !s.1.late && s.1.level <= our_max.saturating_add(10));
        // Catchable soon first, then strongest; legendaries included, the user can skip them.
        examples.sort_by_key(|id| (!soon(id), std::cmp::Reverse(dex.species[id].base_stats.iter().sum::<u16>())));
        examples.truncate(5);
        let spots = examples.iter().map(|&id| spot(id)).collect();
        out.push(CatchRec { gain, types: ts, examples, spots, replacing });
    }
    out
}

/// Stats at a level: average IVs (15), no EVs, neutral nature.
/// shortcut: Gen 1/2 used DVs and stat experience; the same estimate is close enough for comparing teams.
pub fn stats_at(base: &[u16; 6], level: u8) -> [u16; 6] {
    let l = level as u32;
    let mut out = [0u16; 6];
    for (i, &b) in base.iter().enumerate() {
        let core = (2 * b as u32 + 15) * l / 100;
        out[i] = (if i == 0 { core + l + 10 } else { core + 5 }) as u16;
    }
    out
}

fn stat_total(dex: &Dex, species: u16, level: u8) -> f32 {
    dex.species.get(&species).map_or(0.0, |s| stats_at(&s.base_stats, level).iter().map(|&x| x as f32).sum())
}

/// What the member turns into by `level`: level evolutions happen, the rest (stones, trade, friendship) are options.
fn grow(game: &Game, species: u16, from: u8, level: u8) -> (Growth, Vec<(u16, u8, u8)>) {
    let mut stages = vec![];
    let mut evolved = vec![];
    let (mut cur, mut start) = (species, from);
    loop {
        let evos = game.learn.evolutions.get(&cur).map(Vec::as_slice).unwrap_or(&[]);
        let next = evos.iter().find_map(|&(to, how)| match how {
            EvoHow::Level(l) if l <= level => Some((to, l.max(start))),
            _ => None,
        });
        match next {
            // A stage covers the levels it is held at: (species, first level, last level).
            Some((to, at)) if evolved.len() < 3 => {
                stages.push((cur, start, at));
                evolved.push((to, at));
                cur = to;
                start = at;
            }
            _ => {
                stages.push((cur, start, level));
                let could = evos.iter().copied().filter(|e| !matches!(e.1, EvoHow::Level(_))).collect();
                return (Growth { species: cur, level, evolved, could, stats: [0; 6] }, stages);
            }
        }
    }
}

/// What moves are scored against: every type once, the Pokémon of the given bosses three times.
fn targets(dex: &Dex, game: &Game, bosses: &[usize]) -> Vec<(Vec<u8>, f32)> {
    let gn = game.vg.gn;
    let mut t: Vec<(Vec<u8>, f32)> = dex.types_in_gen(gn).into_iter().map(|x| (vec![x], 1.0)).collect();
    for &bi in bosses {
        for bm in &game.bosses[bi].team {
            if let Some(s) = dex.species.get(&bm.dex) { t.push((dex.species_types(s, gn), 3.0)); }
        }
    }
    t
}

/// One member raised to `level`: evolutions, the moves it can have by then, and the best 4 against `targets`.
fn plan(dex: &Dex, game: &Game, m: &Mon, level: u8, targets: &[(Vec<u8>, f32)]) -> (ProjMember, Fighter) {
    let vg = game.vg;
    let gn = vg.gn;
    let (mut growth, stages) = grow(game, m.species, m.level, level.max(m.level));
    let sp = dex.species.get(&growth.species).or(dex.species.get(&m.species)).expect("species");
    growth.stats = stats_at(&sp.base_stats, growth.level);
    let types = dex.species_types(sp, gn);
    let set_value = |set: &[(u8, f32)]| -> f32 {
        targets.iter().map(|(ty, w)| w * set.iter().map(|&(mt, v)| dex.eff(mt, ty, gn) * v).fold(0.0, f32::max)).sum()
    };

    let mut cands: Vec<(u16, Source)> = m.moves.iter().map(|&id| (id, Source::Known)).collect();
    let mut learns = vec![];
    for (k, &(stage_sp, first, last)) in stages.iter().enumerate() {
        for &(lv, id) in game.learn.level_up.get(&stage_sp).into_iter().flatten() {
            // The first form could also relearn older moves; an evolved form only learns from its evolution level on.
            let reachable = lv <= last && (k == 0 || lv > first);
            if !reachable { continue; }
            if lv > m.level && !learns.contains(&(lv, id)) { learns.push((lv, id)); }
            if lv >= 1 && !cands.iter().any(|c| c.0 == id) { cands.push((id, Source::Level(lv))); }
        }
    }
    learns.sort();
    let mut machine_options: Vec<(u16, u16)> = vec![];
    for &id in game.learn.machine.get(&growth.species).into_iter().flatten() {
        let Some(&n) = game.learn.machine_no.get(&id) else { continue };
        if cands.iter().any(|c| c.0 == id) { continue; }
        match game.owned_machines {
            Some(o) if o.contains(&id) => cands.push((id, Source::Machine(n))),
            _ => machine_options.push((n, id)),
        }
    }
    // Greedy: add the move that raises the set's value most; stop under 2 % (coverage moves matter even when weak).
    let valued: Vec<(usize, (u8, f32))> = cands.iter().enumerate()
        .filter_map(|(i, (id, _))| Some((i, move_value(dex, vg, &types, &sp.base_stats, dex.moves.get(id)?)?)))
        .collect();
    let value_of = |i: usize| valued.iter().find(|v| v.0 == i).unwrap().1;
    let mut chosen: Vec<usize> = vec![];
    while chosen.len() < 4 {
        let cur: Vec<(u8, f32)> = chosen.iter().map(|&i| value_of(i)).collect();
        let base = set_value(&cur);
        let best = valued.iter().filter(|v| !chosen.contains(&v.0))
            .map(|v| { let mut s = cur.clone(); s.push(v.1); (v.0, set_value(&s) - base) })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((i, gain)) if gain > 0.02 * base.max(1.0) => chosen.push(i),
            _ => break,
        }
    }
    // TM ideas: machines that would add most to the chosen set.
    let cur: Vec<(u8, f32)> = chosen.iter().map(|&i| value_of(i)).collect();
    let base = set_value(&cur);
    let mut ideas: Vec<(f32, (u16, u16))> = machine_options.iter().filter_map(|&(n, id)| {
        let v = move_value(dex, vg, &types, &sp.base_stats, dex.moves.get(&id)?)?;
        let mut s2 = cur.clone();
        s2.push(v);
        let gain = set_value(&s2) - base;
        (gain > 0.1 * base.max(1.0)).then_some((gain, (n, id)))
    }).collect();
    ideas.sort_by(|a, b| b.0.total_cmp(&a.0));
    let keep_status = if chosen.len() < 4 {
        m.moves.iter().copied().filter(|id| dex.moves.get(id).is_some_and(|mv| mv.class == 1)).collect()
    } else {
        vec![]
    };
    let moves: Vec<u16> = chosen.iter().map(|&i| cands[i].0).collect();
    let fighter = fighter(dex, vg, growth.species, &moves);
    let mut picks = vec![];
    for (i, c) in cands.into_iter().enumerate() {
        if chosen.contains(&i) { picks.push(c); }
    }
    let pm = ProjMember { growth, learns, picks, tm_ideas: ideas.into_iter().take(3).map(|x| x.1).collect(), keep_status };
    (pm, fighter)
}

fn bosses(dex: &Dex, game: &Game, team: &[Mon], list: &[usize], beaten: bool) -> Vec<BossView> {
    list.iter().map(|&bi| {
        let b = &game.bosses[bi];
        let their_max = b.team.iter().map(|m| m.level).max().unwrap_or(1);
        // Judge the fight with the team you will realistically bring: raised to their top level, evolved, with its best moves.
        let t = targets(dex, game, &[bi]);
        let plans: Vec<(ProjMember, Fighter)> = team.iter().map(|m| plan(dex, game, m, their_max, &t)).collect();
        judge(dex, game, b, bi, &plans, beaten)
    }).collect()
}

/// The rival's team depends on your starter: keep only the version that matches (by the starter line in your team).
fn rival_matches(team: &[Mon], b: &Boss) -> bool {
    let lines: [[u16; 3]; 3] = [[152, 153, 154], [155, 156, 157], [158, 159, 160]];
    let line = |sp: u16| lines.iter().position(|l| l.contains(&sp));
    let Some(ours) = team.iter().find_map(|m| line(m.species)) else { return true };
    let Some(theirs) = b.team.iter().find_map(|m| line(m.dex)) else { return true };
    theirs == (ours + 1) % 3 // the line that beats yours: grass -> fire -> water -> grass
}

/// Every other trainer, judged against your team raised to their level (plans shared per level).
fn trainers(dex: &Dex, game: &Game, team: &[Mon]) -> Vec<BossView> {
    let t = targets(dex, game, &[]);
    let mut cache: HashMap<u8, Vec<(ProjMember, Fighter)>> = HashMap::new();
    let mut out: Vec<BossView> = game.trainers.iter().enumerate()
        .filter(|(_, b)| b.kind != "rival" || rival_matches(team, b))
        .map(|(i, b)| {
            let lv = b.team.iter().map(|m| m.level).max().unwrap_or(1);
            let plans = cache.entry(lv).or_insert_with(|| team.iter().map(|m| plan(dex, game, m, lv, &t)).collect());
            let beaten = b.id.zip(game.beaten).is_some_and(|(id, set)| set.contains(&id));
            judge(dex, game, b, i, plans, beaten)
        })
        .collect();
    out.sort_by_key(|v| (v.their_max, v.boss));
    out
}

fn judge(dex: &Dex, game: &Game, b: &Boss, bi: usize, plans: &[(ProjMember, Fighter)], beaten: bool) -> BossView {
        let gn = game.vg.gn;
        let their_max = b.team.iter().map(|m| m.level).max().unwrap_or(1);
        let fighters: Vec<&Fighter> = plans.iter().map(|p| &p.1).collect();
        let mut our_score = 0.0;
        let mut their_threat = 0.0;
        let mons = b.team.iter().map(|bm| {
            let moves: Vec<u16> = bm.moves.iter().filter_map(|m| dex.move_id(m)).collect();
            let foe = fighter(dex, game.vg, bm.dex, &moves);
            let answer = fighters.iter().enumerate()
                .map(|(i, f)| (i, best_hit(dex, f, &foe.types, gn)))
                .max_by(|a, b| a.1.1.total_cmp(&b.1.1))
                .map(|(i, (mult, _))| (i, mult));
            let threatens = fighters.iter().filter(|f| best_hit(dex, &foe, &f.types, gn).0 >= 2.0).count();
            our_score += answer.map_or(0.0, |a| a.1.min(2.0));
            their_threat += threatens as f32 / fighters.len().max(1) as f32;
            BossMonView { species: bm.dex, level: bm.level, types: foe.types, moves, answer, threatens }
        }).collect();
        let n = b.team.len().max(1) as f32;
        // Raw strength: average stat total of their Pokémon vs. ours at that level (evolved forms count here).
        let theirs = b.team.iter().map(|bm| stat_total(dex, bm.dex, bm.level)).sum::<f32>() / n;
        let ours = plans.iter().map(|p| p.0.growth.stats.iter().map(|&x| x as f32).sum::<f32>()).sum::<f32>() / plans.len().max(1) as f32;
        let strength = if theirs > 0.0 { (ours / theirs - 1.0).clamp(-0.5, 0.5) } else { 0.0 };
        let score = our_score / n - their_threat / n * 1.5 + strength;
        let verdict = if score >= 1.2 { Verdict::Dominate } else if score >= 0.6 { Verdict::Even } else { Verdict::Struggle };
        let team = plans.iter().map(|p| (p.0.growth.species, p.0.growth.level)).collect();
        BossView { boss: bi, verdict, their_max, team, beaten, mons }
}

/// How much a damaging move is worth to this Pokémon: (type, power x STAB x accuracy x attacking stat / 100).
fn move_value(dex: &Dex, vg: &VersionGroup, types: &[u8], base: &[u16; 6], mv: &Move) -> Option<(u8, f32)> {
    if mv.class == 1 { return None; }
    let (t, p) = dex.move_stats(mv, vg);
    let p = p? as f32;
    // Before Gen 4 the type decides physical/special (Normal..Steel physical).
    let physical = if vg.gn >= 4 { mv.class == 2 } else { t <= 9 };
    let stat = if physical { base[1] } else { base[3] } as f32;
    let acc = mv.accuracy.unwrap_or(100) as f32 / 100.0;
    let stab = if types.contains(&t) { 1.5 } else { 1.0 };
    let drawback = match mv.id {
        120 | 153 => 0.0,                                  // Finale, Explosion: the user faints
        63 | 416 | 307 | 308 | 338 | 439 | 459 => 0.5,    // must recharge
        13 | 19 | 76 | 91 | 130 | 143 | 291 | 340 => 0.6, // two-turn moves
        138 | 264 => 0.3,                                  // Traumfresser needs sleep, Power-Punch fails if hit
        _ => 1.0,
    };
    Some((t, p * stab * acc * stat / 100.0 * drawback))
}

/// The team raised to the level of the chosen boss (default: the next one), with evolutions and the best 4 moves.
fn project(dex: &Dex, game: &Game, team: &[Mon], ahead: &[usize], focus: Option<usize>) -> Projection {
    let our_max = team.iter().map(|m| m.level).max().unwrap_or(1);
    let pos = focus.and_then(|f| ahead.iter().position(|&b| b == f)).unwrap_or(0);
    let boss = ahead.get(pos).copied();
    let target = boss.map(|b| game.bosses[b].team.iter().map(|m| m.level).max().unwrap_or(1))
        .unwrap_or(our_max.saturating_add(5)).max(our_max);
    let t = targets(dex, game, ahead.get(pos..).map(|r| &r[..r.len().min(3)]).unwrap_or(&[]));
    let (members, fighters): (Vec<ProjMember>, Vec<Fighter>) = team.iter().map(|m| plan(dex, game, m, target, &t)).unzip();
    Projection { target, for_boss: boss, members, rating_after: summarize(dex, &fighters, game.vg.gn).rating }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_specific_type_chart() {
        let dex = Dex::load();
        let (ghost, psychic, fire, ice) = (8, 14, 10, 15);
        assert_eq!(dex.factor(ghost, psychic, 1), 0);
        assert_eq!(dex.factor(ghost, psychic, 4), 200);
        assert_eq!(dex.factor(ice, fire, 1), 100);
        assert_eq!(dex.factor(ice, fire, 2), 50);
        let clefairy = &dex.species[&35];
        assert_eq!(dex.species_types(clefairy, 4), vec![1]);
        assert_eq!(dex.species_types(clefairy, 6), vec![18]);
        assert_eq!(dex.find_species("Plinfa").unwrap().id, 393);
        let hgss = dex.vg("heartgold-soulsilver").unwrap();
        let ls = dex.learnsets(hgss);
        assert!(ls.level_up[&74].contains(&(11, 88))); // Kleinstein: Steinwurf at 11
        assert_eq!(ls.machine_no[&317], 39); // TM39 Felsgrab
        assert_eq!(ls.machine_no[&15], 101); // HM01 Zerschneider
        // Bite was Normal in Gen 1 and Dark from Gen 2.
        let bite = dex.find_move("bite").unwrap();
        assert_eq!(dex.move_stats(bite, dex.vg("red-blue").unwrap()).0, 1);
        assert_eq!(dex.move_stats(bite, dex.vg("gold-silver").unwrap()).0, 17);
    }

    #[test]
    fn rival_follows_your_starter() {
        let boss = |sp: u16| Boss { kind: "rival".into(), name_en: String::new(), name_de: String::new(), type_: None, location_en: None,
            location_de: None, variant: None, team: vec![crate::data::BossMon { dex: sp, level: 5, moves: vec![] }], id: None };
        let karnimani = [Mon { species: 158, level: 10, moves: vec![], nickname: None }];
        assert!(rival_matches(&karnimani, &boss(152)));  // you: water -> Silber: Endivie
        assert!(!rival_matches(&karnimani, &boss(155)));
        assert!(!rival_matches(&karnimani, &boss(158)));
    }

    #[test]
    fn counters_and_projection() {
        let dex = Dex::load();
        let vg = dex.vg("heartgold-soulsilver").unwrap();
        let learn = dex.learnsets(vg);
        let bosses = dex.bosses.iter().find(|g| g.id == vg.ident).unwrap().bosses.clone();
        let game = Game { vg, bosses: &bosses, badges: &[], learn: &learn, owned_machines: None, regional: false, trainers: &[], beaten: None };
        let aqua = dex.find_move("Aquaknarre").unwrap().id;
        let tackle = dex.find_move("Tackle").unwrap().id;
        let team = vec![
            Mon { species: 74, level: 8, moves: vec![tackle], nickname: None },    // Kleinstein
            Mon { species: 158, level: 10, moves: vec![aqua], nickname: None },    // Karnimani
        ];
        let r = analyse(&dex, &game, &team, None);
        let ground = r.defense.iter().find(|d| d.atk == 5).unwrap();
        assert_eq!(ground.counters, vec![(1, aqua)]); // Karnimani answers Boden
        assert_eq!(r.projection.for_boss, Some(0)); // next boss: Falk
        // Kleinstein learns Steinwurf (88) at Lv 11 before Falk's Lv 13 and should pick it.
        let k = &r.projection.members[0];
        assert!(k.learns.contains(&(11, 88)));
        assert!(k.picks.iter().any(|p| p.0 == 88));
        assert_eq!(k.growth.species, 74); // no evolution before Lv 13

        // Projected to the last boss: Kleinstein -> Georok at 25 (Geowaz by trade in vanilla), Karnimani -> Impergator.
        let last = *r.bosses.last().map(|b| &b.boss).unwrap();
        let r2 = analyse(&dex, &game, &team, Some(last));
        let g = &r2.projection.members[0].growth;
        assert_eq!((g.species, g.evolved[0]), (75, (75, 25)));
        assert!(g.could.contains(&(76, crate::data::EvoHow::Trade)));
        assert_eq!(r2.projection.members[1].growth.species, 160);
        assert_eq!(r2.bosses.last().unwrap().team[1].0, 160); // the boss verdict uses the evolved team
    }
}
