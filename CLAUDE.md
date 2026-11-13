# PKMN Lab

Rust (eframe/egui 0.37) team analyser for Pokémon runs: reads emulator saves and HGSS ROMs, German and English UI.
Public repo maxlvl01/pkmn-lab with a self-updater that downloads the release asset.

## Build and test
- `taskkill //IM pkmn-lab.exe //F` before `cargo build --release`, else "failed to remove file pkmn-lab.exe".
- `cargo test --release` runs the unit tests; ROM/save tests skip unless `PKMN_ROM` (German SoulSilver ROM) and `PKMN_SAVE` (HGSS .dsv) are set.
- PowerShell scripts need `powershell -ExecutionPolicy Bypass -File ...`.
- Running the app writes the user's settings (`app.ron`: save/ROM path); after test runs, put the user's own paths back.
- Never commit ROMs or saves.

## egui 0.37 gotchas
- `egui::Modal::new(Id::unique(..))` (`Id::new` is deprecated); `.should_close()` to dismiss.
- Default font lacks arrows like → and squares like ■: use » or paint shapes; never emoji as icons.
- Charts (radar, line) are painted by hand in `gui.rs` (`radar()`, `line_chart()`); no plot crate.

## HGSS ROM layout (`src/rom.rs`)
- Evolutions `a/0/3/4`: 6-byte rows (method, param, target); methods 1/2/3 = friendship any/day/night (needs >= 220, evolves on level-up).
- Wild encounters: SS `a/1/3/6`, HG `a/0/3/7`; 196-byte entries (grass levels 0x08, morning/day/night species 0x14/0x2C/0x44, surf 0x64, rock smash 0x78, rods 0x80/0x94/0xA8).
- Map headers: 24-byte rows in ARM9, found by scan; byte 0 = encounter id, 0x12 = place id.
- Text `a/0/2/7`: 279 place names (= Gen 4 met locations), 222 items, 720 abilities, 34 natures.
- Story order (badges per place) in `HGSS_STORY` / `research/story-order.csv` is partly from memory, not game checks.

## Save layout (`src/save.rs`)
- PK4: species 0x08, item 0x0A, EXP 0x10, friendship 0x14, ability 0x15, EVs 0x18, moves 0x28, IVs u32 0x38, party stats 0x90; nature = PID % 25.

## Names
- In-game names are German, taken from the ROM text or PokeAPI German names; say when a name is unverified.

## Release
1. Bump `version` in `Cargo.toml`, update README, commit.
2. `cargo build --release`, push, then copy `target/release/pkmn-lab.exe` to `pkmn-lab-windows-x86_64.exe` (the updater looks for that name) and `gh release create vX.Y.Z <that file> --notes ...`.
3. Only push or release on the user's explicit yes.
