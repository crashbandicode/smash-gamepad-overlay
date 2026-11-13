# Status and release history

Moved here from the README and rechecked on 2026-11-13 against the Forgejo
release pages and manifests, the card install receipts and the run logs named
below. Each entry is as of its date.

## On the Switch card

- v0.3.1 (`5ab98bdf…`) replaced v0.3.0 on the operator's card on 2026-11-05
  18:19Z, with TrainingRooms 0.2.0-rc.3
  (`smash-training-lobby/artifacts/release-v0.2.0-rc.3/CARD-INSTALL-RECEIPT.json`).
  The 2026-11-12 readback still shows it
  (`smash-training-lobby/artifacts/card-install-detector-0.5.1/verify.json`).
  No console log of v0.3.1 has been read back.
- v0.3.0 (`a7fba179…`) was installed 2026-11-01 and v0.2.0 (`39c7e9f0…`)
  2026-10-31 (`ssbu-cpu-pilot-install/switch-ums-sgpo-v030/` and `-v020/`,
  both `installed_verified`).

## Tested on 2026-10-14 (v0.1.0 source)

- **Switch:** Atmosphère 1.12.0 with hekate 6.5.4, the then-current Skyline
  release, ARCropolis 4.1.1, Training Modpack installed, Smash 13.0.5. Two
  online matches and Training ran in one session with the overlay loaded,
  alongside other Skyline plugins.
- **Eden:** development build `9a5d704c9c` (2026-10-14), isolated profile,
  Smash 13.0.5, with Skyline, ARCropolis and Training Modpack. The overlay
  loaded in every run that day and the game held 60 fps, in Training and in
  matches.

Both ran build `c25-b17-31994bc-dirty-c702bd88435e27c6` (NRO SHA-256
`20e9e062c9ace6230b3fadda6fb9a1ffdb76659479f3b32de0f1100e9e72a494`), built with
the `diagnostic-pane-userdata` feature (an observer-only probe that writes
`sd:/smash-gamepad-overlay-probe.log`). Its source is the v0.1.0 source apart
from handoff notes: commit `1bae4ee` in the history of that day, `b90b711`
("Fix cached HUD pane lifetimes on Smash 13.0.5") in the published history,
whose commit IDs changed in a later rewrite. Release `v0.1.0` is the same
source built with default features (probe off), so it is not byte-identical to
the tested binary. Both tests had Training Modpack loaded, so they exercised
the HUD (non-draw) path. Source: the GitHub `v0.1.0` release notes; the build
ID also appears in the probe log on the card
(`smash-training-lobby/artifacts/phase23/overlay-investigation/card-20261105T131402Z/overlay/smash-gamepad-overlay-probe.log`).

## Releases

All on Forgejo (`v0.1.1` to `v0.3.1`; `v0.1.0` is a tag there and a release on
the GitHub mirror only). Each release's notes and `manifest.json` hold its
validation.

- **v0.1.1** (2026-10-17): the v0.1.0 source plus a memory pass (release
  profile `opt-level = "s"`, smaller skin tables, stack buffers on the text
  path; see [TECHNICAL](TECHNICAL.md#memory)). No hook, offset, pane name or
  runtime behaviour changed; the binary was not run on a Switch or in Eden.
- **v0.1.2** (2026-10-19): log and layout-flavor strings name the Rust
  installer. Booted on the operator's Switch on 2026-10-18 (Smash 13.0.5,
  Atmosphère 1.12.1, Horizon OC 2.6.0, hekate 6.5.4, Training Modpack); the
  release notes and manifest cite the operator's report and the overlay log
  (build `c33-b35-0d25b0d`, the HUD path, the skin captured from P1 at each of
  six match starts, no errors). It is the last release with a recorded console
  test.
- **v0.1.3** (2026-10-27): practice-only placement for native Training and
  lobby waiting-room practice. Lobby detection is read-only and guarded for
  Smash 13.0.5; it does not change the game's mode or enable Training Modpack
  features. Existing patched layouts and skin configs stayed compatible.
- **v0.2.0** (2026-10-31): showed the CPU's inputs while the `ssbu-cpu-pilot`
  plugin's CPU drove P1. When that plugin's `ssbu_cpu_pilot_state_v1` export
  reported an armed CPU level, the overlay drew the CPU's last input instead of
  the physical pad (Attack as A, Special as B, Jump as X, Shield as R, Grab as
  ZR, taunts as the D-pad, and the left stick); the DebugText header read
  `CPU Lv<n>`. In Eden on 2026-10-31 (ssbu-cpu-pilot runs g01–g04) both draw
  paths followed the CPU with the pad idle and the pad again afterwards. On the
  card 2026-10-31 to 2026-11-01; no console log of it.
- **v0.3.0** (2026-11-01): replaced that bridge with the game's own final
  input, read each frame for the player on the polled controller, drawn on
  that player's layout:
  - a pressed button lights when the action it is mapped to reached the game;
  - an action no pressed button explains (a CPU pilot, input playback) lights
    the button the player's controls map to it (first of X, Y, A, B, R, ZR, L,
    ZL, then the D-pad);
  - each stick shows the physical stick when the game's stick agrees with it,
    otherwise the game's;
  - Plus, Minus, stick clicks and GC trigger travel stay as read from the pad.

  Nine 64-byte code windows are checked at boot; on a mismatch, or when no
  player uses the polled controller, the raw pad is shown. Two display rules
  were approved by the operator on 2026-11-01: a game stick smaller and in the
  same direction as the physical one counts as agreeing (plus one count for
  rounding), because the game's deadzone curve is read from a parameter file
  and is not modelled; an A+B smash keeps both buttons lit although the game
  clears Special for it.

  Eden on Windows, 2026-11-01 (ssbu-cpu-pilot runs f01, the diag build on the
  `Layout::Draw` path, and f02, the release candidate with the Training Modpack
  on the HUD path): the overlay found P1 by its controller (f01 log:
  `c4=0x0` equal to `hid_npad=0x0`); host presses lit as held and stick up lit
  no button; with the CPU pilot driving P1 and the pad idle it followed the CPU
  on P1's own layout (that save maps Jump to R and ZR); it followed the pad
  again after the hand-back. Training Modpack input playback was not tested.

  On the console beside TrainingRooms (card log read 2026-11-05 13:14Z,
  `…/card-20261105T131402Z/overlay/smash-gamepad-overlay.log`, build
  `c42-b11-d9b6b70` = the v0.3.0 release): the nine windows matched, the
  overlay took its `Layout::Draw` path (the misplacement v0.3.1 fixes), and it
  logged two one-time raw-HID fallbacks (`no player for npad 32`,
  `npad 0 style 1 disagreed with HID`). The release build does not log when
  the final input is used.
- **v0.3.1** (2026-11-05): works with TrainingRooms (`libtraining_rooms.nro`)
  as well as the Training Modpack fork. Up to v0.3.0 the plugin used its HUD
  path only when it found Training Modpack's NRO by file name; with
  TrainingRooms it switched to its `Layout::Draw` path (full size at the bottom
  right in matches, and a different place in Training). v0.3.1 always renders
  through the HUD path and no longer looks up other plugins or hooks
  `Layout::Draw`; the `DebugText` fallback and the
  `FORCE_TRAINING_MODPACK_COMPAT` flag are gone. In Eden on 2026-11-05 (Smash
  13.0.5, TrainingRooms 0.2.0-rc.2, run `ov-fix-tr-yoga`, the release
  candidate `3daa2bdd…`) the overlay sat beside P1's HUD in a 1v1 against a
  CPU and right of the CPU's HUD in Training, as with the fork; it does not
  show in waiting-room practice; `trace.unmapped_access` total 0.

The release manifests of v0.3.0 and v0.3.1 name commits `d9b6b70` and
`ca5f633`; after the history rewrite their tags point at `560c769` and
`a0dc8fc`.
