# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GUI Scale Applet (aka cosmic-tailscale-applet) is a COSMIC Desktop panel applet for managing Tailscale VPN. It uses `libcosmic` (iced-based) and shells out to the `tailscale` CLI for all operations. Requires Tailscale installed with operator mode set (`sudo tailscale set --operator=$USER`).

**App ID:** `com.github.bhh32.GUIScaleApplet`

## Build Commands

```bash
just                    # Build release (default)
just build              # Debug build
just build-release      # Release build
just check              # Clippy with --all-features -W clippy::pedantic
just run                # Run release with RUST_BACKTRACE=full
just dev                # cargo fmt + run
just clean              # cargo clean
sudo just install       # Build release + install binary, desktop entry, icon
just uninstall          # Remove installed files
```

No test suite exists. No flake.nix exists.

## Architecture

The applet follows the standard libcosmic Application pattern (Elm architecture):

- **`src/main.rs`** - Entry point, launches `cosmic::applet::run::<Window>()`
- **`src/window.rs`** - Core application struct (`Window`) implementing `cosmic::Application`. Contains all state, the `Message` enum, `update()` for message handling, and `view()`/`view_window()` for UI rendering. The popup is a Wayland popup surface with configurable size limits.
- **`src/logic.rs`** - All Tailscale CLI interactions via `std::process::Command`. Every function shells out to `tailscale` (ip, status, debug prefs, set, up/down, file cp/get, exit-node, switch). Async functions (`tailscale_send`, `tailscale_recieve`, `clear_status`) use tokio via `cosmic::task::future`.
- **`src/config.rs`** - Persistent config via `cosmic_config` (COSMIC's XDG-based config system). Stores `exit-node` index and `allow-lan` bool. Config version is 1.

## Key Design Details

- All Tailscale state is read synchronously at init time and on account switch (SSH, routes, connection, devices, exit nodes, accounts)
- TailDrop (file send/receive) runs async to avoid blocking the UI thread
- Exit node selection is mutually exclusive with the host being an exit node
- Status messages auto-clear after `STATUS_CLEAR_TIME` (5 seconds) via async delay
- i18n uses Fluent (`.ftl` files in `i18n/`), currently has en, nl, sv translations

## Dependencies

- `libcosmic` from git (pop-os/libcosmic) with features: applet, wayland, tokio, desktop
- `tokio`, `serde`, `regex`, `url`, `rust-embed`
- Rust edition 2024
