pub mod errors;
pub mod schema;
pub mod selection;

#[tauri::command]
fn capture_selection_diagnostics() -> selection::SelectionDiagnostics {
    selection::capture_selection_diagnostics()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![capture_selection_diagnostics])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
