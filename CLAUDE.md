# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GUI Scale Applet (cosmic-tailscale-applet) is a COSMIC Desktop panel applet for managing Tailscale VPN. Fork of [cosmic-utils/gui-scale-applet](https://github.com/cosmic-utils/gui-scale-applet) by Bryan Hyland. Uses `libcosmic` (iced-based) and shells out to the `tailscale` CLI for all operations. Requires Tailscale installed with operator mode set (`sudo tailscale set --operator=$USER`).

**App ID:** `com.github.bhh32.GUIScaleApplet`

## Build Commands

```bash
# Using Nix (recommended on NixOS)
nix develop              # Enter dev shell with all dependencies
nix build                # Full reproducible build
nix flake check          # Validate flake, modules, packages

# Using just (any distro)
just                     # Build release (default)
just build               # Debug build
just build-release       # Release build
just check               # Clippy with --all-features -W clippy::pedantic
just run                 # Run release with RUST_BACKTRACE=full
just dev                 # cargo fmt + run
just clean               # cargo clean
sudo just install        # Build release + install binary, desktop entry, icon
just uninstall           # Remove installed files
```

No test suite exists.

**Important for NixOS:** New source files must be `git add`-ed before `nix build` (the `cleanSource` filter only includes git-tracked files). Do not use `--all-features` with clippy — the `rfd` feature in libcosmic has orphan rule issues.

## Architecture

The applet follows the standard libcosmic Application pattern (Elm architecture):

- **`src/main.rs`** - Entry point, initializes i18n, launches `cosmic::applet::run::<Window>()`
- **`src/window.rs`** - Core `Window` struct implementing `cosmic::Application`. Contains all state, the `Message` enum, `update()` for async message handling, `view()`/`view_window()` for UI rendering. Popup is a Wayland popup surface with configurable size limits. All UI strings use the `fl!()` macro for i18n.
- **`src/logic.rs`** - All Tailscale CLI interactions via `tokio::process::Command` (fully async). `TailscaleState` struct bundles all CLI queries into a single batch fetch triggered by `RefreshState`. Helper functions `get_tailscale_pref()` and `set_tailscale_flag()` eliminate duplication. Regex patterns are cached with `LazyLock`.
- **`src/config.rs`** - Persistent config via `CosmicConfigEntry` derive macro (version 2). Stores `exit_node_idx: usize` and `allow_lan: bool`. Auto-generated setters (`set_exit_node_idx`, `set_allow_lan`).
- **`src/error.rs`** - `AppError` enum using `thiserror`: `CliExec` (io::Error), `Utf8Error` (FromUtf8Error).
- **`src/i18n.rs`** - Internationalization module using `rust-embed` + `i18n-embed` + Fluent. Provides `fl!()` macro for compile-time key validation. Supports en, nl, sv.
- **`flake.nix`** - Nix flake with `buildRustPackage`, rust-overlay, `makeWrapper` for tailscale PATH injection, dev shell, NixOS module, Home Manager module.
- **`nix/nixos-module.nix`** - NixOS module: `programs.gui-scale-applet.enable` auto-enables tailscale service and firewall rules.
- **`nix/hm-module.nix`** - Home Manager module: per-user install with optional XDG autostart.

## Key Design Details

- All Tailscale CLI calls are async (`tokio::process::Command`) wrapped in `cosmic::task::future` — the UI never blocks
- `init()` is non-blocking: sets defaults, returns a `Task` that triggers `RefreshState` to batch-fetch all state
- Error handling uses `thiserror` + `tracing` (structured logging); graceful degradation when tailscale is unavailable
- TailDrop (file send/receive) runs async with status auto-clear after `STATUS_CLEAR_TIME` (5 seconds)
- Exit node selection is mutually exclusive with the host being an exit node
- Config uses `CosmicConfigEntry` derive macro (v2) with auto-generated setters for type-safe writes
- i18n uses Fluent `.ftl` files in `i18n/` — all UI strings use `fl!("key")`, no hardcoded text

## Dependencies

- `libcosmic` from git (pop-os/libcosmic) with features: applet, wayland, tokio, desktop
- `tokio` (process, time), `serde`, `regex`, `url`, `dirs`, `thiserror`, `tracing`
- `rust-embed`, `i18n-embed`, `i18n-embed-fl` (for i18n)
- Rust edition 2024

## Nix Build Dependencies

System libraries required (provided by `nix develop`): libxkbcommon, wayland, wayland-protocols, libinput, fontconfig, freetype, libGL, mesa, vulkan-loader, libx11, libxcursor, libxi, libxrandr, openssl. Use `pkgs.libx11` not `pkgs.xorg.libX11` (deprecated in nixos-unstable 2026+).
