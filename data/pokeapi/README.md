# PokeAPI reference data (CSV)

Source: https://raw.githubusercontent.com/PokeAPI/pokeapi/master/data/v2/csv/<name>.csv
Downloaded: 2026-11-13. 25 files (the 24 requested + move_changelog.csv). Not edited.
Files: pokemon_species, pokemon_species_names, pokemon, pokemon_types, pokemon_types_past, pokemon_stats, stats, types,
type_names, type_efficacy, type_efficacy_past, moves, move_names, move_damage_classes, move_changelog, versions,
version_names, version_groups, generations, languages, items, item_names, abilities, ability_names, pokemon_abilities.

## Languages (languages.csv)
German = local_language_id 6 (de), English = 9 (en). French 5, Spanish 7, Italian 8, Japanese 1/11, Korean 3.
German rows exist in all *_names files (counts: species 1025, moves 937, types 21, items 2219, abilities 311).
Samples: species 393 "Plinfa", 25 "Pikachu", 1 "Bisasam"; moves 1 "Klaps", 2 "Karateschlag", 3 "Duplexhieb";
types 1 "Normal", 2 "Kampf", 8 "Geist" (9 Stahl, 14 Psycho, 17 Unlicht, 18 Fee); items 1 "Meisterball", 2 "Hyperball",
3 "Superball"; abilities 1 "Duftnote", 2 "Niesel", 3 "Temposchub".
Column naming: species_names has pokemon_species_id, the other *_names have <x>_id; all use local_language_id.
Type 10001 = "???" (unknown), 10002 = shadow, 19 = stellar (gen 9).

## Generation-specific types
types.csv generation_id = generation the type was introduced (steel 9 and dark 17 = gen 2, fairy 18 = gen 6, stellar = 9).
So "Steel/Dark don't exist before Gen 2" and "Fairy before Gen 6" are NOT encoded as rows in the past files; the tool must
filter by types.generation_id.

type_efficacy.csv = CURRENT chart (damage_type_id, target_type_id, damage_factor in percent: 0/50/100/200).
type_efficacy_past.csv = only the DIFFERENCES (6 rows). generation_id semantics: "this value held up to and including
this generation"; the current value applies after it. Find the value for gen G: take the past row with the smallest
generation_id >= G for that (attacker, target) pair if one exists, else the current row. Full content:
- 4 Poison -> 7 Bug 200, gen 1 (now 100)
- 7 Bug -> 4 Poison 200, gen 1 (now 50)
- 8 Ghost -> 14 Psychic 0, gen 1 (now 200; the Gen 1 bug, Ghost has no effect on Psychic)
- 15 Ice -> 10 Fire 100, gen 1 (now 50)
- 8 Ghost -> 9 Steel 50, gen 5 (now 100, so gen 2-5 = 50)
- 17 Dark -> 9 Steel 50, gen 5 (now 100, so gen 2-5 = 50)
Gen 1 has no Steel/Dark/Fairy, so ignore those types via types.generation_id. Fairy rows are only in current type_efficacy.

pokemon_types_past.csv (pokemon_id, generation_id, type_id, slot): the types a Pokemon had up to and including that
generation (37 rows incl. header; all slots of the old typing are listed). Current typing is in pokemon_types.csv.
Examples: gen-5 rows (typing before Fairy, valid through gen 5): 35 Clefairy, 36 Clefable, 39/40 Pummeluff line, 173-175, 209/210,
298 -> Normal (now Fairy or Normal+Fairy); 176 Togetic, 468 Togekiss -> Normal+Flying; 122 Mr. Mime, 280-282 Ralts line, 439 -> Psychic;
183/184 Marill line -> Water; 546/547 -> Grass; 303 Mawile -> Steel (now Steel+Fairy). Gen-1 rows: 81 Magnemite, 82 Magneton ->
Electric only (now Electric+Steel, so Steel arrives gen 2). 10008-10012 (Rotom forms) have gen-4 rows Electric+Ghost (now Electric+Fire etc.).
Rule: for gen G, if the pokemon has past rows with generation_id >= G use the rows of the smallest such generation, else the
current pokemon_types.csv rows. Also check types.generation_id (e.g. Dark/Steel are not available in Gen 1 at all).

## Moves
moves.csv columns: id, identifier, generation_id (introduced), type_id, power, pp, accuracy, priority, target_id,
damage_class_id (1 status, 2 physical, 3 special; see move_damage_classes.csv), effect_id, effect_chance, contest_*.
Values are the CURRENT ones. damage_class_id is the modern per-move class (Gen 4+ split); for Gen 1-3 derive the class from the move's type.
move_changelog.csv exists (198 rows): move_id, changed_in_version_group_id, type_id, power, pp, accuracy, priority, target_id,
effect_id, effect_chance. Semantics: the row says "in version group X this move was changed"; the filled columns hold the OLD
values valid BEFORE that version group (blank = unchanged). Example: move 2 Karate Chop, changed_in 3 (gold-silver), type 1 Normal
= it was Normal in version groups < 3 (red-blue/yellow), now Fighting; move 85 Thunderbolt, changed_in 15 (x-y), power 95
= 95 before X/Y, now 90. Several rows per move give a chain: for version group V use the row with the smallest
changed_in_version_group_id > V (by version_groups.order, not id!), else the current value. Version groups with rows: 3, 5, 8, 9, 11, 15, 17, 19, 20, 25.

## Version groups (version_groups.csv: id, identifier, generation_id, order); versions.csv maps version -> version_group_id
1 red-blue g1 (order 3) | 2 yellow g1 (4) | 3 gold-silver g2 (5) | 4 crystal g2 (6) | 5 ruby-sapphire g3 (7) | 6 emerald g3 (8)
7 firered-leafgreen g3 (11) | 8 diamond-pearl g4 (12) | 9 platinum g4 (13) | 10 heartgold-soulsilver g4 (14)
11 black-white g5 (15) | 12 colosseum g3 (9) | 13 xd g3 (10) | 14 black-2-white-2 g5 (16) | 15 x-y g6 (17)
16 omega-ruby-alpha-sapphire g6 (18) | 17 sun-moon g7 (19) | 18 ultra-sun-ultra-moon g7 (20)
19 lets-go-pikachu-lets-go-eevee g7 (21) | 20 sword-shield g8 (22) | 21 the-isle-of-armor g8 (23) | 22 the-crown-tundra g8 (24)
23 brilliant-diamond-shining-pearl g8 (25) | 24 legends-arceus g8 (26) | 25 scarlet-violet g9 (27) | 26 the-teal-mask g9 (28)
27 the-indigo-disk g9 (29) | 28 red-green-japan g1 (1) | 29 blue-japan g1 (2) | 30 legends-za g9 (30)
31 mega-dimension g9 (31) | 32 champions g9 (32)
IDs are NOT chronological (colosseum/xd 12-13, Japan-only 28-29): sort by "order". generations.csv: 1-9 (generation-i .. generation-ix).
German version names: version_names.csv with local_language_id 6 (1 Rot, 2 Blau, 3 Gelb, 4 Gold).

## Learnsets and TMs (pokemon_moves.csv, machines.csv, pokemon_move_methods.csv)
Downloaded from PokeAPI master (data/v2/csv). pokemon_moves.csv is ~10.7 MB (header: pokemon_id, version_group_id, move_id,
pokemon_move_method_id, level, order, mastery), machines.csv ~33 KB (machine_number, version_group_id, item_id, move_id).
pokemon_move_methods.csv: 1 level-up, 2 egg, 3 tutor, 4 machine (TM/HM), 5 stadium-surfing-pikachu, 6 light-ball-egg
(Pichu Volt Tackle), 7 colosseum-purification, 8 xd-shadow, 9 xd-purification, 10 form-change (Rotom), 11 zygarde-cube, 12 train.
Filter by version_group_id (HG/SS = 10, Platin 9, D/P 8). For level-up rows `level` is the learn level and `order` is only
filled for ties at the same level (e.g. Geodude level 1: Tackle order 1, Defense Curl order 2); level 1 = known from the start.
Level 0 for method 1 does NOT occur in vg 10 (only in vg 17+, where it marks "learned on evolution"); for method 4/2/3 the
level column is always 0.
machines.csv in vg 10 has 100 rows: TM01-TM92 as machine_number 1-92, HM01-HM08 as machine_number 101-108 (HM = 100 + n).
item_id there is the PokeAPI item id, NOT the Gen 4 save item id. Examples: TM39 -> move 317 (Felsgrab), HM01 -> move 15 (Zerschneider).
Machine learnsets are method 4 rows of the Pokemon in that version group (Geodude has 35 in vg 10) and refer to the move_id,
so a bag TM is matched via machines.csv (machine_number -> move_id), then looked up in pokemon_moves.
