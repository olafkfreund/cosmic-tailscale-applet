{
  description = "COSMIC Tailscale Applet - Tailscale VPN manager for COSMIC Desktop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Runtime dependencies
        runtimeDeps = with pkgs; [
          tailscale
        ];

        # Build dependencies for libcosmic/wayland
        buildInputs = with pkgs; [
          openssl
          libxkbcommon
          wayland
          fontconfig
          freetype
          libGL
          wayland-protocols
          libinput
          mesa
          vulkan-loader
          libx11
          libxcursor
          libxi
          libxrandr
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
          makeWrapper
          cmake
        ];

        # Library paths for runtime
        libPath = pkgs.lib.makeLibraryPath buildInputs;

      in {
        packages = {
          default = self.packages.${system}.gui-scale-applet;

          gui-scale-applet = pkgs.rustPlatform.buildRustPackage {
            pname = "gui-scale-applet";
            version = "3.0.0";

            src = pkgs.lib.cleanSource ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            inherit nativeBuildInputs buildInputs;

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";

            postInstall = ''
              # Install desktop file
              install -Dm644 data/com.github.bhh32.GUIScaleApplet.desktop \
                $out/share/applications/com.github.bhh32.GUIScaleApplet.desktop

              # Install metainfo
              install -Dm644 data/com.github.bhh32.GUIScaleApplet.metainfo.xml \
                $out/share/metainfo/com.github.bhh32.GUIScaleApplet.metainfo.xml

              # Install icon
              install -Dm644 data/icons/scalable/apps/tailscale-icon.png \
                $out/share/icons/hicolor/scalable/status/tailscale-icon.png

              # Wrap binary with tailscale in PATH and runtime libraries
              wrapProgram $out/bin/gui-scale-applet \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
                --prefix LD_LIBRARY_PATH : ${libPath}
            '';

            meta = with pkgs.lib; {
              description = "Tailscale VPN management applet for COSMIC Desktop";
              homepage = "https://github.com/bhh32/GUIScaleApplet";
              license = licenses.gpl3Only;
              maintainers = [ ];
              platforms = platforms.linux;
              mainProgram = "gui-scale-applet";
            };
          };
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;

          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            clippy
            rustfmt
            just

            # Nix tools
            nixd
            statix
            deadnix
          ]);

          packages = runtimeDeps;

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          LD_LIBRARY_PATH = libPath;

          shellHook = ''
            echo "COSMIC Tailscale Applet development environment"
            echo "Run 'just' to build, 'just run' to test"
          '';
        };
      }
    ) // {
      # NixOS module
      nixosModules = {
        default = self.nixosModules.gui-scale-applet;
        gui-scale-applet = import ./nix/nixos-module.nix self;
      };

      # Home-manager module
      homeManagerModules = {
        default = self.homeManagerModules.gui-scale-applet;
        gui-scale-applet = import ./nix/hm-module.nix self;
      };
    };
}
