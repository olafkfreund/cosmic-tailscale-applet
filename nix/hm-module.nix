# Home Manager module for cosmic-tailscale-applet
flake:
{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.gui-scale-applet;

  defaultPackage = flake.packages.${pkgs.stdenv.hostPlatform.system}.gui-scale-applet;

in {
  options.programs.gui-scale-applet = {
    enable = mkEnableOption "COSMIC Tailscale Applet - Tailscale VPN manager for COSMIC Desktop panel";

    package = mkOption {
      type = types.package;
      default = defaultPackage;
      defaultText = literalExpression "flake.packages.\${pkgs.stdenv.hostPlatform.system}.gui-scale-applet";
      description = "The gui-scale-applet package to use.";
    };

    autostart = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Whether to automatically start the applet with COSMIC Desktop.
        When enabled, adds the applet to XDG autostart.
      '';
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

    # Add autostart entry
    xdg.configFile = mkIf cfg.autostart {
      "autostart/com.github.bhh32.GUIScaleApplet.desktop".source =
        "${cfg.package}/share/applications/com.github.bhh32.GUIScaleApplet.desktop";
    };
  };

  meta.maintainers = with lib.maintainers; [ ];
}
