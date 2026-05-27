pub mod errors;
pub mod llm;
pub mod schema;
pub mod secrets;
pub mod selection;
pub mod settings;
pub mod shortcut;

#[tauri::command]
fn capture_selection_diagnostics() -> selection::SelectionDiagnostics {
    selection::capture_selection_diagnostics()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(llm::SelectedTextState::default())
        .manage(settings::SettingsState::default())
        .manage(shortcut::ShortcutRegistrationState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            capture_selection_diagnostics,
            llm::list_provider_models,
            llm::run_transform,
            llm::run_transform_stream,
            settings::get_provider_settings,
            settings::update_provider_settings,
            shortcut::get_shortcut_status,
        ])
        .setup(shortcut::setup)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            shortcut::unregister_all(app_handle);
        }
    });
}
