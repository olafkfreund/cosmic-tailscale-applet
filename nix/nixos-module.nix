# NixOS module for cosmic-tailscale-applet
flake:
{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.gui-scale-applet;

  defaultPackage = flake.packages.${pkgs.stdenv.hostPlatform.system}.gui-scale-applet;

in {
  options.programs.gui-scale-applet = {
    enable = mkEnableOption "COSMIC Tailscale Applet - Tailscale VPN manager for COSMIC Desktop panel";

    package = mkPackageOption pkgs "gui-scale-applet" {
      default = defaultPackage;
      description = "The gui-scale-applet package to use.";
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # Ensure tailscale service is enabled
    services.tailscale.enable = mkDefault true;

    # Open firewall for tailscale
    networking.firewall = {
      trustedInterfaces = mkDefault [ "tailscale0" ];
      allowedUDPPorts = mkDefault [ config.services.tailscale.port ];
    };

    warnings = optional (!(config.services.desktopManager.cosmic.enable or false)
      && !(config.services.xserver.desktopManager.cosmic.enable or false))
      "gui-scale-applet is designed for COSMIC Desktop. Consider enabling services.desktopManager.cosmic.";
  };

  meta.maintainers = with lib.maintainers; [ ];
}
