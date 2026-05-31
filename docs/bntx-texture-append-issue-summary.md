# BNTX Texture Append Issue Summary

Status: resolved.

The in-game HUD texture corruption seen during early PNG-backed skin testing was
traced to Toolbox-Cli rebuilding the BNTX `_DIC` dictionary in string-pool order
instead of BRTI texture order. That made existing Smash texture names resolve to
the wrong texture indices after appending SGPO textures.

Toolbox-Cli fixed the bug by rebuilding `_DIC` from the texture list order. The
current SGPO Rust installer uses `nx-layout-toolbox` from the local checkout at
`local-checkouts/Toolbox-Cli`, which includes that fix.

Keep `docs/bntx-texture-append-toolbox-cli-findings.md` as the detailed
investigation log. This short file exists so future agents do not treat the old
BNTX append corruption as an active issue.
