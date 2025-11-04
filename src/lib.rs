// General structure of the module was borrowed from https://github.com/LawnGnome/niri-taskbar/blob/main/src/lib.rs

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
        return cfg.window_icon_default.clone().unwrap_or_default();
    };

    let app_id_lower = app_id.to_lowercase();

    cfg.window_icons
        .as_ref()
        .and_then(|m| m.get(&app_id_lower))
        .cloned()
        .or_else(|| {
            log::warn!("No icon configured for app_id='{}'", app_id);
            cfg.window_icon_default.clone()
        })
        .unwrap_or_default()
}

fn format_icon(cfg: &Config, icon: &str, is_focused: bool, is_urgent: bool) -> String {
    let format = if is_urgent {
        cfg.window_icon_formats
            .as_ref()
            .and_then(|f| f.urgent.as_deref())
            .unwrap_or(DEFAULT_URGENT_FORMAT)
    } else if is_focused {
        cfg.window_icon_formats
            .as_ref()
            .and_then(|f| f.focused.as_deref())
            .unwrap_or(DEFAULT_FOCUSED_FORMAT)
    } else {
        cfg.window_icon_formats
            .as_ref()
            .and_then(|f| f.default.as_deref())
            .unwrap_or(DEFAULT_FORMAT)
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
    type Config = Config;

    fn init(info: &InitInfo, config: Config) -> Self {
        env_logger::init();

        // Validate window icon formats
        if let Some(ref formats) = config.window_icon_formats {
            if let Err(err) = formats.validate() {
                log::error!("Invalid window-icon-format configuration: {}", err);
            }
        }

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
    log::warn!("update_workspaces called");

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
struct WindowIconFormats {
    focused: Option<String>,
    urgent: Option<String>,
    default: Option<String>,
}

impl WindowIconFormats {
    fn validate(&self) -> Result<(), String> {
        let formats = [
            ("focused", &self.focused),
            ("urgent", &self.urgent),
            ("default", &self.default),
        ];

        for (name, format) in formats {
            if let Some(fmt) = format {
                if !fmt.contains("{icon}") {
                    return Err(format!(
                        "window-icon-format.{} must contain the substring \"{{icon}}\"",
                        name
                    ));
                }
            }
        }

        Ok(())
    }
}

// TODO: can active vs urgent styling be done with css instead of a config option?
#[derive(Deserialize, Debug, Clone)]
struct Config {
    #[serde(default, rename = "window-icons")]
    window_icons: Option<HashMap<String, String>>,
    #[serde(default, rename = "window-icon-default")]
    window_icon_default: Option<String>,
    #[serde(default, rename = "window-icon-format")]
    window_icon_formats: Option<WindowIconFormats>,
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
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: None,
        };
        let result = format_icon(&config, "🔥", false, false);
        assert_eq!(result, "🔥");
    }

    #[test]
    fn test_format_icon_focused() {
        let formats = WindowIconFormats {
            focused: Some("[{icon}]".to_string()),
            urgent: None,
            default: None,
        };
        let config = Config {
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: Some(formats),
        };
        let result = format_icon(&config, "🔥", true, false);
        assert_eq!(result, "[🔥]");
    }

    #[test]
    fn test_format_icon_urgent() {
        let formats = WindowIconFormats {
            focused: None,
            urgent: Some("!{icon}!".to_string()),
            default: None,
        };
        let config = Config {
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: Some(formats),
        };
        let result = format_icon(&config, "🔥", false, true);
        assert_eq!(result, "!🔥!");
    }

    #[test]
    fn test_format_icon_urgent_takes_precedence() {
        let formats = WindowIconFormats {
            focused: Some("[{icon}]".to_string()),
            urgent: Some("!{icon}!".to_string()),
            default: None,
        };
        let config = Config {
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: Some(formats),
        };
        let result = format_icon(&config, "🔥", true, true);
        assert_eq!(result, "!🔥!");
    }

    #[test]
    fn test_format_icon_custom_default() {
        let formats = WindowIconFormats {
            focused: None,
            urgent: None,
            default: Some("<{icon}>".to_string()),
        };
        let config = Config {
            window_icons: None,
            window_icon_default: None,
            window_icon_formats: Some(formats),
        };
        let result = format_icon(&config, "🔥", false, false);
        assert_eq!(result, "<🔥>");
    }

    #[test]
    fn test_window_icon_formats_validate_valid() {
        let formats = WindowIconFormats {
            focused: Some("[{icon}]".to_string()),
            urgent: Some("!{icon}!".to_string()),
            default: Some("{icon}".to_string()),
        };
        assert!(formats.validate().is_ok());
    }

    #[test]
    fn test_window_icon_formats_validate_missing_icon_focused() {
        let formats = WindowIconFormats {
            focused: Some("[]".to_string()),
            urgent: None,
            default: None,
        };
        let result = formats.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("focused"));
        assert!(err.contains("{icon}"));
    }

    #[test]
    fn test_window_icon_formats_validate_missing_icon_urgent() {
        let formats = WindowIconFormats {
            focused: None,
            urgent: Some("!!".to_string()),
            default: None,
        };
        let result = formats.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("urgent"));
    }

    #[test]
    fn test_window_icon_formats_validate_missing_icon_default() {
        let formats = WindowIconFormats {
            focused: None,
            urgent: None,
            default: Some("<>".to_string()),
        };
        let result = formats.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("default"));
    }

    #[test]
    fn test_window_icon_formats_validate_none_is_valid() {
        let formats = WindowIconFormats {
            focused: None,
            urgent: None,
            default: None,
        };
        assert!(formats.validate().is_ok());
    }

    #[test]
    fn test_get_raw_icon_with_mapping() {
        let mut icons = HashMap::new();
        icons.insert("firefox".to_string(), "🦊".to_string());
        icons.insert("code".to_string(), "💻".to_string());

        let config = Config {
            window_icons: Some(icons),
            window_icon_default: Some("❓".to_string()),
            window_icon_formats: None,
        };

        let window = create_test_window(Some("Firefox".to_string()));
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "🦊");
    }

    #[test]
    fn test_get_raw_icon_fallback_to_default() {
        let mut icons = HashMap::new();
        icons.insert("firefox".to_string(), "🦊".to_string());

        let config = Config {
            window_icons: Some(icons),
            window_icon_default: Some("❓".to_string()),
            window_icon_formats: None,
        };

        let window = create_test_window(Some("unknown".to_string()));
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "❓");
    }

    #[test]
    fn test_get_raw_icon_no_app_id() {
        let config = Config {
            window_icons: None,
            window_icon_default: Some("❓".to_string()),
            window_icon_formats: None,
        };

        let window = create_test_window(None);
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "❓");
    }

    #[test]
    fn test_get_raw_icon_case_insensitive() {
        let mut icons = HashMap::new();
        icons.insert("firefox".to_string(), "🦊".to_string());

        let config = Config {
            window_icons: Some(icons),
            window_icon_default: None,
            window_icon_formats: None,
        };

        let window = create_test_window(Some("FIREFOX".to_string()));
        let result = get_raw_icon(&config, &window);
        assert_eq!(result, "🦊");
    }
}
