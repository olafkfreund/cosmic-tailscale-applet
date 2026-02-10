# GUI Scale COSMIC Desktop Applet

A Tailscale VPN management applet for the [COSMIC Desktop Environment](https://github.com/pop-os/cosmic-epoch) by System76. Built with Rust and [libcosmic](https://github.com/pop-os/libcosmic).

**App ID:** `com.github.bhh32.GUIScaleApplet`

## Features

- **Connection Management** - Connect/disconnect Tailscale with a single toggle
- **Account Switching** - Switch between multiple Tailscale accounts
- **SSH Toggle** - Enable/disable Tailscale SSH
- **Route Acceptance** - Toggle accept-routes on/off
- **Exit Nodes** - Select exit nodes or make the host an exit node with LAN access control
- **Tail Drop** - Send and receive files between devices via Tail Drop
- **Multi-language** - Internationalized UI with English, Dutch, and Swedish translations
- **Non-blocking UI** - All Tailscale CLI operations run asynchronously
- **Persistent Config** - Settings stored via COSMIC's config system (CosmicConfigEntry v2)
- **NixOS Support** - Nix flake with NixOS module and Home Manager module

## Screenshots

![gui-scale-applet-panel](/screenshots/gui-scale-panel.png)
![gui-scale-applet-open](/screenshots/gui-scale-applet-open.png)

## Prerequisites

Tailscale must be installed and the operator must be set to your user:

```bash
sudo tailscale set --operator=$USER
```

This allows the applet to manage Tailscale without root privileges.

## Installation

### NixOS (Flake)

Add the flake input to your system configuration:

```nix
# flake.nix
{
  inputs = {
    gui-scale-applet.url = "github:olafkfreund/cosmic-tailscale-applet";
  };
}
```

**NixOS module** (system-wide, auto-enables tailscale and firewall):

```nix
# configuration.nix
{ inputs, ... }: {
  imports = [ inputs.gui-scale-applet.nixosModules.default ];
  programs.gui-scale-applet.enable = true;
}
```

**Home Manager module** (per-user with XDG autostart):

```nix
# home.nix
{ inputs, ... }: {
  imports = [ inputs.gui-scale-applet.homeManagerModules.default ];
  programs.gui-scale-applet = {
    enable = true;
    autostart = true; # default
  };
}
```

**Build only:**

```bash
nix build github:olafkfreund/cosmic-tailscale-applet
```

### Fedora / Fedora-based

```bash
sudo dnf copr enable bhh32/gui-scale-applet
sudo dnf update --refresh
sudo dnf install -y gui-scale-applet
```

### Debian / Ubuntu / Pop!_OS

Download the `.deb` package from the [releases](https://github.com/cosmic-utils/gui-scale-applet/releases) page.

### From Source

```bash
git clone https://github.com/cosmic-utils/gui-scale-applet.git
cd gui-scale-applet
sudo just install
```

## Development

### Using Nix (recommended)

```bash
nix develop          # Enter dev shell with all dependencies
just                 # Build release
just run             # Run with RUST_BACKTRACE=full
just check           # Clippy with pedantic warnings
```

### Manual Setup

Requires Rust (edition 2024), pkg-config, and system libraries: libxkbcommon, wayland, libinput, udev.

```bash
just                 # Build release (default)
just build           # Debug build
just build-release   # Release build
just check           # Clippy
just run             # Run release
just dev             # cargo fmt + run
just clean           # cargo clean
sudo just install    # Install binary, desktop entry, icon
just uninstall       # Remove installed files
```

## Architecture

The applet follows the standard libcosmic Application pattern (Elm architecture):

```
src/
  main.rs      - Entry point, i18n init, launches applet
  window.rs    - Window struct (state), Message enum, update(), view()
  logic.rs     - Async Tailscale CLI interactions (tokio::process::Command)
  config.rs    - Persistent config via CosmicConfigEntry derive macro
  error.rs     - AppError enum with thiserror
  i18n.rs      - Internationalization (rust-embed + fluent)

i18n/
  en/          - English translations
  nl/          - Dutch translations
  sv/          - Swedish translations

nix/
  nixos-module.nix  - NixOS system module
  hm-module.nix     - Home Manager module

data/
  *.desktop    - XDG desktop entry
  *.metainfo   - AppStream metadata
  icons/       - Tailscale icon
```

### Key Design Decisions

- **Async CLI** - All `tailscale` CLI calls use `tokio::process::Command` wrapped in `cosmic::task::future`, keeping the UI responsive
- **Batch State Fetch** - `TailscaleState` struct bundles all CLI queries into a single async operation triggered by `RefreshState`
- **Error Handling** - `thiserror`-based `AppError` with `tracing` for structured logging; graceful degradation when tailscale is unavailable
- **Config** - `CosmicConfigEntry` derive macro (v2) with auto-generated setters for type-safe persistent storage
- **i18n** - `i18n-embed` + `rust-embed` + Fluent `.ftl` files with the `fl!()` macro for compile-time key validation

## Translations

Translation files are in `i18n/<lang>/gui_scale_applet.ftl` using [Fluent](https://projectfluent.org/) syntax. To add a new language:

1. Create `i18n/<lang-code>/gui_scale_applet.ftl`
2. Copy keys from `i18n/en/gui_scale_applet.ftl` and translate the values
3. The language will be auto-detected from the system locale

## License

GPL-3.0
