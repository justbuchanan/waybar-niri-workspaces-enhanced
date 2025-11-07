# waybar-niri-workspaces-enhanced [![Nix CI](https://github.com/justbuchanan/waybar-niri-workspaces-enhanced/actions/workflows/nix.yml/badge.svg)](https://github.com/justbuchanan/waybar-niri-workspaces-enhanced/actions/workflows/nix.yml)

Enhanced niri workspaces module for waybar with window icons.

## Installation

### Using Home Manager (Flake)

This repo provides a home manager module you can include and enable. See the [example](./home-manager-example/flake.nix).

When enabled, this will create two symlinks:

- `~/.config/waybar/niri-workspaces-enhanced.so` - the waybar module
- `~/.config/niri/rename-workspace.sh` - a script for dynamic workspace renaming

### Manual Installation

#### Using Cargo

Build with cargo:

```bash
cargo build --release
```

Then copy the library:

```bash
cp target/release/libwaybar_niri_workspaces_enhanced.so ~/.config/waybar/niri-workspaces-enhanced.so
```

## Configuration

See [waybar-config.jsonc](./waybar-config.jsonc) and [style.css](./style.css) for configuration examples. Note that this module replaces waybar's builtin niri-workspaces module.

## Rename Workspace Script

The included `rename-workspace.sh` script allows you to rename the current niri workspace using a dialog box. This can be bound to a keyboard shortcut with something like:

```kdl
binds {
    # Or using raw script
    Mod+R { spawn "~/.config/niri/rename-workspace.sh"; }
}
```

## Caveats

- I have not tested this with a multi-monitor setup and I expect it to need some changes to support multiple monitors well.
- Waybar's builtin niri module has some settings that I have not implemented (yet).
