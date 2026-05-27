# Info Melee Layout Notes

These notes are from the local Smash 13.0.4 `data.arc` dump. The extracted assets are intentionally ignored under `local-assets/`.

## Extraction

- Source archive: `data.arc`
- Extracted with `smash-arc` crate path:
  - `ui/layout/info/info_melee/info_melee/layout.arc`
- Output:
  - `local-assets/original/info_melee/layout.arc`
- Unpacked with Python `sarc` package:
  - `local-assets/original/info_melee/unpacked/`

## Archive Inventory

- `blyt/`: 25 BFLYT layouts
- `anim/`: 315 BFLAN animations
- `timg/`: 1 BNTX texture bundle, `__Combined.bntx`
- `bgsh/`: 2 shader files
- `info/layout.info`: layout and animation name table

Primary BFLYT files:

- `info_melee.bflyt`: root match HUD layout
- `info_melee_lct_player_00.bflyt`: 4-player HUD player parts
- `info_melee_lct_player_01.bflyt`: compact/more-than-4-player HUD player parts
- Character-specific HUD parts include Hero/Brave, Steve/Pickel, Joker/Jack, Kazuya/Demon, Sephiroth/Trail, Inkling, Shulk/Monad, etc.

## Root Pane Tree Summary

`info_melee.bflyt` uses a 1920x1080 root pane. Coordinates are centered: right is positive X, up is positive Y.

Top-level root children:

```text
RootPane size=(1920,1080)
  time                     pos=( 906, 466) alpha=0
  bgm                      pos=(-960, 397) alpha=0
  title_special            pos=(-960, 484) alpha=0
  p1                       pos=(-825,-420) parts -> info_melee_lct_player_00
  p2                       pos=(-595,-420) parts -> info_melee_lct_player_00
  p3                       pos=(-365,-420) parts -> info_melee_lct_player_00
  p4                       pos=(-135,-420) parts -> info_melee_lct_player_00
  p1_2                     pos=(-832,-444) parts -> info_melee_lct_player_01
  p2_2                     pos=(-415,-444) parts -> info_melee_lct_player_01
  p3_2                     pos=(   2,-444) parts -> info_melee_lct_player_01
  p4_2                     pos=( 420,-444) parts -> info_melee_lct_player_01
  p5/p6/p7/p8              pos=( 837,-444) parts -> info_melee_lct_player_01
  sandbag                  pos=(1600,-420) parts -> info_melee_lct_player_00 alpha=0
  set_parts_enemy_01       pos=(-960, 466) parts -> info_melee_lct_enemy_count alpha=0
  set_parts_gsp            pos=(   5, 494) parts -> info_melee_lct_gsp alpha=0
  set_rep_stock_01..08     pos=(-980, 560) picture sprites
```

The player parts layouts contain many character-specific panes. This explains the earlier Hero MP gauge behavior: reusing or moving panes inside player parts can reveal unrelated character HUD elements.

## Visual Insertion Plan

For the first stable visual test, do not reuse player text/name panes and do not add runtime-created panes.

The repo now has a reproducible local patch script:

```sh
python tools/patch_info_melee_layout.py
python -m sarc create \
  --base-path "$PWD/local-assets/modified/info_melee/unpacked" \
  "$PWD/local-assets/modified/info_melee/unpacked" \
  "$PWD/local-assets/modified/info_melee/layout.arc"
```

This creates:

- `local-assets/modified/info_melee/unpacked/blyt/info_melee.bflyt`
- `local-assets/modified/info_melee/unpacked/blyt/info_melee_lct_player_00.bflyt`
- `local-assets/modified/info_melee/unpacked/blyt/info_melee_lct_player_01.bflyt`
- `local-assets/modified/info_melee/layout.arc`

The current edit inserts SGPO pane trees into the root match layout and the two player-parts layouts:

```text
info_melee.bflyt / info_melee_lct_player_00.bflyt / info_melee_lct_player_01.bflyt
  sgpo_root
    sgpo_pro_lt
    sgpo_pro_lb
    sgpo_pro_rt
    sgpo_pro_rb
    sgpo_pro_minus
    sgpo_pro_plus
    sgpo_pro_l3
    sgpo_pro_r3
    sgpo_pro_ls_gate
    sgpo_pro_ls_dot
    sgpo_pro_rs_gate
    sgpo_pro_rs_dot
    sgpo_pro_du
    sgpo_pro_dd
    sgpo_pro_dl
    sgpo_pro_dr
    sgpo_pro_dul
    sgpo_pro_dur
    sgpo_pro_ddl
    sgpo_pro_ddr
    sgpo_pro_btn_y
    sgpo_pro_btn_x
    sgpo_pro_btn_b
    sgpo_pro_a_marker
```

Notes:

1. `sgpo_pro_a_marker` keeps the previously tested A-button name.
2. The other panes use short names because BFLYT pane names are limited.
3. Root visual children are cloned `Picture` panes from `set_rep_stock_01`.
4. Player-parts visual children are cloned `Picture` panes from `set_rep_01`, but their material/vertex-color fields are copied from `set_rep_stock_01`. This keeps the player-HUD pause visibility behavior from `set_rep_01` while avoiding the red P1 marker material.
5. Injected panes start hidden with alpha `0`. Runtime code makes the active copy visible and updates alpha/position.
6. No BFLAN animations are added.
7. No names referenced by existing BFLANs are reused.

The root copy supports the normal `Layout::Draw` renderer. The player-parts copies support the Training Modpack compatibility renderer, which captures P1's HUD parts layout instead of hooking `Layout::Draw`.

Install `local-assets/modified/info_melee/layout.arc` as a normal Smash data replacement for:

```text
ui/layout/info/info_melee/info_melee/layout.arc
```

Do not commit or publish this generated layout file.

This gives the Rust plugin real Smash UI panes to find and update:

The Rust renderer:

- Searches `info_melee` for `sgpo_root`.
- Or, in Training Modpack compatibility mode, captures P1's HUD parts layout and searches that layout data for `sgpo_root`.
- Iterates the built-in `SkinElement` table.
- Sets each pane visible.
- Updates alpha, scale, and position from `ControllerViewState`.
- Leaves DebugText fallback intact on the normal `Layout::Draw` path if visual panes are missing.
- On the Training Modpack path, missing visual panes are logged and the overlay stays inactive, because SGPO intentionally does not install the draw hook needed by the text fallback.

## Later Visual HUD Expansion

For a more polished HUD, a separate `sgpo_controller.bflyt` inserted as a `Parts` pane may be cleaner. The direct root-child approach is still better for stability because it avoids modifying `layout.info`, creating a new BFLYT, or dealing with nested Parts traversal.
