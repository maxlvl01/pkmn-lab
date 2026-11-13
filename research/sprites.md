# Sprite sources for runtime download (verified 2026-11-13)

All checks done with curl (User-Agent `PKMNLab-research/0.1`); sizes and alpha read from PNG IHDR/tRNS/pixel data.
Base: `https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/`  (called `$P` below). `N` = national dex number, no zero padding.
Missing file = HTTP 404 (body `404: Not Found`, text/plain), so a 404 is a clean "not available" signal.

## 1. PokeAPI sprites: version group -> front sprite

| Version group | URL (after `$P`) | Species coverage | Transparent? | Size | Notes |
|---|---|---|---|---|---|
| red-blue | `versions/generation-i/red-blue/transparent/{N}.png` | 1-151 | YES (palette + tRNS) | 96x96 | Use this. Plain `red-blue/{N}.png` is 40x40 2-bit, OPAQUE (corner alpha 255, light background). `gray/` variants are 40x40 grayscale, opaque. |
| yellow | `versions/generation-i/yellow/transparent/{N}.png` | 1-151 | YES | 96x96 | Plain `yellow/{N}.png` is 40x40 opaque; also `gbc/` (colourised, opaque). |
| gold-silver | `versions/generation-ii/gold/transparent/{N}.png` and `.../silver/transparent/{N}.png` | 1-251 | YES | 56x56 | Plain `gold/{N}.png`, `silver/{N}.png` are 40x40 OPAQUE. |
| crystal | `versions/generation-ii/crystal/transparent/{N}.png` | 1-251 | YES | 56x56 | Plain `crystal/{N}.png` 40x40 opaque. `crystal/animated/{N}.gif` also exists (not checked in detail). |
| ruby-sapphire | `versions/generation-iii/ruby-sapphire/{N}.png` | 1-386 complete | YES (palette + tRNS) | 64x64 | |
| emerald | `versions/generation-iii/emerald/{N}.png` | 1-386 complete | YES | 64x64 | |
| firered-leafgreen | `versions/generation-iii/firered-leafgreen/{N}.png` | 1-386 complete | YES | 64x64 | |
| diamond-pearl | `versions/generation-iv/diamond-pearl/{N}.png` | 1-493 | YES (RGBA) | 80x80 | |
| platinum | `versions/generation-iv/platinum/{N}.png` | 1-493 | YES (palette + tRNS) | 80x80 | |
| heartgold-soulsilver | `versions/generation-iv/heartgold-soulsilver/{N}.png` | 1-493 | YES (RGBA) | 80x80 | Verified 74, 179, 25, 493 = 200; 494 = 404. |
| black-white | `versions/generation-v/black-white/{N}.png` | 1-1025 present | YES (palette + tRNS; some RGBA) | 96x96 | 650-1025 are community "Gen 5 style" fill-ins (see repo CONTRIBUTING_SPRITES.md), not official. Animated: `black-white/animated/{N}.gif` (e.g. 393 = 30x41 GIF). |
| black-2-white-2 | NO FOLDER (404) | - | - | - | PokeAPI's sprite repo has no B2W2 front sprites. Use `black-white`. |
| x-y | `versions/generation-vi/x-y/{N}.png` | 1-721 complete | YES (palette + tRNS or RGBA) | VARIABLE (25 = 60x60, 658 = 128x73, 721 = 95x93) | Sprites are cropped to content, so size differs per species. Layout must not assume square. 722 = 404. |
| omega-ruby-alpha-sapphire | `versions/generation-vi/omega-ruby-alpha-sapphire/{N}.png` | 1-721 complete | YES (RGBA) | VARIABLE (658 = 119x69) | Same crop behaviour. |
| sun-moon | NO FOLDER (404) | - | - | - | Only `ultra-sun-ultra-moon` exists. |
| ultra-sun-ultra-moon | `versions/generation-vii/ultra-sun-ultra-moon/{N}.gif` | 1-~807 (+forms); 809 = 404 | animated GIF | VARIABLE (25 = 77x60) | NOT PNG. `.png` = 404. Rendered 3D-model animation, not the same style as older sprites. |
| lets-go-pikachu-lets-go-eevee | `versions/generation-vii/lets-go-pikachu-lets-go-eevee/{N}.gif` | only 188 files (1-151 + Meltan/Melmetal + forms); 152 = 404 | GIF | 384x384 | PNG icons only under `.../icons/{N}.png`. |
| sword-shield | `versions/generation-viii/sword-shield/{N}.webp` | most species | animated WebP with alpha (VP8X alpha+anim flags set) | 512x512, ~520 KB each | `.png` = 404. Heavy; needs `image` crate "webp" feature and animated decoding gives only frame 1. Icons: `sword-shield/icons/{N}.png`. |
| brilliant-diamond-shining-pearl | `versions/generation-viii/brilliant-diamond-shining-pearl/{N}.webp` | most species | animated WebP, alpha | 512x512, ~546 KB | `.png` = 404. Icons: `.../icons/{N}.png`. |
| scarlet-violet | `versions/generation-ix/scarlet-violet/{N}.webp` | 998 files (1-1025 not complete; e.g. 25 and 1025 are 200) | animated WebP, alpha | 512x512, ~900 KB | `.png` = 404. Icons `.../icons/{N}.png` (1018 files). |

Other folders exist but were not requested: `legends-arceus`, `legends-za`, `champions`, `red-green-japan`, `generation-{iii,iv,v,vi,vii}/icons`.

Gen 1 and Gen 2 gray/opaque summary: opaque (background colour baked in) are the plain `red-blue/{N}.png`, `yellow/{N}.png`, `gold|silver|crystal/{N}.png` and all `gray/` and `gbc/` folders. The `transparent/` subfolders are the ones to use (also available as `back/`, `shiny/` variants). Gen 3-6 and the default folder are transparent without needing a subfolder. Verified by reading the top-left pixel alpha: transparent folders = 0, plain Gen 1/2 = 255.

Tree check: Gen 3 ruby-sapphire has 389 png (386 + a few forms), emerald/frlg 391; DP 501, Pt/HGSS 508 files (493 + forms, ids 10xxx are forms). Female variants live in `.../female/{N}.png`, shiny in `.../shiny/{N}.png`, back in `.../back/{N}.png`.

### Default, HOME, official artwork

| Kind | URL (after `$P`) | Transparent | Size | Verified |
|---|---|---|---|---|
| default | `{N}.png` | yes | 96x96 (393 is Gen 5 style palette; 1025 RGBA) | 393, 1025 = 200 |
| HOME | `other/home/{N}.png` | yes (RGBA) | 512x512 | 393, 1025 = 200 |
| official artwork | `other/official-artwork/{N}.png` | yes (RGBA) | 475x475 | 393, 1025 = 200 |

Forms use ids like 10015 in the same folders.

## Recommended fallback order (per game)

1. Game's own folder from the table (Gen 1/2: the `transparent/` subfolder).
2. For games with no folder or non-PNG (black-2-white-2 -> `black-white`; sun-moon, ultra-*, lets-go, sword-shield, BDSP, scarlet-violet): `other/home/{N}.png` (512x512 PNG, easy to decode, consistent look across all gens). If you want pixel-art feel, `black-white/{N}.png` (96x96, covers 1-1025) is a good alternate for those games.
3. Any 404 -> default `{N}.png`, then `other/official-artwork/{N}.png`.
Avoid WebP/GIF 3D renders in egui (animated, 0.5-0.9 MB, extra decoder features). Draw all sprites with nearest-neighbour filtering for pixel art (up to 96x96) and linear for HOME/official artwork. Don't assume square sizes (x-y / oras vary).

For a HGSS-focused app: HGSS 1-493 covers the game's whole dex (80x80 RGBA).

## 3. Trainer sprites (Pokemon Showdown)

URL: `https://play.pokemonshowdown.com/sprites/trainers/{name}.png` (the directory index `.../sprites/trainers/` returns an HTML listing, 1500 files; I parsed it for names).
Format: PNG, palette + tRNS, 80x80, transparent (corner alpha 0). Served with `Cache-Control: max-age=691200`, ETag, behind Cloudflare.

Naming: lowercase, no spaces, English name. Suffix variants:
- `{name}` = base/modern sprite (usually the newest or main portrait).
- `{name}-gen1` (45), `-gen1rb`, `-gen2` (66), `-gen3` (129), `-gen3rs`/`-gen3frlg`, `-gen4`(41) / `-gen4dp` / `-gen4pt`, `-gen5bw`/`-gen5bw2`, `-gen6`, `-gen6xy`/`-gen6oras`, `-gen7`, `-gen8`, `-gen9`; extras `-lgpe`, `-masters`, `-masters2..5`, `-contest`, `-pokeathlon`, `-gen2c` (Crystal), `-gen2jp`, `-gen2kanto`.
- Not every trainer has every variant; 404 means none (e.g. `falkner-gen4`, `whitney-gen4`, `lance-gen4`, `red-gen4` = 404).

Verified HGSS-relevant names from the index and HTTP status (all 200 unless noted):

| Trainer | Base | Gen 2 variant | Other variants present |
|---|---|---|---|
| Falkner | `falkner` | `falkner-gen2` | - |
| Bugsy | `bugsy` | `bugsy-gen2` | `bugsy-masters` |
| Whitney | `whitney` | `whitney-gen2` | `whitney-masters` |
| Morty | `morty` | `morty-gen2` | `morty-masters`, `-masters2`, `-masters3` |
| Chuck | `chuck` | `chuck-gen2` | - |
| Jasmine | `jasmine` | `jasmine-gen2` | `-contest`, `-masters`.. |
| Pryce | `pryce` | `pryce-gen2` | - |
| Clair | `clair` | `clair-gen2` | `clair-masters` |
| Lance | `lance` | `lance-gen2` (also gen1, gen1rb, gen3, lgpe, masters) | |
| Red | `red` | `red-gen2` (also gen1, gen1rb, gen3, gen7, lgpe, masters..4) | `red-gen1main`, `red-gen1title` |
| Will / Koga / Bruno / Karen | `will`, `koga`, `bruno`, `karen` | `will-gen2`, `koga-gen2`, `bruno-gen2`, `karen-gen2` | |
| Kanto leaders | `brock`, `misty`, `erika`, `janine` ... | `brock-gen2`, `misty-gen2`, `erika-gen2`, `sabrina-gen2`, `blaine-gen2`, `janine-gen2` | Surge and Blue: check index (`blue-gen2` exists) |

Downloaded and checked (in a local scratch folder): all 15 of falkner, bugsy, whitney, morty, clair, lance, red, and their `-gen2` forms returned 200 image/png with a PNG header. `falkner.png` (809 bytes) and `falkner-gen2.png` (484 bytes) are different files (md5 differ), same for lance and red, so base and -gen2 are different artwork. Recommendation for an HGSS app: try `{name}-gen2` first (the Gen 2 Game Boy style) and then `{name}`; I did not visually compare which one matches HGSS's DS art, so a human should look at both once. Names not yet verified in this folder: Surge (`surge`?), Sabrina, etc. - grep the index file `names.txt` in the same scratch folder.

German-names rule: the file names are English by necessity; map the German in-game names to these English ids in your own table (not verified here).

## 4. Terms and etiquette

PokeAPI sprites repo:
- README says: "To save load on PokeAPI, we host all the sprite images here", and you may download the whole contents (git clone / npm / bower). So pulling files by raw URL is intended use, but cloning is the polite way for bulk.
- The repo has NO LICENSE file (LICENSE / LICENSE.md / LICENCE return 404 on raw; the GitHub licence API was rate-limited when I tried, so I could not double-check there). The images are Nintendo / Game Freak / The Pokemon Company assets plus community fill-ins; treat them as fan use only, non-commercial, no redistribution bundled in your app. That fits your "download at runtime, don't bundle" plan.
- raw.githubusercontent.com has no explicit hotlink policy but is rate limited (unauthenticated) and meant for light use; GitHub is not a CDN contract.

Pokemon Showdown:
- Sprites are not part of the client repo (the client README says `/sprites/` is excluded "for size reasons"). The client is AGPLv3; sprites carry no licence statement and are likewise Nintendo/Game Freak art plus community-made sprites (the trainer pack is community drawn).
- `robots.txt` returned 200 with an empty body (no disallow rules). I found no written hotlinking ban, but also no permission. Showdown's own client loads these from that host, and responses are CDN cached (`max-age=691200`, 8 days).

Behaviour to implement:
- Download each file once, store in a local cache (path mirroring the URL), and never re-request while the file exists; on 404 store a small negative marker (e.g. empty `.404` file) so you never retry it.
- Send an honest User-Agent, e.g. `PKMNLab/0.1 (personal fan project)` (curl `-A`), do not fake a browser; no personal data in it.
- Limit concurrency (1-2 requests at once), no bulk crawl of the whole dex (fetch only what is shown), exponential backoff on 403/429/5xx, use `--fail` and write to a temp file then rename so partial downloads are not cached.
- Optionally send `If-None-Match` with the stored ETag only if you want updates; not needed for static sprites.
- Prefer a single clone/zip download (PokeAPI allows it) if you ever want the whole set offline.

Not checked: GitHub licence API (rate limit), exact species gaps beyond 1-1025 for scarlet-violet (998 files, ids not listed), Gen 7+ GIF/WebP species lists, visual look of base vs `-gen2` trainer art, Kanto/Johto trainers Surge/Sabrina name details.
