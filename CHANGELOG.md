# Changelog

## v0.4.0 (2026-11-17)

- **Install just the NRO.** No `layout.arc` is needed any more. When the game loads its match HUD layout
  (`info_melee`), the plugin adds its own panes to the layout ARCropolis hands it, in place, through ARCropolis's
  file callback. A HUD mod that changes the same layout now works alongside the overlay instead of replacing it.
- An old patched `layout.arc` that is still installed keeps working: the plugin finds its panes already there and
  leaves the file alone.
- If the merge cannot run (no ARCropolis, or a layout it does not recognise), the game gets its layout unchanged
  and the overlay stays off; the log says why (`layout merge: …`).
- The two RetroSpy skins (`switch_pro_alt_builtin`, `gamecube_tron_builtin`) still need the installer's patched
  layout: their textures are not merged yet.
- Costs 16 KiB more memory for the plugin (120 KiB resident).
- Tested in Eden only; not yet tested on a Switch. Details: `docs/NRO-ONLY-DESIGN.md`.

Earlier releases: [docs/STATUS.md](docs/STATUS.md).
