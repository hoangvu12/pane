# Pane

A small, extensible desktop launcher for Windows, macOS and Linux, inspired by Raycast and Pi.

Pane is in early development. There is no installable application yet; the first slice is a native window running one bundled Rust sample command. The intended UI uses GPUI CE, with JavaScript, TypeScript and Rust extensions through a WASI 0.3 component interface. Runtime/backend choices still require validation.

## Build, run and test

The same commands work on Windows, macOS and Linux.

Prerequisites:

- [rustup](https://rustup.rs). Run `rustup toolchain install` once in the repository to install the toolchain pinned in `rust-toolchain.toml` (Rust 1.98.1 with the `wasm32-wasip2` target, rustfmt and clippy).
- **Windows:** Visual Studio Build Tools with the "Desktop development with C++" workload (MSVC and a Windows SDK).
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Linux (Debian/Ubuntu names):** `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxcb1-dev libfontconfig1-dev libfreetype-dev libvulkan1` and a Vulkan driver (`mesa-vulkan-drivers` provides a software fallback).

Commands, from the repository root:

```sh
cargo xtask guests   # build the extension guests into target/guests/
cargo run -p pane    # open the launcher window
cargo xtask ci       # build guests, then check formatting, lints and tests
```

In the window, use the arrow keys to select, Enter to open a command or run an item, and Escape to go back; clicking a row runs it too.

Layout:

- `wit/extension.wit`: the host/guest contract for one extension command.
- `crates/pane-core`: the launcher model (the public host interface the tests drive) and the extension runtime, a Wasmtime 49.0.1 engine registering only WASI 0.3.
- `crates/pane`: the GPUI CE window.
- `guests/`: extension guests, including [the Rust sample command](guests/README.md) and test fixtures.
- `scripts/smoke-*`: native GUI smoke runs used by CI, which uploads their screenshots.

## Project documents

- [Specification](https://github.com/hoangvu12/pane/issues/1)
- [Implementation tickets](https://github.com/hoangvu12/pane/issues?q=is%3Aissue+label%3Aimplementation)
- [Cross-platform contributor requirements](https://github.com/hoangvu12/pane/issues/1#cross-platform-contributor-requirement)
- [Current decisions](docs/current-decisions.md)
- [Handoff and evidence limits](docs/HANDOFF.md)
- [Domain vocabulary](CONTEXT.md)

Contributors on all three operating systems must have working build/run/test workflows early in development. Existing prototype evidence covers Windows only; macOS and Linux support remains to be validated. Research scripts may depend on the temporary toolchains and local paths documented alongside them.

The 52 implementation issues are published; work starts when each issue's blockers are complete. Research results and prototype source are under `docs/research/`; they are not production support guarantees.

The provisional licensing direction is GPL-3.0-or-later for the application, with the exact component/SDK split still unresolved. A project license has not yet been applied. Existing third-party notices and licenses remain applicable.
