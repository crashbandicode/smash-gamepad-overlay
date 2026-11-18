# Changelog

## v0.4.0 (2026-11-18)

- **Install just the NRO.** No `layout.arc` is needed any more. When the game loads its match HUD layout
  (`info_melee`), the plugin adds its own panes to the layout ARCropolis hands it, in place, through ARCropolis's
  file callback. A HUD mod that changes the same layout now works alongside the overlay instead of replacing it.
- **The RetroSpy skins are built in too.** `switch_pro_alt_builtin` and `gamecube_tron_builtin` work with just the
  NRO: the plugin adds their panes, materials and textures the same way. With the same `config.json`, the overlay
  looks as it did with the installer's patched layout.
- If another mod has reshaped the HUD's texture archive or layouts so the skins cannot be added, the default panes
  still go in and the log says why (`skins=0 … skin_error=…`).
- An old patched `layout.arc` that is still installed keeps working: the plugin finds its panes already there and
  leaves the file alone.
- If the merge cannot run (no ARCropolis, or a layout it does not recognise), the game gets its layout unchanged
  and the overlay stays off; the log says why (`layout merge: …`).
- The plugin is larger: 444 KiB (448 KiB of memory, was 104 KiB), mostly the compressed skin textures (288 KiB).
  While the HUD layout loads, ARCropolis's buffer for it is about 5 MB, as with the installer's patched layout.
- Release builds embed the skins from `tools/skin_blob.py`'s output (`SGPO_SKIN_BLOB`); a build without it has the
  default skins only.
- Tested in Eden only; not yet tested on a Switch. Details: `docs/NRO-ONLY-DESIGN.md`.

Earlier releases: [docs/STATUS.md](docs/STATUS.md).
