use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const SHOW_MENU_ID: &str = "show";
const SETTINGS_MENU_ID: &str = "settings";
const QUIT_MENU_ID: &str = "quit";

pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, SHOW_MENU_ID, "Show Lexi", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS_MENU_ID, "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_MENU_ID, "Quit Lexi", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default window icon is not configured")?;

    TrayIconBuilder::with_id("main")
        .tooltip("Lexi")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(&tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_MENU_ID => show_main_window(app),
            SETTINGS_MENU_ID => show_settings_window(app),
            QUIT_MENU_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Some(settings_state) = app.try_state::<crate::settings::SettingsState>() {
            if let Ok(settings) = settings_state.load_settings(app) {
                let _ = window.set_always_on_top(settings.popup_always_on_top);
            }
        }
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn show_settings_window(app: &AppHandle) {
    let window = match app.get_webview_window("settings") {
        Some(window) => window,
        None => match build_settings_window(app) {
            Ok(window) => window,
            Err(_) => return,
        },
    };

    let _ = window.center();
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn build_settings_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("Lexi Settings")
        .inner_size(420.0, 760.0)
        .min_inner_size(380.0, 520.0)
        .resizable(true)
        .transparent(false)
        .decorations(true)
        .skip_taskbar(false)
        .visible(true)
        .center()
        .build()
}
