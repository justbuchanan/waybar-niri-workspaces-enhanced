{
  description = "Enhanced niri workspaces module for waybar with window icons";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    {
      # System-agnostic outputs
      homeModules.default = { config, lib, pkgs, ... }: {
        options.programs.waybar.niri-workspaces-enhanced = with lib; {
          enable = mkEnableOption "niri-workspaces-enhanced waybar module";
        };

        config = lib.mkIf config.programs.waybar.niri-workspaces-enhanced.enable {
          home.file.".config/waybar/niri-workspaces-enhanced.so" = {
            source = "${self.packages.${pkgs.system}.default}/lib/libwaybar_niri_workspaces_enhanced.so";
          };

          home.file.".config/niri/rename-workspace.sh" = {
            source = "${self.packages.${pkgs.system}.rename-workspace}/bin/niri-rename-workspace";
            executable = true;
          };
        };
      };
    } // flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.beta.latest.default;
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "waybar-niri-workspaces-enhanced";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "waybar-cffi-0.1.1" = "sha256-Ah3pJKUBnLqLIGcSUNP8SLfpI0JHwvA8EUM3YvZPIx4=";
            };
          };

          nativeBuildInputs = [ pkgs.pkg-config ];

          buildInputs = with pkgs; [
            glib
            gtk3
            pango
            cairo
            gdk-pixbuf
          ];

          meta = with pkgs.lib; {
            description = "Enhanced niri workspaces module for waybar";
            homepage = "https://github.com/justbuchanan/waybar-niri-workspaces-enhanced";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.linux;
          };
        };

        packages.rename-workspace = pkgs.writeShellApplication {
          name = "niri-rename-workspace";
          runtimeInputs = with pkgs; [ zenity jq niri ];
          text = builtins.readFile ./rename-workspace.sh;
        };

        devShells.default = with pkgs; mkShell {
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [
            # TODO: deduplicate with the default package
            glib
            gtk3
            pango
            cairo
            gdk-pixbuf

            rustToolchain
            rustfmt
            taplo
            nixpkgs-fmt
            nodePackages.prettier
            treefmt

            # For rename-workspace.sh script
            zenity
            jq
            niri
          ];
        };
      }
    );
}
