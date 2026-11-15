mod commands;
mod state;
mod tracker;
mod updates;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(updates::plugin())
        .manage(updates::PendingUpdate::default())
        .setup(|app| {
            let state = state::AppState::init()?;
            app.manage(state);
            tracker::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_desktop,
            commands::import_shortcuts,
            commands::scan_launchers,
            commands::import_launcher_games,
            commands::sync_launchers,
            commands::list_games,
            commands::query_games,
            commands::get_game,
            commands::update_game,
            commands::launch_game,
            commands::running_games,
            commands::sessions,
            commands::undo_import,
            commands::remove_game,
            commands::restore_all,
            commands::list_tags,
            commands::add_tag,
            commands::remove_tag,
            commands::list_collections,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::set_in_collection,
            commands::list_views,
            commands::save_view,
            commands::delete_view,
            commands::art_options,
            commands::set_art_from_url,
            commands::set_art_from_file,
            commands::refresh_metadata,
            commands::get_settings,
            commands::save_keys,
            commands::open_data_folder,
            updates::check_update,
            updates::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
