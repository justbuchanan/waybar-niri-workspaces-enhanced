/// Default icon mappings for common applications. These will be merged with
/// user-provided icons, with user icons taking precedence. Note that this
/// module does case-insensitive matching of app_ids, so capitalization doesn't
/// matter.
pub const DEFAULT_ICONS: &[(&str, &str)] = &[
    ("alacritty", ""),
    ("atom", ""),
    ("banshee", ""),
    ("blender", ""),
    ("chromium", ""),
    ("com.mitchellh.ghostty", ""),
    ("cura", ""),
    ("darktable", ""),
    ("discord", ""),
    ("eclipse", ""),
    ("emacs", ""),
    ("eog", ""),
    ("evince", ""),
    ("evolution", ""),
    ("factorio", ""),
    ("feh", ""),
    ("file-roller", ""),
    ("filezilla", ""),
    ("firefox", ""),
    ("firefox-esr", ""),
    ("foot", ""),
    ("gimp", ""),
    ("gimp-2.8", ""),
    ("gnome-control-center", ""),
    ("gnome-terminal-server", ""),
    ("google-chrome", ""),
    ("google-chrome", ""),
    ("gpick", ""),
    ("imv", ""),
    ("insomnia", ""),
    ("java", ""),
    ("jetbrains-idea", ""),
    ("jetbrains-studio", ""),
    ("keepassxc", ""),
    ("keybase", ""),
    ("kicad", ""),
    ("kitty", ""),
    ("libreoffice", ""),
    ("lua5.1", ""),
    ("mpv", ""),
    ("mupdf", ""),
    ("mysql-workbench-bin", ""),
    ("nautilus", ""),
    ("nemo", ""),
    ("openscad", ""),
    ("pavucontrol", ""),
    ("postman", ""),
    ("prusa-slicer", ""),
    ("rhythmbox", ""),
    ("robo3t", ""),
    ("signal", ""),
    ("slack", ""),
    ("slic3r.pl", ""),
    ("spotify", ""),
    ("steam", ""),
    ("subl", ""),
    ("subl3", ""),
    ("sublime_text", ""),
    ("thunar", ""),
    ("thunderbird", ""),
    ("totem", ""),
    ("urxvt", ""),
    ("xfce4-terminal", ""),
    ("xournal", ""),
    ("yelp", ""),
    ("zenity", ""),
    ("zoom", ""),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_default_icon_keys_are_lowercase() {
        for (key, _) in DEFAULT_ICONS {
            assert_eq!(
                key,
                &key.to_lowercase(),
                "Default icon key '{}' must be lowercase",
                key
            );
        }
    }
}
