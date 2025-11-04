## efficiency

right now all workspace "buttons" are deleted and recreated every update. we could instead keep track of existing buttons and only make changes when needed. this is more complicated and more error-prone, so maybe the way we're doing it already is ok.

it also seems that at waybar launch, update_workspaces gets called like 10 times. can we make that not happen?

## config validation

be better about letting the user know if their config is messed up

## flexibility

i got rid of the toplevel "format": "{value}: {icons}" config. maybe bring it back?

## more testing

## compatibility with existing niri module

existing spec is at https://man.archlinux.org/man/extra/waybar/waybar-niri-workspaces.5.en

being fully compatible would mean that existing configs would "just work", which would be nice. do we really want to support all that stuff though?

add info to the readme about compatibility. be clear about what we do and don't support. include a link to the existing waybar-niri-workspaces docs.

## docs

show how to launch waybar with the included example config and style.css.

add comments to example config and style.css to explain things.

in the past, I've found css discoverability really difficult for waybar. make sure we have an example for everything you can do.

is it possible to test that the css works in a ci environment?

## other

make treefmt format the style.css

in cargo.toml, pin the waybar-cffi-rs dep to a specific commit so we don't have to have an outputHashes entry in flake.nix for it.
