# Save-file and ROM format notes (for the Rust tool)

All numbers are bytes, offsets are hex, multi-byte values are **little-endian unless stated** (Gen 1/2 use big-endian for
TID/HP/stats/exp). "PKHeX" = https://github.com/kwsch/PKHeX/tree/master/PKHeX.Core (file paths below are relative to that).
Everything marked **[verified on ROM]** was checked against `Pokemon Silberne Edition - Hard Mod.nds` with a throwaway
node script (scratchpad, not modified ROM). Everything else was read from the cited source, not tested against a real save.

Companion data file: `gen4-charmap-intl.json` (same folder) = Gen 4/5-style 16-bit character table (see Part C).

---------------------------------------------------------------------------------------------------------------------

# PART A - Emulator containers (get the raw cartridge save)

General strategy for the tool: look at file **length and magic**, strip header/footer, then hand the raw bytes to Part B
detection. Raw sizes (PKHeX `Saves/Util/SaveUtil.cs`, constants `SIZE_G*`):

| Raw save | Size |
|---|---|
| Gen 1 (RBY) | 0x8000 (32 KiB) |
| Gen 2 (GSC) | 0x8000 (US/EU/KOR); PKHeX also accepts 0x10000 (`SIZE_G2RAW_J`, JP dumps) |
| Gen 3 (RSE/FRLG) | 0x20000 (128 KiB); 0x10000 also accepted (only slot 0 present) |
| Gen 4 (DP/Pt/HGSS) | 0x80000 (512 KiB) |
| Gen 5 (BW/B2W2) | 0x80000 (512 KiB) |
| Gen 6 XY / ORAS | 0x65600 / 0x76000 |
| Gen 7 SM / USUM / LGPE | 0x6BE00 / 0x6CC00 / 0x100000 |
| Gen 8 SwSh | 0x1716B3 .. 0x1876B1 depending on patch (see SaveUtil `SIZE_G8SWSH*`) |
| Gen 8 BDSP | 0xE9828, 0xEDC20, 0xEED8C, 0xEF0A4 |
| Gen 9 SV | ~0x31626F .. 0x31CF7C (+DLC variants up to ~0x4329D0+); see `SIZE_G9_*` |

## A1. DeSmuME `.dsv`  (Gen 4/5 DS)
Source: PKHeX `Saves/Util/Recognition/SaveHandlerDeSmuME.cs`; DeSmuME `src/mc.cpp` (`readFooter`, ~line 970).
- Detect: file **ends with the 16 ASCII bytes `|-DESMUME SAVE-|`**.
- Layout: `[raw save, padded][footer]`. Footer = 82-byte text `|<--Snip above here to create a raw sav by excluding this DeSmuME savedata footer:`
  + six u32 LE (`size_written`, `padSize`, `type`, `addr_size`, `mem_size`, `version`=0) + the 16-byte cookie.
  Footer length = 82 + 24 + 16 = **0x7A (122) bytes**, independent of the save size.
- Strip: `raw = file[0 .. len - 0x7A]`. For a 512 KiB DS save the .dsv is 0x8007A bytes (PKHeX `ExpectedSize`).
  The u32 `padSize` (at `len-0x28`... i.e. footer offset 82+4) equals the raw length and may be used as a cross-check.
- A plain `.sav` from DeSmuME ("export raw") has no footer.

## A2. No$GBA `.sav` (GBA) and No$GBA DS saves
Source: DeSmuME `src/mc.cpp` `readNoGbaSaveInfo` / `no_GBA_unpackSAV` (it imports these files). Layout:
- 0x00..0x1E: ASCII `NocashGbaBackupMediaSavDataFile` (31 chars), 0x1F = `0x1A`.
- 0x40: ASCII `SRAM` (this tag is used for all save types).
- 0x44 u32: compression method. 0 = stored, 1 = RLE.
- Method 0: 0x48 u32 = size; **data starts at 0x4C**, `size` bytes.
- Method 1 (compressed): 0x48 u32 = packed size, 0x4C u32 = unpacked size, **packed data starts at 0x50**.
  RLE stream: control byte `c`: `0x00` = end; `0x01..0x7F` = copy next `c` literal bytes; `0x80` = next byte is value, then
  u16 LE count (run); `0x81..0xFF` = run of `c-0x80` copies of the next byte. Output must equal `unpacked size`.
- Raw = decoded bytes (128 KiB for Gen 3 saves, 512 KiB for Gen 4/5). If the decoded size is not a known raw size, tell the user.

## A3. mGBA / VBA / VBA-M (GBA: `.sav`, `.sa1`, `.srm`)
- Gen 3 .sav from mGBA/VBA is normally the **raw 0x20000 flash image**, no header. PKHeX additionally tolerates a footer
  (`SaveHandlerFooterRTC`: `len & 0x3F` in 0x0C..0x30 and even, or exactly 7, after subtracting it the size must be
  0x20000 / 0x10000 / 0x8000 (also used for Gen 1/2)).
- Rule for the tool: if `len` in `[0x20000, 0x20000+0x30]` -> take the first 0x20000 bytes. A 0x10000 file is a half image
  (slot A only).  `.sa1/.srm` are the same raw image with another extension.
- Not verified against emulator source: whether mGBA/VBA ever append RTC data to GBA saves (PKHeX's tolerance suggests yes for some).

## A4. Gambatte / BGB / SameBoy (GB/GBC)  `.sav`
- Gen 1 raw = 0x8000. Gen 2 raw = 0x8000. Crystal/Gold/Silver have an MBC3 RTC, so emulators may **append an RTC footer**:
  BGB / VBA-style = 0x30 bytes (48), VBA-M = 0x30 (10 u32 + u64 timestamp), Gambatte = 8-byte (not in PKHeX range 0x0C..0x30, unverified),
  SameBoy = 0x30-ish (unverified).
- Rule for the tool: if `0x8000 <= len <= 0x8030` (or up to ~0x8040 to be generous), raw = first 0x8000 bytes. Longer than that is not a valid GB save.
- PKHeX source for the tolerance: `Saves/Util/Recognition/SaveHandlerFooterRTC.cs` (accepts footer 0x0C..0x30, even length, or 7).

## A5. melonDS `.sav` (DS)
- melonDS writes the **raw cartridge save** (HGSS/DPPt/BW = 0x80000, no header/footer). Treat as raw. (Not verified from melonDS source; consistent with PKHeX only accepting 0x80000 raw.)

## A6. Citra / Azahar (3DS)
- Citra's emulated save lives in `.../sdmc/Nintendo 3DS/<id0>/<id1>/title/00040000/<titleid-low>/data/00000001/main`
  (a plain file named `main`). It is the **raw** Pokémon save (XY 0x65600, ORAS 0x76000, SM 0x6BE00, USUM 0x6CC00). Checkpoint/JKSM dumps also produce a file `main`.
  Detect by exact size + the `BEEF` footer (see Gen 6/7 below). Not verified against Citra source (path from general knowledge).

## A7. Switch (Yuzu/Ryujinx/Checkpoint/JKSV)
- Save dumps contain a file called `main` (SwSh, SV, BDSP `SaveData.bin`, LGPE `savedata.bin`). It is the raw (still encrypted) SCBlock
  file for SwSh/SV; detect by size (Part B Gen 8/9). PKHeX also has an NSO header handler (`SaveHandlerNSO.cs`: magic `SRAM`) for
  Nintendo Switch Online saves, irrelevant here.

---------------------------------------------------------------------------------------------------------------------

# PART B - Game/generation detection and party reading

Reminder: **species IDs in the file are not always the national dex number** (Gen 1, Gen 3, Gen 9 differ; tables below).
Valid ranges (PKHeX `Legality/Legal.cs`: `MaxSpeciesID_n`, `MaxMoveID_n`):

| Gen / game | species (national) | move ids |
|---|---|---|
| 1 RBY | 1..151 | 1..165 |
| 2 GSC | 1..251 | 1..251 |
| 3 RSE/FRLG | 1..386 (file stores internal ids) | 1..354 |
| 4 DPPt/HGSS | 1..493 | 1..467 |
| 5 BW/B2W2 | 1..649 | 1..559 |
| 6 XY / ORAS | 1..721 | 1..617 / 1..621 |
| 7 SM / USUM | 1..802 / 1..807 | 1..719 / 1..728 |
| 7b LGPE | 1..809 | 1..742 |
| 8 SwSh | 1..898 | 1..826 |
| 8a LA | to Enamorus (905) | to Take Heart (919) |
| 8b BDSP | 1..493 | 1..826 |
| 9 SV | 1..1025 (Pecharunt; file stores internal ids >= 917) | 1..919 (Malignant Chain) |

## B1. Gen 1 (Red/Blue/Yellow)
Source: `Saves/SAV1.cs`, `Saves/Substructures/Gen12/SAV1Offsets.cs`, `PKM/PK1.cs`, `SaveUtil.cs` (`IsG1*`).
- Detect (PKHeX `IsG1*`): length exactly 0x8000 and both lists valid (count byte <= max, and the byte at `list+1+count` == 0xFF): INT = lists at **0x2F2C** (party, max 20 in PKHeX test) and **0x30C0** (current box); JP = **0x2ED5** and **0x302D** (max 30).
  JP: 0x2ED5 & 0x302D (<=30). (PKHeX uses max 20 / 30 for both lists.)
- Yellow vs Red/Blue: `Data[Starter]` (INT 0x29C3, JP 0x29B9) != 0 ? == 0x54 (Pikachu) : (Pikachu friendship byte at INT 0x271C / JP 0x2712) != 0. Red vs Blue is not distinguishable.
- Offsets (INT | JP): player name 0x2598 (both; 11 bytes INT = 10 chars + 0x50, JP 6 bytes: 5+term, field still starts 0x2598), rival 0x25F6 | 0x25F1,
  money (3 byte BCD) 0x25F3 | 0x25EE, badges (bitmask, bit0 Boulder ... bit7 Earth) **0x2602 | 0x25F8**, TID (u16 **big-endian**) 0x2605 | 0x25FB,
  play time 0x2CED | 0x2CA0, **party list 0x2F2C | 0x2ED5**, current box 0x30C0 | 0x302D. Checksum byte at 0x3523 | 0x3594 = `~sum(bytes[0x2598..checksum_offset))`.
- Party list layout at `P`: `[count u8][species u8 x (6+1), terminated by 0xFF][6 x 44-byte mon][6 x OT name][6 x nickname]`, names are 11 bytes (INT) / 6 bytes (JP),
  so INT total 1+7+264+66+66 = 404, JP 1+7+264+36+36 = 344. Slot `i` mon at `P+8+44*i`, OT at `P+8+264+11*i`, nick at `P+8+264+66+11*i`.
- 44-byte party mon (PK1.cs): 0 species (INTERNAL index), 1-2 cur HP (BE), 3 box level, 4 status, 5-6 types, 7 catch rate (Gen1 has no held item), **8..11 four move ids (1 byte each, 0 = none)**,
  0x0C-0x0D OT ID (BE), 0x0E-0x10 exp (3 byte BE), 0x11-0x1A stat exp, 0x1B-0x1C DVs, 0x1D-0x20 PP (low 6 bits PP, top 2 bits PP ups), **0x21 level**, 0x22-0x2B max HP/Atk/Def/Spd/Spc (BE).
- Species internal -> national (from PKHeX `PKM/Util/Conversion/SpeciesConverter.cs` `Table1InternalToNational`; index = internal id 0x00..0xBE, 0 = invalid/glitch slot):
  ```
  0,112,115,32,35,21,100,34,80,2,103,108,102,88,94,29,31,104,111,131,59,151,130,90,72,92,123,120,9,127,114,0,0,58,95,22,16,79,64,75,113,67,122,106,107,24,47,54,96,76,0,126,0,125,82,109,0,56,86,50,128,0,0,0,83,48,149,0,0,0,84,60,124,146,144,145,132,52,98,0,0,0,37,38,25,26,0,0,147,148,140,141,116,117,0,0,27,28,138,139,39,40,133,136,135,134,66,41,23,46,61,62,13,14,15,0,85,57,51,49,87,0,0,10,11,12,68,0,55,97,42,150,143,129,0,0,89,0,99,91,0,101,36,110,53,105,0,93,63,65,17,18,121,1,3,73,0,118,119,0,0,0,0,77,78,19,20,33,30,74,137,142,0,81,0,0,4,7,5,8,6,0,0,0,0,43,44,45,69,70,71
  ```
  (e.g. internal 0x01 = Rhydon = 112, 0x99 = Bulbasaur = 1.) Generated by script from the PKHeX table; 191 entries (0x00..0xBE).
- JP differences: smaller name fields (5 chars + term = 6 bytes), different offsets above, 30-slot boxes; Japanese charset (Part C).

## B2. Gen 2 (Gold/Silver/Crystal)
Source: `Saves/SAV2.cs`, `Saves/Substructures/Gen12/SAV2Offsets.cs`, `PKM/PK2.cs`, `SaveUtil.cs` (`IsG2*`).
- Detect: length 0x8000 (or 0x10000), then list tests (`HasListAt(party, current box)`): GS INT party 0x288A & box 0x2D6C; **Crystal INT 0x2865 & 0x2D10**; GS JP 0x2D10 & 0x283E ... (see `IsG2*` in SaveUtil.cs). Korean GS: 0x2DAE & 0x28CC.
- Common: options 0x2000, `Trainer1` = 0x2009: **TID u16 BE at 0x2009, OT name at 0x200B** (11 bytes INT, 6 JP/KOR). Rival INT 0x2021, JP 0x2017.
- GS INT: money 0x23DB (3-byte BE... stored as 3-byte value), **Johto badges byte 0x23E4, Kanto badges byte 0x23E5** (bitmasks), **party 0x288A**, play time 0x2053.
- Crystal INT: money 0x23DC, **Johto badges 0x23E5, Kanto 0x23E6**, **party 0x2865**, play time 0x2052, player gender 0x3E3D.
- JP GS: badges 0x23C5(+1), party 0x283E; JP Crystal: badges 0x23C7(+1), party 0x281A. KOR: badges 0x23DC(+1), party 0x28CC.
- GS vs Crystal: the list-position test above is the only difference PKHeX uses (the two layouts differ by a fixed shift; no version byte).
- Party list: same shape as Gen 1 but 48-byte mons: `[count][species x7 incl 0xFF][6 x 48][6 x OT 11][6 x nick 11]` = 1+7+288+66+66 = 428 (73... PKHeX `SIZE_2ULIST`=73 is the box-list size; party is 428 incl. names).
  Slot i mon at `P+8+48*i`. A species byte of **0xFD in the list = Egg** (the mon block still has the real species).
- 48-byte mon (PK2.cs): 0 species (**national id**, 1..251), 1 held item, **2..5 moves**, 6-7 OT ID (BE), 8-0xA exp, 0xB-0x14 stat exp, 0x15-0x16 DVs, 0x17-0x1A PP, 0x1B friendship/egg cycles, 0x1C pokerus,
  0x1D-0x1E caught data (BE), **0x1F level**, 0x20 status, 0x22-0x23 cur HP (BE), 0x24-0x25 max HP, then stats.
- Checksum: u16 LE sum of bytes `Trainer1 .. AccumulatedChecksumEnd` (GS INT 0x2009..0x2D68 stored at 0x2D69 and 0x7E6D; C INT 0x2009..0x2B82 stored 0x2D0D and 0x1F0D).
- RTC footer: see Part A4. 

## B3. Gen 3 (Ruby/Sapphire/Emerald/FireRed/LeafGreen)
Source: `Saves/SAV3.cs`, `Saves/Substructures/Gen3/SAV3BlockDetection.cs`, `Saves/Blocks/Gen3/SaveBlock3{Small,Large}{RS,E,FRLG}.cs`, `PKM/PK3.cs`, `PKM/Util/PokeCrypto.cs`.
- File = 0x20000: two save slots (A at 0x0000, B at 0xE000), 14 sectors of 0x1000 each per slot (sectors 0x1C000.. = hall of fame, e-Reader etc., ignore).
- **Sector footer** (offsets inside the 0x1000 sector): data 0x000..0xF7F, `0xFF4 u16 section id`, `0xFF6 u16 checksum`, `0xFF8 u32 signature 0x08012025`, **`0xFFC u32 save index`**.
  Sectors inside a slot are stored **rotated**, so map section id -> sector by reading 0xFF4. Valid slot = all ids 0..13 present.
- Current slot = the slot whose sectors have the **higher save index** (u32 at 0xFFC; PKHeX compares the first sector's counter; treat 0xFFFFFFFF as uninitialised). Only slot A for 0x10000 files.
- Checksum (optional): sum of all u32 LE words in the 0xF80 data bytes, then `(sum >> 16) + (sum & 0xFFFF)` as u16.
- Section ids: **0 = trainer info ("small")**, **1-4 = team/items/flags ("large", 4 x 0xF80 = concatenate in id order)**, 5-13 = PC boxes (9 x 0xF80).
- Small block (id 0, offsets inside the 0xF80 data): `0x00` name (8 bytes, 7 chars + 0xFF; JP 5 chars), `0x08` gender, **`0x0A` TID u16, `0x0C` SID u16**, `0x0E` hours u16, `0x10` min, `0x11` sec, `0x12` frames.
  Japanese = `u16 at 0x06 == 0`. Security key (Emerald) u32 at 0xAC.
- **Game detection** (PKHeX `GetVersionG3SAV`, reading small-block u32 at 0xAC): `== 1` -> FireRed/LeafGreen (FR vs LG not distinguishable); `== 0` -> Ruby/Sapphire;
  otherwise if any non-zero byte in small block `[0x890..0xF2C)` -> Emerald else R/S. Small block used sizes: RS 0x890, E 0xF2C, FRLG 0xF24.
- **Party** (inside the concatenated large block, i.e. section id 1 data): RS **count u8 @0x234, mons @0x238**; E **0x234 / 0x238**; FRLG **count @0x034, mons @0x038**. 6 slots x **100 bytes**.
- Badges: 8 consecutive **event flags** in the large block: flag array base RS 0x1220, E 0x1270, FRLG 0xEE0; first badge flag index RS 0x807, E 0x867, FRLG 0x820.
  So RS: flag 0x807+n -> byte `0x1220 + (f>>3)`, bit `f&7` (starts at bit 7 of byte 0x1320). E: base 0x1270, bytes 0x137C.. ; FRLG: base 0xEE0 -> 0xFE4 bit 0.. (n=0..7).
  Also: Emerald money 0x490, RS money 0x490, FRLG 0x290 (u32, Emerald/RS XOR with security key at small 0xAC; FRLG as well).
- **PK3 (100 bytes in party)**: 0x00 PID u32, **0x04 OID u32 (TID low u16, SID high u16)**, 0x08 nickname (10 bytes, 0xFF terminated), 0x12 language, 0x13 flags (bit1 = has species, bit2 = egg),
  0x14 OT name (7 bytes), 0x1C checksum u16, 0x1E padding, **0x20..0x4F = 48 bytes encrypted data in 4 blocks of 12**, then party-only stats:
  0x50 status u32, **0x54 level u8**, 0x55 mail id, 0x56 cur HP u16, 0x58 max HP u16, atk def spe spa spd u16.
- **Decryption**: `key = PID ^ OID` (u32). XOR every u32 of the 48 bytes with `key`. Then un-shuffle with `row = BlockPosition[PID % 24]` (table below): **plain block i = encrypted block row[i]** (PKHeX `Decrypt3` calls Shuffle3 with the row as-is),
  i.e. `plain[i] = enc[row[i]]`. Substructure ids (plain order): **0 = G (growth), 1 = A (attacks), 2 = E (EVs/condition), 3 = M (misc)**.
  After un-shuffling: G @0x20: `0x20 species u16 (INTERNAL id)`, `0x22 held item u16`, `0x24 exp u32`, `0x28 pp ups`, `0x29 friendship`;
  A @0x2C: **moves u16 at 0x2C 0x2E 0x30 0x32**, PP bytes 0x34..0x37; E @0x38 EVs; M @0x44: pokerus 0x44, met location 0x45, origins u16 0x46, IVs/egg/ability u32 0x48 (bit30 egg, bit31 ability).
  Checksum = u16 sum of the 24 u16 words 0x20..0x4F (verification only).
  **Shuffle table** (PKHeX `PokeCrypto.BlockPosition`, copied; row index = PID%24 for Gen 3, `sv` for Gen 4-5; row[i] = index of the encrypted block that becomes plain block i; 4 values per row):
  ```
   0: 0 1 2 3    1: 0 1 3 2    2: 0 2 1 3    3: 0 3 1 2    4: 0 2 3 1    5: 0 3 2 1
   6: 1 0 2 3    7: 1 0 3 2    8: 2 0 1 3    9: 3 0 1 2   10: 2 0 3 1   11: 3 0 2 1
  12: 1 2 0 3   13: 1 3 0 2   14: 2 1 0 3   15: 3 1 0 2   16: 2 3 0 1   17: 3 2 0 1
  18: 1 2 3 0   19: 1 3 2 0   20: 2 1 3 0   21: 3 1 2 0   22: 2 3 1 0   23: 3 2 1 0
  ```
  Un-shuffle: `plain_block[i] = enc_block[row[i]]`. Check with the classic table: PID%24=3 -> encrypted order G E M A; row 3 = 0 3 1 2 gives plain[1]=enc[3]=A, plain[2]=enc[1]=E, plain[3]=enc[2]=M -> G A E M. Correct.
- **Internal species id -> national**: ids 1..251 are national. 252..276 are unused filler (invalid -> 0). 277.. are Hoenn in game order:
  national for internal 277..411 (index = internal-277):
  ```
  252,253,254,255,256,257,258,259,260,261,262,263,264,265,266,267,268,269,270,271,272,273,274,275,290,291,292,276,277,285,286,327,278,279,283,284,320,321,300,301,352,343,344,299,324,302,339,340,370,341,342,349,350,318,319,328,329,330,296,297,309,310,322,323,363,364,365,331,332,361,362,337,338,298,325,326,311,312,303,307,308,333,334,360,355,356,315,287,288,289,316,317,357,293,294,295,366,367,368,359,353,354,336,335,369,304,305,306,351,313,314,345,346,347,348,280,281,282,371,372,373,374,375,376,377,378,379,382,383,384,380,381,385,386,358
  ```
  (135 entries covering national 252..386, each exactly once; derived from PKHeX `Table3InternalToNational`. Check: internal 277 = Treecko = 252.) Anything else (e.g. 412+) = invalid.
- Gen 3 move ids 1..354, item ids up to 376 (E).

## B4. Gen 4 (Diamond/Pearl/Platinum/HeartGold/SoulSilver)
Source: `Saves/SAV4.cs`, `SAV4DP.cs`, `SAV4Pt.cs`, `SAV4HGSS.cs`, `SAV4Sinnoh.cs`, `Saves/Substructures/Gen4/SAV4BlockDetection.cs`, `SaveUtil.cs`, `PKM/PK4.cs`, `PokeCrypto.cs`.
- File = 0x80000 = **two partitions of 0x40000** (copy at 0x00000 and 0x40000). Each partition holds a *general* block (at partition offset 0) and a *storage* block (boxes).
- Block sizes (incl. footer): **DP general 0xC100, storage starts 0xC100 (size 0x121E0)**; **Pt general 0xCF2C, storage at 0xCF2C (size 0x121E4)**; **HGSS general 0xF628, storage at 0xF700 (gap 0xD8; size 0x12310)**.
- **Footer** (end of block): `block_end-0x14` u32 major counter (the "general/ storage" save counter), `-0x10` u32 minor counter, `-0xC` u32 block size (must equal block size), `-8` u32 magic **0x20060623** (or 0x20070903 Korean), last u16 = CRC16-CCITT (init 0xFFFF, poly 0x1021) of `data[..len - FooterSize]` where **FooterSize = 0x14 for DP/Pt, 0x10 for HGSS**.
- **Game detection** (`IsValidGeneralFooter2`): file length 0x80000 and, in the second partition (`0x40000`), for each candidate size S in {DP 0xC100, Pt 0xCF2C, HGSS 0xF628}: `u32 at 0x40000+S-0xC == S` and `u32 at 0x40000+S-8` is 0x20060623 or 0x20070903. The size that matches tells DP/Pt/HGSS.
  (Test the second partition because the first save is written to the latter half.) HG vs SS: byte `General[Trainer1+0x1C]` (= **General[0x80]** for HGSS) = 7 (HeartGold) / 8 (SoulSilver) (PKHeX `GameVersion.HG=7, SS=8`; D=10, P=11, Pt=12). German SoulSilver = SS = 8.
- **Choose newer copy** (`SAV4BlockDetection.CompareFooters`): for the general block compare the two footers at `base+len-0x14` (partition 0 and partition 0x40000): larger major counter (u32) wins; on tie, larger minor counter (u32); on tie, partition 0.
  0xFFFFFFFF counters = uninitialised (loses). The storage block is compared independently (not needed for party).
- Offsets **inside the general block** (add partition offset 0 or 0x40000):

  | | DP | Pt | HGSS |
  |---|---|---|---|
  | Trainer block (`Trainer1`) | 0x64 | 0x68 | 0x64 |
  | OT name (8 x u16, 0xFFFF terminated) | 0x64 | 0x68 | 0x64 |
  | TID u16 / SID u16 | +0x10 / +0x12 | same | same |
  | money u32 | +0x14 | | |
  | gender u8 / language u8 | +0x18 / +0x19 | | |
  | **Johto/Sinnoh badges (bitmask, bit0 = first)** | +0x1A = **0x7E** | **0x82** | **0x7E** (Johto) |
  | **Kanto badges (HGSS only)** | - | - | **Trainer1+0x1F = 0x83** (PKHeX `Badges16`) |
  | ROM code (HGSS) | - | - | +0x1C = 0x80 (7 HG / 8 SS) |
  | **party count (u8)** | **0x94** | **0x9C** | **0x94** |
  | **party first mon** | **0x98** | **0xA0** | **0x98** |
  Party slot `i` at `Party + 236*i` (**236 = 0xEC bytes**, 6 slots). (PKHeX: `PartyCount = General[Party-4]`, `GetPartyOffset = Party + 236*slot`.)
  Other: Trainer name for HGSS rival at 0x22D4, Pt 0x27E8 (not needed).
- **PK4 party format (236 bytes)**: header 0x00 PID u32, 0x04 u16 (unused/"sanity"), **0x06 checksum u16**; **0x08..0x87 = 128 bytes = 4 blocks of 32 (encrypted + shuffled)**; **0x88..0xEB = 100 bytes of battle stats (encrypted separately)**.
  - Block-data decryption: **LCRNG** `x = x*0x41C64E6D + 0x6073 (mod 2^32)`, seed = **checksum (u16 at 0x06)**; for each u16 word of 0x08..0x87 in order: `x = next(x); word ^= (x >> 16)`.
  - Battle stats 0x88..0xEB: same LCRNG over u16 words but seed = **PID**.
  - Shuffle: `sv = ((PID >> 13) & 31) % 24`; use the same 24-row table as Gen 3 (rows in the shuffle table above); un-shuffle exactly as Gen 3: `plain[i] = enc[row[i]]` (blocks of 32 bytes). Blocks: **A = 0, B = 1, C = 2, D = 3**.
  - Decrypted layout: A (0x08): species u16 **@0x08 (national id)**, held item u16 **@0x0A**, OT ID **@0x0C TID, @0x0E SID**, exp u32 @0x10, friendship 0x14, ability 0x15, language 0x17. B (0x28): **moves u16 @0x28, 0x2A, 0x2C, 0x2E**, PP 0x30-0x33, PP ups 0x34-0x37, IV/egg/nick u32 **@0x38** (bit 30 = is egg, bit 31 = is nicknamed), form (u8 >> 3) @0x40, gender flags 0x40.
    C (0x48): **nickname** 0x48..0x5D (10 x u16 + terminator, Gen 4 charset, 0xFFFF terminated). D (0x68): **OT name** 0x68..0x77 (8 x u16), met dates 0x78.., met locations 0x7E/0x80, pokeball, met level 0x84 (& 0x7F).
  - **Battle-stat part (after decryption)**: 0x88 status u32, **0x8C level u8**, 0x8D capsule, 0x8E current HP u16, 0x90 max HP u16, 0x92 atk, 0x94 def, 0x96 spe, 0x98 spa, 0x9A spd, then mail/seals.
  - Empty slot: party count < 6; slots beyond count are zero (PID 0, checksum 0). A "bad egg" has invalid checksum - skip when checksum 0.
- HGSS item ids go to 536, DP 464, Pt 467 (`MaxItemID_4_*`).

## B5. Gen 5 (Black/White, Black 2/White 2)
Source: `Saves/SAV5.cs`, `SAV5BW.cs`, `SAV5B2W2.cs`, `Saves/Access/SaveBlockAccessor5BW.cs` / `...B2W2.cs`, `Saves/Substructures/Gen5/PlayerData5.cs`, `Misc5.cs`, `PK5.cs`, `SaveUtil.cs`.
- File 0x80000, **single copy** (no mirrored slot). Main data = first 0x24000 (BW) or 0x26000 (B2W2); the rest are extra blocks (C-Gear, HoF, etc.).
- Detect (`IsValidFooter5`): `footer = data[main-0x100 .. main-0x100+info+0x10]` where (BW: main 0x24000, info 0x8C), (B2W2: main 0x26000, info 0x94);
  u16 LE at `footer[^2]` == CRC16-CCITT (init 0xFFFF, poly 0x1021) of `footer[..info]`. Try BW first, then B2W2.
- **Party**: block at **0x18E00** (both games): **party count u8 at 0x18E04**, mons start at **0x18E08**, stride **220 (0xDC)**, 6 slots. (`GetPartyOffset(slot) = 0x18E00 + 8 + 220*slot`).
- Trainer data block at **0x19400** (both; size BW 0x68, B2W2 0xB0): **OT name at +4 (16 bytes = 8 UTF-16 chars, 0xFFFF terminated)**, **TID u16 at +0x14, SID u16 at +0x16**, country +0x1C, language +0x1E, **game +0x1F (20 W, 21 B, 22 W2, 23 B2)**, gender +0x21.
- Badges: "Misc" block (BW at 0x21200; B2W2 at 0x21100): **money u32 at +0, badges u8 bitmask at +4**.
- PK5 = same format as PK4 (220 bytes in party = 136 + 84): same LCRNG seeded with checksum for 0x08..0x87 and with PID for 0x88.., same shuffle. Offsets as PK4 except: **nickname 0x48 (22 bytes, UTF-16)**, **OT 0x68 (16 bytes UTF-16)**, level at **0x8C**, moves 0x28..0x2E, species 0x08, item 0x0A. Species national (<=649).

## B6. Gen 6 (X/Y, Omega Ruby/Alpha Sapphire)
Source: `Saves/SAV6*.cs`, `Saves/Access/SaveBlockAccessor6XY.cs`, `Saves/Substructures/Gen6/MyStatus6.cs`, `Misc6XY.cs`, `Misc6AO.cs`, `PK6.cs`, `PokeCrypto.cs`.
- Size XY 0x65600, ORAS 0x76000. Detect size + u32 at `len-0x1F0` == 0x42454546 (bytes `46 45 45 42`, "BEEF" footer; the last 0x200 bytes are a block table with checksums). Distinguish XY/ORAS by size only. Single copy.
- **Party at 0x14200** (both): 6 x **0x104 (260)**-byte slots; **count byte at `0x14200 + 6*0x104`** (= 0x14818).
- MyStatus block at **0x14000**: TID u16 +0, SID u16 +2, **game u8 +4 (24 X, 25 Y, 26 AS, 27 OR)**, gender +5, **OT name at +0x48 (0x1A bytes = 13 UTF-16 chars)**.
- Badges: Misc block at 0x4200: **u8 bitmask at +0x0C** (XY and ORAS). Money u32 at +8.
- PK6 (0x104 party / 0xE8 stored): header **EC u32 @0**, 0x04 sanity, 0x06 checksum; **0x08..0xE7 = 4 blocks of 56 bytes (encrypted + shuffled)**; 0xE8..0x103 party stats.
  Decrypt: LCRNG `x*0x41C64E6D+0x6073`, **seed = EC (u32 at 0)** for both the 0xE0 bytes of blocks and the 0x1C party-stat bytes; `sv = ((EC >> 13) & 31) % 24`, same table, block size 56 bytes.
  Decrypted offsets: **species u16 @0x08, held item @0x0A**, TID 0x0C, SID 0x0E, exp 0x10, ability 0x14, **nickname 0x40 (26 bytes UTF-16)**, **moves u16 @0x5A,0x5C,0x5E,0x60**, relearn 0x62.., **OT name 0xB0 (26 bytes)**, form = `u8@0x1D >> 3`, party **level u8 @0xEC**.

## B7. Gen 7 (Sun/Moon, Ultra Sun/Ultra Moon; Let's Go brief)
Source: `Saves/SAV7*.cs`, `Saves/Access/SaveBlockAccessor7SM.cs` / `...7USUM.cs`, `Saves/Substructures/Gen7/MyStatus7.cs`, `PK7.cs`.
- Size SM 0x6BE00, USUM 0x6CC00; footer `BEEF` at `len-0x1F0` (same as Gen 6). Single copy. (3DS "main" file.)
- **Party**: SM block 4 at **0x1400**; USUM at **0x1600**. 6 x 0x104; count byte at `Party + 6*0x104`.
- MyStatus: SM **0x1200**, USUM **0x1400**; ID32 (TID/SID) u32 +0, game +4 (30 SN, 31 MN, 32 US, 33 UM), **OT name at +0x38 (0x1A bytes UTF-16)**.
- Gen 7 has no gym badges (island trials, grand trials flags in event work) - nothing simple to read.
- PK7 = PK6 layout (same offsets: species 0x08, item 0x0A, nickname 0x40, moves 0x5A.., OT 0xB0, level 0xEC, EC-seeded crypto, 56-byte blocks).
- Let's Go Pikachu/Eevee (LGPE): file 0x100000, party/PK7b stored in blocks (`Saves/SAV7b.cs`), species up to 809, moves up to 742. Not researched in detail.

## B8. Gen 8/9 (summary)
Source: `Saves/Encryption/SwishCrypto/*.cs`, `Saves/SAV8SWSH.cs`, `SAV9SV.cs`, `Saves/Access/SaveBlockAccessor8SWSH.cs` / `...9SV.cs`, `PKM/PK8.cs`/`PK9.cs`.
- SwSh/SV/LA/ZA: **SCBlock save** ("SwishCrypto"). File = encrypted block stream followed by a **32-byte SHA-256 hash** (hash = SHA256(intro_64 || payload || outro_64) with fixed 64-byte intro/outro constants in `SwishCrypto.cs`).
  1. Strip the last 0x20 bytes; XOR the payload with a **static xorpad** (`StaticXorpad` in SwishCrypto.cs, 0x7F-byte period; pad is 0x80 long but advanced by 0x7F). 
  2. Parse blocks sequentially: `key u32 LE`, then **per-block xorshift32 stream** seeded with the key: `state = key; repeat popcount(key) times: state = adv(state)` where `adv: s ^= s<<2; s ^= s>>15; s ^= s<<13`; the stream yields bytes `state` low->high, advancing `state` after 4 bytes (`SCXorShift32.cs`).
     `type = data[ofs++] ^ next()`; type codes (SCTypeCode): single values (bool 1/2, u8..u64 etc.: payload size by type), `Object` (u32 len ^ next32, then len bytes ^ stream), `Array` (u32 count ^ next32, subtype byte ^ next, then count*size bytes ^ stream).
  3. Party/trainer blocks are found by **key (u32 hash)**, not by offset. SwSh: **Party `0x2985fe5d`** (object = 6 x PK8 party records of **0x158 bytes** + count byte at `6*0x158`), **MyStatus `0xf25c070e`**, TrainerCard `0x874da6fa`, Misc/money `0x1b882b09`. SV: **Party `0x3AA1A9AD`** (6 x 0x158 party records), **MyStatus `0xE3E89BD1`**, Money `0x4F35D0DD`.
- PK8/PK9 records use the same LCRNG-with-EC + 4 shuffled blocks scheme (blocks of 80 bytes at 0x08; party stats follow), 0x148 stored / 0x158 party. Species/move ids: PK8 national; **PK9 stores an internal species id** (PKHeX `SpeciesConverter.GetNational9`): ids < 917 are national; raw 917..1025 map to national via
  ```
  982,917,918,919,920,953,954,971,972,955,956,981,960,961,977,976,963,964,928,929,930,951,952,938,939,965,966,968,924,925,974,975,996,997,998,978,967,921,922,923,940,941,962,931,973,950,932,933,934,969,970,944,945,926,927,942,943,946,947,999,1000,984,986,1009,989,985,987,988,1005,990,1010,994,992,993,995,991,1006,1003,1002,1001,1004,1007,1008,957,958,959,935,936,937,948,949,983,980,979,1017,1011,1019,1014,1015,1016,1020,1021,1023,1022,1024,1025,1018,1012,1013
  ```
  (index = raw - 917). Field offsets in PK8/PK9 (not verified except PK9 move1 @0x72, nickname @0x58 / OT @0xF8 in PK8): see PK8.cs/PK9.cs.
- BDSP: not SCBlock; flat save 0xE9828.. with its own layout (`SAV8BS.cs`). Not researched.
- Switch saves are not needed for Gen 4 tool; treat as "detect and decline" unless required.

---------------------------------------------------------------------------------------------------------------------

# PART C - Character encodings

## C1. Gen 1 (and Gen 2 mostly the same) 1 byte
Source: PKHeX `PKM/Strings/StringConverter1.cs` (`TableEN`, `TableJP`).
- Terminator **0x50**. Space **0x7F**. `0x80-0x99` = A-Z, `0x9A-0x9F` = `( ) : ; [ ]`, `0xA0-0xB9` = a-z, `0xBA/0xBB/0xBC` = `à è é`, `0xBD ù`, `0xBE À`, `0xBF Á` (international table `TableEN`), `0xC0-0xDE` = Ä Ö Ü ä ö ü È É Ì Í Ñ Ò Ó Ù Ú á ì í ñ ò ó ú º ... (the German umlauts live at `0xC0 Ä, 0xC1 Ö, 0xC2 Ü, 0xC3 ä, 0xC4 ö, 0xC5 ü`),
  `0xE1 PK`, `0xE2 MN`, `0xE3 -`, `0xE6 ?`, `0xE7 !`, `0xE8 .`, `0xEF ♂`, `0xF0 ¥`, `0xF1 ×`, `0xF2 .`, `0xF3 /`, `0xF4 ,`, `0xF5 ♀`, **`0xF6-0xFF` = 0-9**. `0x5D` = "trade OT" placeholder. 
- Japanese (`TableJP`): 0x80.. katakana (ア..), 0xB1.. hiragana, `0xF6-0xFF` digits. Full table in StringConverter1.cs (URL below). Gen 2 table (`StringConverter2.cs`) is identical for 0x50/0x7F/0x80-0xFF with a few differences (`0x4E` = line break...); Gen 2 additionally uses `0xE0 '`, `0xE4/0xE5/0xE6..`.
- Raw URL: https://raw.githubusercontent.com/kwsch/PKHeX/master/PKHeX.Core/PKM/Strings/StringConverter1.cs  and `StringConverter2.cs`.

## C2. Gen 3 1 byte
Source: `PKM/Strings/StringConverter3.cs` (`G3_EN`).
- Terminator **0xFF**; `0x00` = space. `0xA1-0xAA` = 0-9, `0xAB !`, `0xAC ?`, `0xAD .`, `0xAE -`, `0xB0 …`, `0xB1/0xB2 “ ”`, `0xB3/0xB4 ‘ ’`, `0xB5 ♂`, `0xB6 ♀`, `0xB7 $`, `0xB8 ,`, `0xBA /`, **`0xBB-0xD4` = A-Z, `0xD5-0xEE` = a-z**, `0xF0 :`, `0xF1 Ä`, `0xF2 Ö`, `0xF3 Ü`, `0xF4 ä`, `0xF5 ö`, `0xF6 ü`. Accents: `0x01 À ... 0x1A à ... 0x2A º`, etc. (full 256-entry table in the source file above). Japanese table = `G3_JP` in the same file (0x01.. hiragana).
- Raw URL: https://raw.githubusercontent.com/kwsch/PKHeX/master/PKHeX.Core/PKM/Strings/StringConverter3.cs

## C3. Gen 4 16-bit table (HGSS/DPPt)
- Strings are **u16 LE code units**, terminated by **0xFFFF** (also stop at 0x0000).
- Machine-readable copy: **`gen4-charmap-intl.json`** in this folder (hex code -> character, 493 entries 0x0001..0x01EC, generated from PKHeX `StringConverter4Util.TableINT`), e.g. digits 0x0121..0x012A = 0..9, 0x012B.. = A.., 0x0145.. = a.., 0x00E1 = full-width !, 0x01AB = !, 0x01AC = ? (SaveInvalidAs), 0x01DE = space (verify against the JSON, which is authoritative).
- Primary source URL: https://raw.githubusercontent.com/kwsch/PKHeX/master/PKHeX.Core/PKM/Strings/StringConverter4Util.cs (arrays `TableINT` index = code, and `TableKOR` starting at code 0x400).
- pret charmap (also complete, includes control codes): https://github.com/pret/pokeheartgold/blob/master/charmap.txt (the build uses `charmap.txt` at the repo root; contains lines `'A' = 012B` style). Not fetched/verified.
- Codes 0x246D / 0x246E (half-width male/female) appear in names: PKHeX normalises `♂ ♀`.
- Decoding trainer-name text inside the ROM message files also uses this table (Part D).

## C4. Gen 5+ (UTF-16)
- Gen 5 (PK5, save OT names) and Gen 6/7 (PK6/PK7, MyStatus) store names as plain **UTF-16LE**, terminator 0x0000 (Gen 5 also 0xFFFF), padded. Name fields: Gen 5 nick 22 bytes (11 chars), OT 16 bytes; Gen 6/7 26 bytes (13 chars). Gen 8: 26 bytes.
  Special mappings in PKHeX (`StringConverter5/6/7.cs`): Private-use/gender glyphs `0xE08E ♂`, `0xE08F ♀` in Gen 5-7 -> `♂ ♀`; `0xE0A6-ish` apostrophe variants. Otherwise ordinary Unicode.

---------------------------------------------------------------------------------------------------------------------

# PART D - HGSS ROM (German SoulSilver, code IPGD) **[verified on ROM unless said otherwise]**

ROM file: `...\scratchpad\rom\Pokemon Silberne Edition - Hard Mod.nds` (134,217,728 bytes = 128 MiB; header says used size 0x7AB9D50).
Header (title `POKEMON SS`, code `IPGD`, maker `01`, version 0).

## D1. NDS header fields for the file system
| Offset | Size | Meaning | This ROM |
|---|---|---|---|
| 0x00 | 12 | title | `POKEMON SS` |
| 0x0C | 4 | game code | `IPGD` |
| 0x20 / 0x2C | u32 / u32 | ARM9 rom offset / size | 0x0794D000 / 0x111ED8 (**non-standard: ARM9 sits near the end; the ROM was repacked**) |
| 0x30 | u32 | ARM7 rom offset | 0x2F7400 |
| **0x40 / 0x44** | u32 / u32 | **FNT offset / size** | 0x31EC00 / 0xB56 |
| **0x48 / 0x4C** | u32 / u32 | **FAT offset / size** | 0x31F800 / 0x1008 (-> 0x1008/8 = **513 files**) |
| 0x80 | u32 | total used ROM size | 0x7AB9D50 |
- **FAT** entry (8 bytes, index = file id): `u32 start`, `u32 end` (**absolute ROM offsets**). Size = end-start.
- **FNT**: starts with the directory table: entry `d` (8 bytes, `d` = 0 is root) = `u32 subtable_offset` (relative to FNT start), `u16 first_file_id`, `u16 parent` (root's parent field = number of directories). Directory id values in sub-tables are `0xF000 + d`; root = 0xF000.
  Sub-table: sequence of entries: `type byte`; `len = type & 0x7F`; `len` ASCII chars; if `type & 0x80` a u16 directory id follows (it is a directory) else it is a file; file ids are assigned sequentially starting at the directory's `first_file_id`. A 0x00 type byte ends the list.
- **Path resolution a/0/5/5**: start at root 0xF000; entry `a` (dir) -> `0` (dir) -> `5` (dir); in that directory the files are named `0`,`1`,`2`... (decimal strings), file `5` gets file id = first_file_id(dir) + its position. This ROM: root contains `a/ data/ dwc/ fielddata/ msgdata/ pbr/ poketool/ tel/`.
  **Verified results**: `a/0/5/5` -> file id 184, ROM 0x2F24A00..0x2F29AEC (20,716 bytes), magic `NARC`; `a/0/5/6` -> id 185, 0x7944C00..0x794CD20 (33,056 bytes), `NARC`; `a/0/2/7` -> id 156, 0x21E5000..0x26A10C8 (4,964,552 bytes), `NARC`.
  (Vanilla pret layout: a/0/5/5 = trdata, a/0/5/6 = trpoke, a/0/2/7 = message archive. Confirmed by decoding below.) Other NARCs seen: a/0/0/2, a/0/0/3, a/0/1/6.

## D2. NARC container
```
0x00  "NARC"  | u16 0xFFFE (BOM, bytes FE FF) | u16 version 0x0100 (bytes 00 01) | u32 total size | u16 header size 0x10 | u16 section count 3
0x10  "BTAF" | u32 section size | u16 file count | u16 reserved | file_count x {u32 start, u32 end}   (offsets relative to the GMIF data start, 4-byte aligned)
...   "BTNF" | u32 section size | name table (usually just a minimal FNT, 8 bytes of root dir entry; unused here)
...   "GMIF" | u32 section size | file data. File i = gmif_data[start_i .. end_i]
```
`gmif_data` = (offset of GMIF section) + 8. Verified header bytes for a/0/5/5: `4E415243 FEFF 0001 EC500000 1000 0300` (size 0x50EC).

## D3. Trainer data (**verified**; NARC a/0/5/5 = trdata, a/0/5/6 = trpoke, both 738 files; file index = trainer id)
Source: pret/pokeheartgold `include/trainer_data.h`, `files/poketool/trainer/trdata.json.txt`, `trpoke.json.txt`, `src/trainer_data.c`.

**trdata (a/0/5/5) record = 20 (0x14) bytes**:
```
0x00 u8  trainerType   bit0 (1) = has custom moves, bit1 (2) = has held item   (TRTYPE_MON 0, _MOVES 1, _ITEM 2, _ITEM_MOVES 3)
0x01 u8  trainerClass  (class id; names in msg 730)
0x02 u8  unused
0x03 u8  npoke         party size
0x04 u16 items[4]      trainer's battle items
0x0C u32 aiFlags
0x10 u32 doubleBattle  (0/1)
```
**trpoke (a/0/5/6): file `i` = party of trainer `i`**, `npoke` records back to back (file padded to 4 bytes; for npoke == 0 an 8-byte dummy). **Record size depends on trainerType - HGSS records carry a trailing u16 `capsule` field, so the sizes are 8 / 10 / 16 / 18, NOT 6/8/14/16** (6/8/14/16 would be the DP/Pt layout without the capsule field):
```
type 0 (no item, default moves) 8 bytes : difficulty u8 | genderAbility u8 | level u16 | species u16 | capsule u16
type 1 (moves)                  16      : ... species u16 | moves[4] u16 | capsule u16
type 2 (item)                   10      : ... species u16 | item u16 | capsule u16
type 3 (item + moves)           18      : ... species u16 | item u16 | moves[4] u16 | capsule u16
```
- Offsets inside a record: 0 difficulty, 1 gender/ability override, 2 level (u16!), 4 species+form, then `item` (if type&2), then `moves` x4 (if type&1), then `capsule` (ball capsule seal). Item comes **before** moves.
- `species word`: **bits 0-9 = species (national id), bits 10-15 = form**. `species = w & 0x3FF`, `form = w >> 10`.
- `genderAbility`: bits 0-3 gender override (0 none, 1 male, 2 female), bits 4-7 ability override (0 none, 1 ability 1, 2 ability 2).
- IVs (all six) = `difficulty * 31 / 255` (integer, game code `iv = (u8)((difficulty * 31) / 255)`); so 250 -> 30, 255 -> 31, 200 -> 24.
- Default moves (types 0/2) = the last 4 level-up moves at that level (not stored).
- **This ROM**: type histogram over 738 trainers = type0: 119, type1: 545, type2: 4, type3: 70. All leaders/E4 are type 3 (18-byte records), file size = npoke*18 rounded up to 4. Consistency check (file size >= npoke*recordsize) passed for all 738.
- Nature/PID not stored (derived from `difficulty + level + species + trainerId`).

**Trainer ids (pret `include/constants/trainers.h`), names checked against the ROM's own text (all matched):**
| id | German name (msg 729) | class (msg 730) | notes |
|---|---|---|---|
| 20 | Falk | Leiter | Falkner (Violet) |
| 21 | Kai | Leiter | Bugsy |
| 30 | Bianka | Leiterin | Whitney |
| 31 | Jens | Leiter | Morty |
| 32 | Norbert | Leiter | Pryce |
| 33 | Jasmin | Leiterin | Jasmine |
| 34 | Hartwig | Leiter | Chuck |
| 35 | Sandra | Leiterin | Clair |
| 245 | Willi | Top Vier | Will |
| 247 | Koga | Top Vier | Koga |
| 418 | Bruno | Top Vier | Bruno |
| 246 | Melanie | Top Vier | Karen |
| 244 | Siegfried | Champ | Lance (champion) |
| 253 | Rocko | Leiter | Brock |
| 254 | Misty | Leiterin | Misty |
| 255 | Major Bob | Leiter | Lt. Surge |
| 256 | Erika | Leiterin | Erika |
| 257 | Janina | Leiterin | Janine |
| 258 | Sabrina | Leiterin | Sabrina |
| 259 | Pyro | Leiter | Blaine |
| 261 | Blau | Leiter | Blue |
| 260 | Rot | PKMN-Trainer | Red (Mt. Silver) |
| 675 | Siegfried | PKMN-Trainer | Lance (Dragon's Den / Lance in Mahagoni? separate 1-mon trainer; class `TRAINERCLASS_PKMN_TRAINER_LANCE`) |
Rematch/other-version entries (pret): 702 Will_2, 705 Karen_2, 703 Koga_2, 704 Bruno_2, 701 Champion Lance_2, 733 Champion Lance_3, 712..727 leaders (Falkner_2 712 ... Blue_2 727), 734 Clair_3.
(Order of Johto E4 in the game: Will, Koga, Bruno, Karen, Lance.)
Class ids (pret `trainer_class.h`): Leader Falkner 66, Bugsy 67, Whitney 70, Morty 72, Pryce 73, Jasmine 74, Chuck 75, Clair 76, Champion 86, Elite Four Will 87, Karen 88, Koga 89, Bruno 112, Brock 98, Misty 103, Surge 104, Erika 105, Janine 106, Sabrina 107, Blaine 108, Red 109, Blue 110, Lance(trainer) 123, Rival 23.
Example data read from this ROM (modified): Falk: Taubsi L9 + Tauboga L13 (type 3); Rot: six Level 100 mons with custom items/moves. So the ROM IS modified; always read the real ROM, not vanilla tables.

## D4. Message files (Gen 4 text) in NARC a/0/2/7 (829 files; file index = message bank number, **verified**)
| bank (file index) | content |
|---|---|
| **237** | species names (index = national id; 496 entries; 0 = "-----"; German upper case e.g. `BISASAM`) |
| **750** | move names (index = move id; 468 entries) |
| **222** | item names (537 entries; 0 = "Keines") |
| **729** | trainer names (738 entries, index = trainer id; stored in the "compressed" 9-bit form, see below) |
| **730** | trainer class names (129 entries, index = class id) |
(pret also: abilities msg_0720? not verified; types etc. not read.)

**Message file format (decoded and verified)**
```
u16 count | u16 key
count x { u32 offset ^ k, u32 length ^ k }       (table at 0x04, 8 bytes per entry; length = number of u16 code units)
text blob: entry i at `offset` (relative to file start), `length` u16 units, each XOR-encrypted
```
- Table key: `k1 = (key * 0x2FD) & 0xFFFF`; for entry `i` (0-based): `k2 = (k1 * (i+1)) & 0xFFFF; k2 = k2 | (k2 << 16)`; `offset ^= k2; length ^= k2`.
- Entry text: `ek = (0x91BD3 * (i+1)) & 0xFFFF`; for each unit: `c = unit ^ ek; ek = (ek + 0x493D) & 0xFFFF`.
- Decode `c` with the Gen 4 16-bit table (Part C3 / `gen4-charmap-intl.json`); terminator `0xFFFF`.
- Control codes: `0xFFFE` starts a command: `0xFFFE, command u16, nargs u16, args...` (e.g. variables). Not needed for names. (pret `tools/msgenc/MessagesDecoder.cpp`.)
- **Trainer-name banks use a packed encoding**: if the first decoded unit is `0xF100` (pret calls it `{TRNAME}`), the rest is a stream of **9-bit characters packed 15 bits per u16 word** (bit order LSB first; units after the 0xF100). Decode: `cont |= (unit & 0x7FFF) << bits; bits += 15; while bits >= 9 { ch = cont & 0x1FF; cont >>= 9; bits -= 9; if ch == 0x1FF stop; emit ch }`.
  `ch` is a normal Gen 4 table code. (pret `DecodeTrainerNameMessage` implements the same: shift 9, advance word after 15 bits.) **Verified**: decodes e.g. bank 729 index 21 -> `Kai`, 33 -> `Jasmin`, 260 -> `Rot`.
- Special glyphs in names: code 0x1E1 = `⒆` and 0x1E2 = `⒇` are the "PK"/"MN" ligature pair -> display as `PKMN`; class names like `⒆⒇-Trainer` mean "PKMN-Trainer". Gen 4 codes 0x01DE = space.
- German species names in this ROM are upper-case (`TAUBSI`); moves are mixed case (`Kaskade`, `Erdbeben`). Item names mixed case. Names for "no item" = `Keines`.

## D5. How the tool should read it (algorithm outline, no code)
1. Read header: FNT/FAT offsets and sizes (0x40..0x4F). 2. Resolve `a/0/5/5`, `a/0/5/6`, `a/0/2/7` through FNT; get absolute start/end from FAT. 3. Parse NARC (BTAF/GMIF). 4. For trainer id `t`: trdata file `t` -> type, class, npoke; trpoke file `t` -> `npoke` records of the size from `type`.
5. Names: msg bank 729[t], class 730[class], species 237[sp & 0x3FF], moves 750[m], items 222[item].

---------------------------------------------------------------------------------------------------------------------

# What I could NOT verify (or only partly)

- **No real save files were available**: all save offsets (Part B) come from PKHeX source (current master) and were not tested against a real .sav. HGSS/Pt/DP party offsets (0x98/0xA0), HGSS ROM-code byte (0x80, HG=7, SS=8) and badge bytes (0x7E / 0x83) are from PKHeX as quoted; confirm with a real German SoulSilver save first.
- Gen 3/4 block shuffle: table copied from PokeCrypto.cs; direction `plain[i] = enc[row[i]]` derived from PKHeX Shuffle() and checked by hand on PID%24=3 only; unit-test against a real PK3/PK4.
- Gen 3 FireRed vs LeafGreen cannot be told apart from the save (PKHeX defaults to FR).
- Emulator footers: exact RTC footer sizes of mGBA/Gambatte/SameBoy/BGB are not verified from their sources (only the DeSmuME and No$GBA formats were verified from DeSmuME source, and the PKHeX 0x0C..0x30 tolerance). melonDS/Citra/Switch container notes are general knowledge, not checked in code.
- DS .dsv footer `size` fields were taken from DeSmuME `mc.cpp` source, not tested on a real .dsv.
- Gen 8/9 (SwSh, SV, BDSP) only summarised; the 0x7F-period static xorpad bytes are in `SwishCrypto.cs` (not copied), PK8/PK9 field offsets not listed fully. LGPE/BDSP/LA/ZA not researched.
- Gen 4 char table: the JSON covers only the international table (493 codes); the Korean table (>= 0x400) is not exported. The pret `charmap.txt` URL is from memory (not fetched).
- Gen 5 B2W2 Misc/badge offset (0x21100+4) and Gen 6 ORAS/XY badge byte (Misc+0x0C) come from PKHeX; not tested.
- ROM: only trainer/message data was decoded. The ROM is modified (ARM9 relocated to 0x794D000, 70 type-3 trainers with custom sets); species/move name banks 237/750/222 verified by reading only the first few entries. The moves/ability/personal NARCs were not examined. HGSS `a/0/0/2` (personal?) etc. not identified.
- Gen 1/2 charmap specifics for German games (extra umlaut codes) taken from PKHeX tables, not from pokered/pokecrystal charmaps.


---------------------------------------------------------------------------------------------------------------------

# Gen 4 bag / TM pocket

Source: PKHeX.Core `Items/Bags/PlayerBag4DP.cs`, `PlayerBag4Pt.cs`, `PlayerBag4HGSS.cs`, `Saves/Substructures/Inventory/Pouch/InventoryPouch4.cs`,
`Items/ItemStorage4.cs`, `PKM/Util/Conversion/ItemConverter.cs`. Offsets are inside the **general block** (partition start + offset).

| Game | Bag base | TM/HM pouch offset | Absolute in general block |
|---|---|---|---|
| DP | 0x624 | +0x35C | 0x980 |
| Platinum | 0x630 | +0x35C | 0x98C |
| HGSS | 0x644 | +0x35C | 0x9A0 |

- Slot = 4 bytes: u16 item id, u16 count (little-endian), empty slot = 0/0. The list is a compacted inventory, NOT indexed by TM
  number (the real save has TM70 in slot 0): scan all slots and use the item id as the key. Max count 99 (HMs: 1).
  PKHeX reads **100 slots** (= length of the `Machine` list).
  The pouch region is 0x190 bytes (100 slots) in DP/Pt; in HGSS the next pouch (mail) starts 4 bytes later (0x4F0 vs 0x4EC), so HGSS has
  one spare 101st slot, always empty in the real save.
- Item ids: **TM01..TM92 = 328..419** (TMn = 327+n), **HM01..HM08 = 420..427** (HMn = 419+n) in DP, Pt and HGSS (same list `Machine`).
  Map to moves via PokeAPI machines.csv (vg 8/9/10; machine_number n for TM, 100+n for HM); not via PokeAPI item_id.
- Other pouch offsets, same bag base: items 0x000 (165 slots), key items 0x294, TMs 0x35C, mail 0x4EC (HGSS 0x4F0), medicine 0x51C (HGSS 0x520),
  berries 0x5BC, balls 0x6BC (HGSS 0x6C0), battle items 0x6F8 (HGSS 0x720).

**[verified on Max's German SoulSilver .dsv]** (512 KiB, partition 0x0 and 0x40000, general block 0xF628). The block footer is the last
0x10 bytes: u32 counter at `block_end-0x10` (NOT -0x14), then u32 size (0xF628), u32 0x20060623, u32 with the checksum in its low half.
Counters: partition 0 = 6, partition 0x40000 = 7, so 0x40000 is the newer one. Offsets checked: key items (0x294), medicine, balls,
battle items all contain plausible ids at the PKHeX offsets, so the layout is right. TM/HM pocket (id -> TM):
- partition 0x0 (older, counter 6): one entry, slot 0 = item 397 = TM70 (Blitz / Flash) x1; no HMs.
- partition 0x40000 (newer, counter 7): TM/HM pocket **empty** (no TMs, no HMs). This save is very early game (OT name 3 letters, only
  Pokegear-era key items, few balls/potions). So the tool must read the newer partition, and an empty pocket is a valid result.
