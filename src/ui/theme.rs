use crate::MainWindow;
use slint::{Color, ComponentHandle, SharedString};
use std::path::PathBuf;

/// Theme IDs matching the Slint theme-ids array.
pub const THEME_IDS: &[&str] = &["terminal", "light", "monokai", "solarized", "midnight"];

/// Get the config directory for theme persistence.
fn theme_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".gwd").join("theme"))
}

/// Load saved theme from disk. Returns theme index (0-4).
pub fn load_saved_theme() -> usize {
    if let Some(path) = theme_config_path() {
        if let Ok(contents) = std::fs::read_to_string(path) {
            let id = contents.trim();
            return THEME_IDS.iter().position(|&t| t == id).unwrap_or(0);
        }
    }
    0 // default: Terminal Dark
}

/// Save theme selection to disk.
pub fn save_theme(index: usize) {
    if let Some(path) = theme_config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let id = THEME_IDS.get(index).unwrap_or(&"terminal");
        let _ = std::fs::write(path, id);
    }
}

/// Apply theme colors to the MainWindow's Theme global.
pub fn apply_theme(ui: &MainWindow, index: usize) {
    let colors = match index {
        0 => terminal_dark(),
        1 => light(),
        2 => monokai(),
        3 => solarized_dark(),
        4 => midnight_blue(),
        _ => terminal_dark(),
    };

    ui.global::<crate::Theme>().set_colors(colors);
    ui.global::<crate::Theme>().set_current_theme(SharedString::from(THEME_IDS[index.min(4)]));
    ui.set_selected_theme_index(index as i32);
}

fn c(hex: u32) -> Color {
    Color::from_argb_encoded(0xFF000000 | hex)
}

fn terminal_dark() -> crate::ThemeColors {
    crate::ThemeColors {
        bg_main: c(0x0a0e14),
        bg_surface: c(0x111720),
        bg_sidebar: c(0x0d1117),
        bg_hover: c(0x1a2233),
        bg_input: c(0x0d1117),
        bg_header: c(0x0d1117),
        border: c(0x1e2d3d),
        border_light: c(0x162029),
        text_primary: c(0xb0c4d8),
        text_muted: c(0x5a7a94),
        text_heading: c(0xe0ecf5),
        text_bright: c(0xffffff),
        accent_green: c(0x00e676),
        accent_red: c(0xff5252),
        accent_yellow: c(0xffd740),
        accent_cyan: c(0x00e5ff),
        accent_purple: c(0xb388ff),
        accent_orange: c(0xff9100),
        status_in_progress: c(0xffd740),
        status_queued: c(0x5a7a94),
        status_success: c(0x00e676),
        status_failure: c(0xff5252),
        status_cancelled: c(0x5a7a94),
        tint_cyan: c(0x0a2a35),
        tint_green: c(0x0a2a1a),
        tint_red: c(0x2a0a0a),
        shadow_color: c(0x000000),
    }
}

fn light() -> crate::ThemeColors {
    crate::ThemeColors {
        bg_main: c(0xf5f5f5),
        bg_surface: c(0xffffff),
        bg_sidebar: c(0xfafafa),
        bg_hover: c(0xeeeeee),
        bg_input: c(0xffffff),
        bg_header: c(0xfafafa),
        border: c(0xe0e0e0),
        border_light: c(0xf0f0f0),
        text_primary: c(0x333333),
        text_muted: c(0x888888),
        text_heading: c(0x111111),
        text_bright: c(0x000000),
        accent_green: c(0x2e7d32),
        accent_red: c(0xc62828),
        accent_yellow: c(0xf57f17),
        accent_cyan: c(0x00838f),
        accent_purple: c(0x6a1b9a),
        accent_orange: c(0xe65100),
        status_in_progress: c(0xf57f17),
        status_queued: c(0x888888),
        status_success: c(0x2e7d32),
        status_failure: c(0xc62828),
        status_cancelled: c(0x888888),
        tint_cyan: c(0xe0f7fa),
        tint_green: c(0xe8f5e9),
        tint_red: c(0xfce4ec),
        shadow_color: c(0xcccccc),
    }
}

fn monokai() -> crate::ThemeColors {
    crate::ThemeColors {
        bg_main: c(0x272822),
        bg_surface: c(0x2d2e27),
        bg_sidebar: c(0x1e1f1a),
        bg_hover: c(0x3e3d32),
        bg_input: c(0x1e1f1a),
        bg_header: c(0x1e1f1a),
        border: c(0x49483e),
        border_light: c(0x3e3d32),
        text_primary: c(0xf8f8f2),
        text_muted: c(0x75715e),
        text_heading: c(0xf8f8f2),
        text_bright: c(0xffffff),
        accent_green: c(0xa6e22e),
        accent_red: c(0xf92672),
        accent_yellow: c(0xe6db74),
        accent_cyan: c(0x66d9ef),
        accent_purple: c(0xae81ff),
        accent_orange: c(0xfd971f),
        status_in_progress: c(0xe6db74),
        status_queued: c(0x75715e),
        status_success: c(0xa6e22e),
        status_failure: c(0xf92672),
        status_cancelled: c(0x75715e),
        tint_cyan: c(0x1a2a30),
        tint_green: c(0x1a2a10),
        tint_red: c(0x2a1020),
        shadow_color: c(0x000000),
    }
}

fn solarized_dark() -> crate::ThemeColors {
    crate::ThemeColors {
        bg_main: c(0x002b36),
        bg_surface: c(0x073642),
        bg_sidebar: c(0x002028),
        bg_hover: c(0x0a4050),
        bg_input: c(0x002028),
        bg_header: c(0x002028),
        border: c(0x586e75),
        border_light: c(0x073642),
        text_primary: c(0x839496),
        text_muted: c(0x586e75),
        text_heading: c(0x93a1a1),
        text_bright: c(0xfdf6e3),
        accent_green: c(0x859900),
        accent_red: c(0xdc322f),
        accent_yellow: c(0xb58900),
        accent_cyan: c(0x2aa198),
        accent_purple: c(0x6c71c4),
        accent_orange: c(0xcb4b16),
        status_in_progress: c(0xb58900),
        status_queued: c(0x586e75),
        status_success: c(0x859900),
        status_failure: c(0xdc322f),
        status_cancelled: c(0x586e75),
        tint_cyan: c(0x003540),
        tint_green: c(0x003520),
        tint_red: c(0x350020),
        shadow_color: c(0x001a20),
    }
}

fn midnight_blue() -> crate::ThemeColors {
    crate::ThemeColors {
        bg_main: c(0x0d1b2a),
        bg_surface: c(0x1b2838),
        bg_sidebar: c(0x0a1628),
        bg_hover: c(0x243447),
        bg_input: c(0x0a1628),
        bg_header: c(0x0a1628),
        border: c(0x2a3f55),
        border_light: c(0x1b2838),
        text_primary: c(0xa8c4d8),
        text_muted: c(0x5a7a94),
        text_heading: c(0xd0e4f0),
        text_bright: c(0xffffff),
        accent_green: c(0x4caf50),
        accent_red: c(0xef5350),
        accent_yellow: c(0xffc107),
        accent_cyan: c(0x29b6f6),
        accent_purple: c(0xab47bc),
        accent_orange: c(0xff7043),
        status_in_progress: c(0xffc107),
        status_queued: c(0x5a7a94),
        status_success: c(0x4caf50),
        status_failure: c(0xef5350),
        status_cancelled: c(0x5a7a94),
        tint_cyan: c(0x0a2040),
        tint_green: c(0x0a2010),
        tint_red: c(0x200a10),
        shadow_color: c(0x000510),
    }
}
