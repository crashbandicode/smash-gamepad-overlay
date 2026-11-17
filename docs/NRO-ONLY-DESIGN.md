# NRO-only overlay: no `layout.arc` on the SD card

2026-11-17. Branch `nro-only-merge` (prototype, not released). The overlay draws into panes that a patched
`ui/layout/info/info_melee/info_melee/layout.arc` adds to the game's match HUD. Today the user generates that file
with `tools/sgpo_installer` and installs it as an ARCropolis mod, so it replaces the whole archive: any other mod
that changes `info_melee` (a HUD mod) and the overlay exclude each other. This note compares ways to drop the file
and recommends one, which the branch prototypes. Smash 13.0.5 only.

## What the overlay changes in `info_melee`

Read from `tools/sgpo_installer` (`patch_info_melee_layout`) and the 13.0.5 archive (344 members, 2,166,040 B):

- Three layouts change: `blyt/info_melee.bflyt` and `blyt/info_melee_lct_player_00/01.bflyt` (P1's HUD parts,
  where the HUD path draws).
- In each, `sgpo_root` (a copy of `RootPane`'s `pan1`) becomes the last child of `RootPane`, with 24 `pic1`
  children copied from `set_rep_stock_01` (root) or `set_rep_01` bound to `set_rep_stock_01`'s material (player
  parts). Each gets the shared name, position and size; alpha 0. That is 3,364 B per layout.
- The default square skins (`default_simple`, `default_simple_gamecube`) need nothing else: no material, texture
  or new SARC member.
- The RetroSpy skins (`switch_pro_alt_builtin`, `gamecube_tron_builtin`) also add materials, `txl1` names and
  textures to `timg/__Combined.bntx`. The working setup's file (`82a674e7…`, on the card and in every local Eden
  profile) is 4,795,344 B; its BNTX grows from 1,749,120 to 4,345,984 B.

The overlay's changes are therefore pane edits inside Nintendo's layouts, not separate SARC members. A merge has to
work at pane level; replacing whole members would still override other mods.

## Options

| | How | Other `info_melee` mods | Nintendo bytes shipped | NRO cost | Per load | Work |
| --- | --- | --- | --- | --- | --- | --- |
| A. Embedded archive (TrainingRooms' pattern) | `include_flate` a whole patched archive, swap it in at the UI-holder allocation (`main+0x3774704`) | replaced | the whole archive (2.2–4.8 MB raw) | +~200 KB–2 MB compressed, plus an inflated copy | inflate a few tens of ms (TR's 4.2 MB, `resource-costs.md` row 12) | small |
| B. Member merge | replace the three `.bflyt` members with prebuilt ones inside the archive the game loads | their changes to those three layouts lost | three patched layouts | ~90 KB | copy | small |
| **C. Pane merge at load** (recommended) | splice `sgpo_root` and its panes into the three layouts of the archive the game loads, in place | kept, unless they remove `RootPane`, the two template panes or use `sgpo_*` names | none | +16 KiB resident (measured) | ~0.25 ms host for 2.2 MB, no copy | done (default skins) |
| D. Own layout at runtime | build a separate layout from an embedded SGPO archive and attach its root under P1's HUD pane | untouched | none | small | small | large RE: layout build and pane attach calls; also live pane appends |
| E. Today | patched `layout.arc` as a mod | replaced | none in releases | 0 | 0 | none |

C keeps everything the installer already proves (the same panes, so the renderer, placements and skins are
unchanged), ships no Nintendo data and costs no extra copy of the archive.

## Where to merge: ARCropolis's file callback, not the allocation hook

- **ARCropolis callback** (`arcrop_register_callback(hash, max_size, cb)`; upstream since 394ff06, the version on
  the card is ARCropolis 4.1.1 `ac3e05a3…`): callbacks for one file form a chain, newest first
  (`src/modfs/virt.rs`). Inside a callback `arcrop_load_file` serves the next layer: the next plugin's callback,
  then another mod's file, then vanilla. Returning `false` makes ARCropolis serve that next layer unchanged. The
  file's size in the game's table becomes `max_size`.
  - Composes with other mods and with any other plugin that uses the same API.
  - The overlay already requires ARCropolis (today's layout is an ARCropolis mod), so nothing new is required.
  - No game hook: no site guard, no hook-sharing election.
- **UI-holder allocation hook** (`main+0x3774704`, TrainingRooms' `archive.rs`): sees the decompressed archive just
  before the game copies it. It is a shared site (TrainingRooms owns an inline hook there), so the overlay would
  need HOOK-SHARING's owner election and both load orders tested. It also runs on the game's task workers, so it
  needs preallocated buffers, and it comes after ARCropolis, so it can only see the final bytes.

### Coexistence with TrainingRooms

- TrainingRooms rewrites `info_training` and `info_waiting`, which it recognises by member name
  (`blyt/info_training.bflyt`, `blyt/info_waiting.bflyt`). It returns early on `info_melee`, so the two plugins
  touch different archives.
- Order: ARCropolis's layers (SGPO's callback included) produce the file; TrainingRooms' hook sees it later, at the
  allocation. If TrainingRooms ever rewrote `info_melee` with its byte-range recipe, the recipe would not match
  SGPO's merged bytes. It would then fall back to the vanilla archive (its B3 rule), and the overlay would be lost.
  Such a change in TrainingRooms should use the ARCropolis chain, or a recipe that tolerates other plugins' panes.

## The prototype (`crates/sgpo-layout-merge`, `src/layout_merge.rs`)

- The pane table (`PANE_SPECS`, templates, root position) moved into `crates/sgpo-layout-merge`. The installer
  imports it, so both paths build the same panes.
- `merge_in_place(buffer, len)` runs entirely within ARCropolis's buffer:
  1. Reads the SARC table and plans each of the three layouts: RootPane's closing `pae1`, the templates and the
     material index.
  2. Moves members forward from the last, keeping each member's alignment.
  3. Splices the new sections and fixes each FLYT's size and section count, the SFAT offsets and the archive size.
  - It never allocates or panics. It uses about 16 KiB of stack (`on_layout_load`'s frame is `0x3f50` B).
  - A layout that already has every SGPO pane is left alone, so an old patched `layout.arc` still installed as a
    mod keeps working. A layout with only some of them is refused.
- `src/layout_merge.rs`, at boot (13.0.5 only):
  - Finds `arcrop_register_callback` and `arcrop_load_file` by symbol.
  - Sizes `max_size` as the larger of vanilla's 2,166,040 B and any `ultimate/mods/*/…/info_melee/…/layout.arc`
    on the card, plus `MAX_GROWTH` (137 KiB), and registers.
  - The callback loads the next layer into ARCropolis's buffer, merges, and publishes the outcome to atomics. The
    scene update logs one line per load:
    `layout merge: load=N result=merged code=0 patched=3 already=0 base=… len=… merge_us=…`.
- **Failure:** any refusal, or a base ARCropolis could not serve, returns `false`. The game then gets the archive
  unchanged, and the overlay finds no panes and draws nothing (its existing "layout looks unpatched" log).
  Without ARCropolis the plugin logs that the layout must be installed, as today.

### Proof on the host

- `crates/sgpo-layout-merge/tests/merge.rs`, 7 tests on synthetic archives (no game data):
  - placement under `RootPane`; fields and material binding;
  - other members byte-identical, with their alignment kept;
  - idempotence, and another mod's extra panes surviving;
  - refusals (partial patch, missing member or template, not a SARC, buffer too small);
  - no panic on any truncation or single-byte corruption.
- `tools/sgpo_installer` `in_place_merge_matches_the_installer_patch`, with `SGPO_STOCK_INFO_MELEE` set to the
  player's own 13.0.5 file: the merge's three layouts are **byte-identical** to the installer's, and the other 341
  members are unchanged. The archive grows from 2,166,040 to 2,181,912 B, in 0.25 ms on the host.

## Size and memory

- **NRO:** 106,496 → 122,880 B resident (+16 KiB, `tools/build/nro_size.py`; v0.3.1 `5ab98bdf…` against the
  prototype `60e35921…`).
- **Per `info_melee` load:** ARCropolis allocates a `max_size` buffer for the callback (2,166,040 + 137 KiB with no
  other mod) and the game sizes its own buffer from the same table. That is about 2.3 MB transient, against
  2,166,040 B for vanilla and 4,795,344 B for today's patched file with the RetroSpy skins. SGPO adds no copy.
- **Textured skins:** about 2.6 MB of BNTX texture data. Embedding it in the NRO would add that much resident
  memory for every user. If they move to the NRO-only path, the textures should be SGPO's own data file (not a
  `layout.arc`), read at merge time. That also needs a BNTX append in the merge (dictionary and relocation table,
  `docs/bntx-texture-append-toolbox-cli-findings.md`) and `mat1`/`txl1` additions. Until then the installer path
  stays for those skins.

## Risks

- ARCropolis is required, and its callback API and chain semantics are its own; an ARCropolis that does not chain
  (older than 394ff06) would give the newest registrant only.
- `max_size` is fixed at boot. A mod's `info_melee` larger than any found under `ultimate/mods` (another mod root,
  or a workspace ARCropolis resolves differently) fails the base load: logged, no overlay, the game unaffected.
- The game is handed a buffer of `max_size`, with the archive shorter than it. The SARC header carries the real
  size; Eden runs show whether the game accepts this (below).
- Another mod that removes `RootPane`'s child block, `set_rep_01` or `set_rep_stock_01` makes the merge refuse.
- Recorded decisions this changes, for the operator to confirm:
  - AGENTS.md "do not clone/append panes at runtime": the merge edits file bytes before the game builds any pane,
    and no live pane is cloned.
  - AGENTS.md "extend the installer path rather than adding a second one": one pane table now serves both paths.
  - `docs/release-bundle-strategy.md`, no generated `layout.arc` in releases: unchanged; the NRO carries no game
    bytes.

## Eden (2026-11-17, XPS, Eden-GIF `e8dcc673…`, muted, Training on Final Destination, P1 Mario)

Build under test: `60e35921…` (`6525cd5`), ARCropolis `ac3e05a3…`, tile cursor `66f00442…`, nro_hook. No SGPO
`layout.arc` and no `config.json` on the card (default square skin). Guest tracing traced `on_layout_load`
(`nro:ec73069e…+0x10b4`, the NRO's build ID) on the same build.

| Run | Also loaded | Rows | Merge line | Shown |
| --- | --- | --- | --- | --- |
| `sgpo-nro-solo-xps-3` | — | 6 PASS; r07 FAILED in the shared `routes:back_to_main` (ended on Games & More, `harness-suspect`); r08–r09 not run | `load=1 result=merged code=0 patched=3 already=0 base=2166040 len=2181912` | overlay right of the CPU's HUD; 6.6 s GIF of jabs, a fireball and a jump with the panes lighting |
| `sgpo-nro-tr-tint-xps-4` | TrainingRooms 0.2.0-rc.15 `ca5a3e4f…`, a dummy `info_melee` mod (`zz-sgpo-test-hud-tint`) | 6/6 PASS | `base=2166056` (the dummy mod's length: vanilla + 16 B), `len=2181912` | overlay right of the CPU's HUD beside TrainingRooms' own UI; 6.0 s GIF |

- **Trace:** in each traced run `on_layout_load` ran once, from ARCropolis (`nro:659810fe…+0x676b4`). At entry
  x0 was the layout's Hash40 (`0x2f26ff8cc2`) and x2 `max_size` (`0x231e44`); it returned x0 = 1. 21–332 ms
  passed from entry to the published outcome, ARCropolis's load of the base included; the merge alone takes about
  0.25 ms on the host.
- **Console:** no `panic`, `abort`, `crash` or `Unmapped` line in any run. Every Down was released and restored,
  with `cleanup_failures=[]`. In `tr-tint-xps-4` the first Down hit a busy ledger lock (`BlockingIOError`) at
  release, after Eden had stopped; the retried Down released it.
- **The dummy mod's change:** the merge ran on the dummy mod's bytes (`base=2166056`). On the host, merging that
  file gives the installer's three layouts byte for byte and leaves the mod's other bytes unchanged
  (`in_place_merge_matches_the_installer_patch` with `SGPO_STOCK_INFO_MELEE` set to it). Its edits to the name
  plates did not show on screen in either form tried, a vertex-colour tint (`tr-tint-xps-2`/`-3`) or 2×/2.5×
  sizes (`-4`). The game appears to drive those plates itself. A visible third-party change is therefore **not
  yet shown**; it needs a mod whose edit the game keeps, such as a real HUD mod.
- **Earlier runs:**
  - `solo-xps-2`, `tr-tint-xps-2` and `-3` failed on harness checks and were fixed in the rows (a gate on a
    once-per-boot line; the tint).
  - `solo-xps-1` was stopped before load for the stack change.
  - Evidence: `smash-training-lobby/artifacts/phase23/sgpo-nro-only/`.
- **Not shown:** a second `info_melee` load in one boot (the shared route back to the menu failed), a console, the
  RetroSpy skins, and other ARCropolis versions.

## Recommendation

Adopt C for the default skins: the ARCropolis callback with the in-place pane merge, as prototyped. Keep the
installer for the RetroSpy skins until the BNTX append exists, then move them to an SGPO data file. Release only
after the operator's go and a console test.
