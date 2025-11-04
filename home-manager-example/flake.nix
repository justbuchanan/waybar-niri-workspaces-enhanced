{
  description = "Test home-manager configuration for waybar-niri-workspaces-enhanced";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    waybar-niri-workspaces-enhanced = {
      url = "path:..";
    };
  };

  outputs = { self, nixpkgs, home-manager, waybar-niri-workspaces-enhanced }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      homeConfigurations."testuser" = home-manager.lib.homeManagerConfiguration {
        inherit pkgs;

        modules = [
          waybar-niri-workspaces-enhanced.homeModules.default
          {
            home.username = "testuser";
            home.homeDirectory = "/home/testuser";
            home.stateVersion = "24.05";

            # Enable the waybar niri workspaces enhanced module
            programs.waybar.niri-workspaces-enhanced.enable = true;
          }
        ];
      };
    };
}
