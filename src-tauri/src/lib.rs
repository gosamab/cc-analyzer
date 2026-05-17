mod analyze;
mod commands;
mod db;
mod parser;
mod pricing;

use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<db::Db>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = db::Db::open().expect("failed to open SQLite cache");
    db.init_schema().expect("failed to init schema");
    pricing::load_from_db(&db).ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { db: Mutex::new(db) })
        .invoke_handler(tauri::generate_handler![
            commands::refresh_logs,
            commands::summary,
            commands::daily_breakdown,
            commands::sessions,
            commands::session_detail,
            commands::recommendations,
            commands::health_signals,
            commands::top_commands,
            commands::tool_usage,
            commands::utilization,
            commands::cache_stats,
            commands::clear_cache,
            commands::pricing_table,
            commands::set_pricing,
            commands::block_usage,
            commands::get_setting,
            commands::set_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cc-analyzer");
}
