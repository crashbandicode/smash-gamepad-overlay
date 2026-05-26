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
- `local-assets/modified/info_melee/layout.arc`

Recommended first edit:

1. Open `blyt/info_melee.bflyt`.
2. Add a new root-level `Pane` under `RootPane`:
   - name: `sgpo_root`
   - position: start around `x=760`, `y=-330`
   - size: about `220 x 160`
   - alpha: `255`
3. Add one child `Picture` pane:
   - name: `sgpo_pro_a_marker`
   - position: `0,0`
   - size: about `28 x 28`
   - alpha: `255`
   - use an existing simple white texture/material if possible, such as the archive's `com_white32^s` texture reference
4. Do not add BFLAN animations for the first test.
5. Do not use names referenced by existing BFLANs.
6. Keep all names prefixed with `sgpo_`.

This should give the Rust plugin a real Smash UI pane to find and update:

```text
RootPane
  sgpo_root
    sgpo_pro_a_marker
```

The Rust renderer should then:

- Search `info_melee` for `sgpo_pro_a_marker`.
- Set it visible.
- Update alpha and scale from `ControllerViewState` when A is pressed/released.
- Leave DebugText fallback intact if the pane is missing.

## Later Visual HUD Expansion

After one static marker is confirmed stable on Switch and emulator, extend the same `sgpo_root` with unique picture panes:

- `sgpo_btn_a`, `sgpo_btn_b`, `sgpo_btn_x`, `sgpo_btn_y`
- `sgpo_btn_l`, `sgpo_btn_r`, `sgpo_btn_zl`, `sgpo_btn_zr`
- `sgpo_btn_l3`, `sgpo_btn_r3`
- `sgpo_left_gate`, `sgpo_left_dot`
- `sgpo_right_gate`, `sgpo_right_dot`

For a larger HUD, a separate `sgpo_controller.bflyt` inserted as a `Parts` pane may be cleaner. The direct root-child approach is better for the first stability test because it avoids modifying `layout.info`, creating a new BFLYT, or dealing with nested Parts traversal.
