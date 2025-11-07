// General structure of the module was borrowed from https://github.com/LawnGnome/niri-taskbar/blob/main/src/lib.rs

mod default_icons;

use niri_ipc::socket::Socket;
use niri_ipc::{Event, Request, Response, Window};
use serde::Deserialize;
use std::collections::HashMap;
use waybar_cffi::{
    gtk::{
        self,
        glib::MainContext,
        prelude::{ButtonExt, LabelExt},
        traits::{ContainerExt, StyleContextExt, WidgetExt},
        Button, Label, Orientation,
    },
    waybar_module, InitInfo, Module,
};

const DEFAULT_FORMAT: &str = "{icon}";
const DEFAULT_FOCUSED_FORMAT: &str = "{icon}";
const DEFAULT_URGENT_FORMAT: &str = "{icon}";

fn get_raw_icon(cfg: &Config, window: &Window) -> String {
    let Some(app_id) = &window.app_id else {
        log::warn!("Window doesn't have an app_id: {:?}", window);
        return cfg.window_icon_default.clone();
    };

    let app_id_lower = app_id.to_lowercase();

    cfg.window_icons
        .get(&app_id_lower)
        .cloned()
        .unwrap_or_else(|| {
            log::warn!("No icon configured for app_id='{}'", app_id);
            cfg.window_icon_default.clone()
        })
}

fn format_icon(cfg: &Config, icon: &str, is_focused: bool, is_urgent: bool) -> String {
    let format = if is_urgent {
        &cfg.window_icon_formats.urgent
    } else if is_focused {
        &cfg.window_icon_formats.focused
    } else {
        &cfg.window_icon_formats.default
    };

    format.replace("{icon}", icon)
}

fn format_workspace_label(info: &WorkspaceInfo) -> String {
    let mut markup = info.idx.to_string();

    if !info.name.is_empty() {
        markup.push(' ');
        markup.push_str(&info.name);
    }

    if !info.icons.is_empty() {
        markup.push_str(": ");
        markup.push_str(&info.icons);
    }

    markup
}

struct NiriWorkspacesEnhanced;

impl Module for NiriWorkspacesEnhanced {
    type Config = UserConfig;

    fn init(info: &InitInfo, user_config: UserConfig) -> Self {
        env_logger::init();

        // Convert UserConfig to Config
        let config = Config::from_user(&user_config);

        // Set up the box that we'll use to contain the actual window buttons.
        let root = info.get_root_widget();
        let container = gtk::Box::new(Orientation::Horizontal, 0);
        container.set_widget_name("workspaces");
        root.add(&container);

        // Create an async channel for sending workspace updates from the background thread
        let (tx, rx) = async_channel::unbounded();

        // Spawn a background thread for blocking I/O
        std::thread::spawn(move || {
            if let Err(err) = background_task(config, tx) {
                log::error!("Background task error: {}", err);
            }
        });

        // Spawn async task on the main context to receive updates
        let context = MainContext::default();
        context.spawn_local(async move {
            while let Ok(mut ws_info) = rx.recv().await {
                // Sort by workspace index (ascending order)
                ws_info.sort_by_key(|info| info.idx);

                // Clear existing buttons
                for child in container.children() {
                    container.remove(&child);
                }

                // Add new buttons in sorted order
                for info in ws_info {
                    let label = Label::new(None);
                    label.set_markup(&format_workspace_label(&info));

                    let button = Button::new();
                    button.add(&label);

                    // Apply CSS classes based on workspace state
                    let style_context = button.style_context();
                    let classes = [
                        ("focused", info.is_focused),
                        ("urgent", info.is_urgent),
                        ("active", info.is_active),
                    ];
                    for (class, should_add) in classes {
                        if should_add {
                            style_context.add_class(class);
                        } else {
                            style_context.remove_class(class);
                        }
                    }

                    // Connect click handler to switch to workspace
                    let workspace_id = info.id;
                    button.connect_clicked(move |_| {
                        // Spawn a blocking task to send the workspace switch command
                        std::thread::spawn(move || {
                            // TODO: use existing socket instead of making a new one here
                            if let Ok(mut socket) = Socket::connect() {
                                let request = Request::Action(niri_ipc::Action::FocusWorkspace {
                                    reference: niri_ipc::WorkspaceReferenceArg::Id(workspace_id),
                                });
                                if let Err(err) = socket.send(request) {
                                    log::error!("Failed to switch workspace: {}", err);
                                }
                            } else {
                                log::error!("Failed to connect to niri socket");
                            }
                        });
                    });

                    container.add(&button);
                }

                container.show_all();
            }
        });

        Self {}
    }
}

waybar_module!(NiriWorkspacesEnhanced);

#[derive(Debug, Clone)]
struct WorkspaceInfo {
    id: u64,
    name: String,
    icons: String,
    idx: u8,
    is_focused: bool,
    is_urgent: bool,
    is_active: bool,
}

fn background_task(
    config: Config,
    tx: async_channel::Sender<Vec<WorkspaceInfo>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd_socket = Socket::connect()?;
    let mut subscribe_socket = Socket::connect()?;

    // Initial update
    update_workspaces(&config, &tx, &mut cmd_socket)?;

    let Ok(Response::Handled) = subscribe_socket.send(Request::EventStream)? else {
        return Err("Expected Handled response".into());
    };

    let mut read_event = subscribe_socket.read_events();
    while let Ok(event) = read_event() {
        if matches!(
            event,
            Event::WindowOpenedOrChanged { .. }
                | Event::WindowClosed { .. }
                | Event::WindowLayoutsChanged { .. }
                | Event::WindowFocusChanged { .. }
                | Event::WorkspacesChanged { .. }
        ) {
            update_workspaces(&config, &tx, &mut cmd_socket)?;
        }
    }

    Ok(())
}

fn update_workspaces(
    config: &Config,
    tx: &async_channel::Sender<Vec<WorkspaceInfo>>,
    cmd_socket: &mut Socket,
) -> Result<(), Box<dyn std::error::Error>> {
    let Response::Workspaces(workspaces) = cmd_socket.send(Request::Workspaces)?? else {
        return Err("Expected Workspaces response".into());
    };

    // Store workspace info using WorkspaceInfo struct
    let mut ws_info: HashMap<u64, WorkspaceInfo> = workspaces
        .iter()
        .map(|ws| {
            (
                ws.id,
                WorkspaceInfo {
                    id: ws.id,
                    name: ws.name.clone().unwrap_or_default(),
                    icons: String::new(),
                    idx: ws.idx,
                    is_focused: ws.is_focused,
                    is_urgent: ws.is_urgent,
                    is_active: ws.is_active,
                },
            )
        })
        .collect();

    let Response::Windows(mut windows) = cmd_socket.send(Request::Windows)?? else {
        return Err("Expected Windows response".into());
    };

    // Sort windows by their position in the scrolling layout
    windows.sort_by_key(|w| w.layout.pos_in_scrolling_layout);

    // Collect icons and track if workspace has urgent windows
    for (workspace_id, window) in windows
        .iter()
        .filter_map(|w| w.workspace_id.map(|id| (id, w)))
    {
        let raw_icon = get_raw_icon(config, window);
        let formatted_icon = format_icon(config, &raw_icon, window.is_focused, window.is_urgent);

        if let Some(ws) = ws_info.get_mut(&workspace_id) {
            if !ws.icons.is_empty() {
                ws.icons.push(' ');
            }
            ws.icons.push_str(&formatted_icon);
        }
    }

    // Convert to Vec for sending
    let ws_vec: Vec<WorkspaceInfo> = ws_info.into_values().collect();

    // Send to main thread (using blocking send since we're in a blocking thread)
    tx.send_blocking(ws_vec)
        .map_err(|_| "Failed to send workspace info")?;

    Ok(())
}

#[derive(Deserialize, Debug, Clone)]
struct UserWindowIconFormats {
    focused: Option<String>,
    urgent: Option<String>,
    default: Option<String>,
}
#[derive(Deserialize, Debug, Clone)]
struct UserConfig {
    #[serde(default, rename = "window-icons")]
    window_icons: Option<HashMap<String, String>>,
    #[serde(default, rename = "window-icon-default")]
    window_icon_default: Option<String>,
    #[serde(default, rename = "window-icon-format")]
    window_icon_formats: Option<UserWindowIconFormats>,
}

#[derive(Debug, Clone)]
struct WindowIconFormats {
    focused: String,
    urgent: String,
    default: String,
}

impl WindowIconFormats {
    fn from_user(user_formats: &UserWindowIconFormats) -> Self {
        // Validate and warn on bad inputs
        let formats = [
            ("focused", &user_formats.focused),
            ("urgent", &user_formats.urgent),
            ("default", &user_formats.default),
        ];
        for (name, format_opt) in formats {
            if let Some(format) = format_opt {
                if !format.contains("{icon}") {
                    log::warn!(
                        "window-icon-format.{} must contain the substring \"{{{{icon}}}}\", using default",
                        name
                    );
                }
            }
        }

        Self {
            focused: user_formats
                .focused
                .clone()
                .unwrap_or_else(|| DEFAULT_FOCUSED_FORMAT.to_string()),
            urgent: user_formats
                .urgent
                .clone()
                .unwrap_or_else(|| DEFAULT_URGENT_FORMAT.to_string()),
            default: user_formats
                .default
                .clone()
                .unwrap_or_else(|| DEFAULT_FORMAT.to_string()),
        }
    }
}

// TODO: can active vs urgent styling be done with css instead of a config option?
#[derive(Debug, Clone)]
struct Config {
    window_icon_default: String,
    window_icon_formats: WindowIconFormats,
    /// Merged icons: default icons + user-provided icons (user icons take precedence)
    window_icons: HashMap<String, String>,
}

impl Config {
    pub fn from_user(uc: &UserConfig) -> Self {
        // Start with default icons
        let mut window_icons: HashMap<String, String> = default_icons::DEFAULT_ICONS
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        // Merge user-provided icons, overwriting defaults
        if let Some(ref user_icons) = uc.window_icons {
            window_icons.extend(user_icons.clone());
        }

        Self {
            window_icon_default: uc.window_icon_default.clone().unwrap_or_default(),
            window_icon_formats: uc
                .window_icon_formats
                .as_ref()
                .map(|f| WindowIconFormats::from_user(f))
                .unwrap_or_else(|| WindowIconFormats {
                    focused: DEFAULT_FOCUSED_FORMAT.to_string(),
                    urgent: DEFAULT_URGENT_FORMAT.to_string(),
                    default: DEFAULT_FORMAT.to_string(),
                }),
            window_icons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_window(app_id: Option<String>) -> Window {
        Window {
            id: 1,
            title: Some("Test".to_string()),
            app_id,
            pid: None,
            workspace_id: Some(1),
            is_focused: false,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
        }
    }

    #[test]
    fn test_format_workspace_label_basic() {
        let info = WorkspaceInfo {
            id: 1,
            name: String::new(),
            icons: String::new(),
            idx: 1,
            is_focused: false,
            is_urgent: false,
            is_active: false,
        };
        assert_eq!(format_workspace_label(&info), "1");
    }

    #[test]
    fn test_format_workspace_label_with_name() {
        let info = WorkspaceInfo {
            id: 1,
            name: "Work".to_string(),
            icons: String::new(),
            idx: 2,
            is_focused: false,
            is_urgent: false,
            is_active: false,
        };
        assert_eq!(format_workspace_label(&info), "2 Work");
    }

    #[test]
    fn test_format_workspace_label_with_icons() {
        let info = WorkspaceInfo {
            id: 1,
            name: String::new(),
            icons: "🔥 💻".to_string(),
            idx: 3,
            is_focused: false,
            is_urgent: false,
            is_active: false,
        };
        assert_eq!(format_workspace_label(&info), "3: 🔥 💻");
    }

    #[test]
    fn test_format_workspace_label_with_name_and_icons() {
        let info = WorkspaceInfo {
            id: 1,
            name: "Dev".to_string(),
            icons: "🚀".to_string(),
            idx: 4,
            is_focused: false,
            is_urgent: false,
            is_active: false,
        };
        assert_eq!(format_workspace_label(&info), "4 Dev: 🚀");
    }

    #[test]
    fn test_format_icon_default() {
        let config = Config {
            window_icon_default: String::new(),
            window_icon_formats: WindowIconFormats {
                focused: "{icon}".to_string(),
                urgent: "{icon}".to_string(),
                default: "{icon}".to_string(),
            },
            window_icons: HashMap::new(),
        };
        let result = format_icon(&config, "🔥", false, false);
        assert_eq!(result, "🔥");
    }

    #[test]
    fn test_format_icon_focused() {
        let formats = WindowIconFormats {
            focused: "[{icon}]".to_string(),
            urgent: "{icon}".to_string(),
            default: "{icon}".to_string(),
        };
        let config = Config {
            window_icon_default: String::new(),
            window_icon_formats: formats,
            window_icons: HashMap::new(),
        };
        let result = format_icon(&config, "🔥", true, false);
        assert_eq!(result, "[🔥]");
    }

    #[test]
    fn test_format_icon_urgent_takes_precedence() {
        let formats = WindowIconFormats {
            focused: "[{icon}]".to_string(),
            urgent: "!{icon}!".to_string(),
            default: "{icon}".to_string(),
        };
        let config = Config {
            window_icon_default: String::new(),
            window_icon_formats: formats,
            window_icons: HashMap::new(),
        };
        let result = format_icon(&config, "🔥", true, true);
        assert_eq!(result, "!🔥!");
    }

    #[test]
    fn test_get_raw_icon_with_mapping() {
        let mut window_icons = HashMap::new();
        window_icons.insert("firefox".to_string(), "🦊".to_string());
        window_icons.insert("code".to_string(), "💻".to_string());

        let config = Config {
            window_icon_default: "❓".to_string(),
            window_icon_formats: WindowIconFormats {
                focused: "{icon}".to_string(),
                urgent: "{icon}".to_string(),
                default: "{icon}".to_string(),
            },
            window_icons,
        };

        let window = create_test_window(Some("Firefox".to_string()));
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "🦊");
    }

    #[test]
    fn test_get_raw_icon_no_app_id() {
        let config = Config {
            window_icon_default: "❓".to_string(),
            window_icon_formats: WindowIconFormats {
                focused: "{icon}".to_string(),
                urgent: "{icon}".to_string(),
                default: "{icon}".to_string(),
            },
            window_icons: HashMap::new(),
        };

        let window = create_test_window(None);
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "❓");
    }

    #[test]
    fn test_get_raw_icon_case_insensitive() {
        let mut window_icons = HashMap::new();
        window_icons.insert("firefox".to_string(), "🦊".to_string());

        let config = Config {
            window_icon_default: String::new(),
            window_icon_formats: WindowIconFormats {
                focused: "{icon}".to_string(),
                urgent: "{icon}".to_string(),
                default: "{icon}".to_string(),
            },
            window_icons,
        };

        let window = create_test_window(Some("FIREFOX".to_string()));
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "🦊");
    }

    #[test]
    fn test_from_user_includes_defaults() {
        let user_config = UserConfig {
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: None,
        };

        let config = Config::from_user(&user_config);

        // Check that default icons are present
        assert!(config.window_icons.contains_key("google-chrome"));
    }

    #[test]
    fn test_from_user_overrides_defaults() {
        let mut user_icons = HashMap::new();
        user_icons.insert("google-chrome".to_string(), "🔥".to_string());

        let user_config = UserConfig {
            window_icons: Some(user_icons),
            window_icon_default: None,
            window_icon_formats: None,
        };

        let config = Config::from_user(&user_config);

        // User icon should override default
        assert_eq!(
            config.window_icons.get("google-chrome"),
            Some(&"🔥".to_string())
        );
    }

    #[test]
    fn test_from_user_adds_new_icons() {
        let mut user_icons = HashMap::new();
        user_icons.insert("custom-app".to_string(), "🎯".to_string());

        let user_config = UserConfig {
            window_icons: Some(user_icons),
            window_icon_default: None,
            window_icon_formats: None,
        };

        let config = Config::from_user(&user_config);

        // User custom icon should be present
        assert_eq!(
            config.window_icons.get("custom-app"),
            Some(&"🎯".to_string())
        );

        // Default icons should still be present
        assert!(config.window_icons.contains_key("google-chrome"));
    }

    #[test]
    fn test_from_user_with_defaults() {
        let user_config = UserConfig {
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: None,
        };

        let config = Config::from_user(&user_config);

        // Should have default icons
        assert!(config.window_icons.contains_key("firefox"));
        assert!(config.window_icons.contains_key("google-chrome"));
        assert!(config.window_icons.contains_key("alacritty"));

        // Should have default formats
        let formats = &config.window_icon_formats;
        assert_eq!(&formats.default, DEFAULT_FORMAT);
        assert_eq!(&formats.focused, DEFAULT_FOCUSED_FORMAT);
        assert_eq!(&formats.urgent, DEFAULT_URGENT_FORMAT);
    }
}
