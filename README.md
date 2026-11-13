# PKMN Lab

Team analyser for Pokémon runs, in German and English. A desktop app written in Rust (egui).

**Download:** the newest `pkmn-lab-windows-x86_64.exe` from [Releases](https://github.com/maxlvl01/pkmn-lab/releases).
It is a single file: no installer, and it updates itself (checks GitHub once a day, or via "Nach Updates suchen").

## How to use

1. Start `pkmn-lab-windows-x86_64.exe`.
2. Drag your emulator save (`.sav`, `.dsv`, …) into the window, or click **Spielstand öffnen**. Game, badges, team,
   moves and the TMs in your bag are read from it. No save? **Team von Hand eingeben**, then pick the game.
3. Optional for HeartGold/SoulSilver: drag the `.nds` ROM in as well. Then the real boss teams and level-up moves are read
   from the ROM, so ROM hacks (e.g. hard mods) show what you will actually face.

The app remembers the last save and ROM and re-reads the save at every start, so it is always current.
You can also start it with files: `pkmn-lab.exe <save> [rom.nds]` (or "Öffnen mit").

## What it shows

- **Team:** each member with sprite, types, base stat total and moves; flags missing STAB or damage moves.
- **Schwächen:** how every attack type hits every member, the biggest weakness, and whether someone hits it back
  super effectively (a counter makes a weakness count less).
- **Angriff:** your best multiplier against each type and how many members hit it super effectively.
- **Projektion:** pick any boss ahead (default: the next one). Each member is raised to that boss's top level, evolves
  where a level evolution allows it (stones, trade and friendship are shown as options), and gets its stats at that level,
  what it learns on the way and the best 4 attacks for that fight (power, STAB, accuracy, matching attack stat, coverage).
  TMs/HMs in your bag are used; others appear as "TM-Ideen". With an HGSS ROM, the hack's own evolutions are used.
- **Fangen:** which typing would raise the team rating most, with example Pokémon.
- **Bosse:** every gym leader, Elite Four member and champion still ahead, with their moves and your best answer to each
  of their Pokémon. Each verdict is judged with your team as it will be at that boss's level (evolved, with its projected
  moves and stats), not with today's team.

Team rating 0–100: offensive coverage 45 %, types handled defensively 45 %, team size 10 %, minus a penalty for
stacked weaknesses.

## Support

| | Read from save | Boss data |
|---|---|---|
| Gen 1 Rot/Blau/Gelb | yes (international saves) | yes |
| Gen 2 Gold/Silber/Kristall | yes (international saves) | yes |
| Gen 3 Rubin/Saphir/Smaragd/Feuerrot/Blattgrün | yes | yes |
| Gen 4 Diamant/Perl/Platin/HeartGold/SoulSilver | yes (tested with a real SoulSilver save) | yes; **HGSS teams and learnsets from the ROM** |
| Gen 5 Schwarz/Weiß (2) | yes | yes |
| Gen 6/7 X/Y, ΩR/αS, Sonne/Mond, US/UM | yes (Citra `main` file) | yes |
| Let's Go, Gen 8, Gen 9 | not yet: enter the team by hand | yes |

Save containers: raw `.sav`/`.srm`/`.sa1`, DeSmuME `.dsv` (with or without footer), No$GBA (incl. compressed), saves with
RTC footers. Only the HGSS reader has been tested against a real save; the other generations follow PKHeX's offsets
(`research/save-formats.md`) and still need a real save each to confirm.

## Sprites

Pokémon sprites (in the style of the chosen game) come from [PokeAPI/sprites](https://github.com/PokeAPI/sprites),
trainer portraits from Pokémon Showdown. They are Nintendo / Game Freak / The Pokémon Company artwork, so they are **not**
part of this repository or the program: each one is downloaded the first time it is shown and kept in
`%LOCALAPPDATA%\pkmn-lab\sprites`. Without internet the app works and shows a Pokéball instead.

## Data

- `data/pokeapi/`: PokeAPI CSVs (names DE/EN, types per generation, moves, move changes, learnsets, TMs).
- `data/bosses/`: gym leaders, Elite Four and champions per game; German names from PokéWiki, teams from pret / PokéWiki.
- Everything except sprites is compiled into the program.

## Build

```
cargo build --release
```

Fan project, not affiliated with Nintendo, Game Freak or The Pokémon Company. No ROMs or saves are included.
