# waybar-niri-workspaces-enhanced [![Nix CI](https://github.com/justbuchanan/waybar-niri-workspaces-enhanced/actions/workflows/nix.yml/badge.svg)](https://github.com/justbuchanan/waybar-niri-workspaces-enhanced/actions/workflows/nix.yml)

A replacement niri plugin for waybar to display icons for running programs.

![screenshot](screenshot.png)

## Installation

## Try it out (without installing)

Assuming you are running niri and already have cargo installed, this will run waybar with the niri-workspaces-ehnanced module.

```.sh
git clone https://github.com/justbuchanan/waybar-niri-workspaces-enhanced
cd waybar-niri-workspaces-enhanced
cargo build
RUST_LOG=info waybar --config ./waybar-config.jsonc
```

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

The included `rename-workspace.sh` script shows a popup window that allows you to rename the current niri workspace. This can be bound to a keyboard shortcut with something like:

```kdl
binds {
    Mod+R { spawn "~/.config/niri/rename-workspace.sh"; }
}
```

## Multi-monitor setups

By default the module shows the workspaces of all outputs. Since niri restarts
workspace indices from 1 on every output, that combined list is usually not
what you want. Set the `output` option to only show the workspaces of one
output, typically by defining one bar per output in the waybar config and
pointing each bar's module at its own output:

```jsonc
[
  {
    "output": "HDMI-A-1",
    "modules-left": ["cffi/niri-workspaces-enhanced"],
    "cffi/niri-workspaces-enhanced": { "output": "HDMI-A-1", ... }
  },
  {
    "output": "eDP-1",
    "modules-left": ["cffi/niri-workspaces-enhanced"],
    "cffi/niri-workspaces-enhanced": { "output": "eDP-1", ... }
  }
]
```

## Caveats
- Waybar's builtin niri module has some settings that I have not implemented (yet).

GitHub issues and PRs are welcome.
