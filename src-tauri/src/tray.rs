use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, PhysicalPosition, Position, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

const SHOW_MENU_ID: &str = "show";
const SETTINGS_MENU_ID: &str = "settings";
const QUIT_MENU_ID: &str = "quit";
const WINDOW_EDGE_MARGIN: i32 = 16;

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
        let _ = position_window_at_monitor_right_edge(&window);
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

fn position_window_at_monitor_right_edge(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let work_area = monitor.work_area();
    let window_size = window.outer_size()?;
    let x = work_area.position.x + work_area.size.width as i32
        - window_size.width as i32
        - WINDOW_EDGE_MARGIN;
    let y = work_area.position.y + WINDOW_EDGE_MARGIN;

    window.set_position(Position::Physical(PhysicalPosition { x, y }))
}
