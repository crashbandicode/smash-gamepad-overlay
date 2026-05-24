# Project instructions

This is a Rust Skyline plugin for Super Smash Bros. Ultimate.

Use Rust only for the plugin implementation.
Do not create C or C++ source files unless explicitly asked.
Do not use a PC overlay, OBS browser source, TCP/UDP streaming, RetroSpy hardware, or external receiver.
The overlay must render inside Smash so it is captured by a capture card.

Build target:
- cargo-skyline
- Rust
- Skyline plugin loaded by Smash

External projects such as RetroSpy, Open Joystick Display, m-overlay, or C/libnx examples may be used only as references for UI behavior, controller mapping, and visual layout.
Translate concepts into Rust instead of copying C/C++ architecture.

First milestone:
- Poll P1 raw controller state.
- Render a simple in-game text display.
- Show buttons, left stick, right stick, and analog trigger values if available.

Prefer simple, testable steps over a large polished implementation.
