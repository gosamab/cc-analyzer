use crate::{analyze, parser, AppState};
use tauri::State;

fn map_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn refresh_logs(state: State<'_, AppState>) -> Result<usize, String> {
    let db = state.db.lock().map_err(map_err)?;
    parser::refresh(&db).map_err(map_err)
}

#[tauri::command]
pub fn summary(
    state: State<'_, AppState>,
    since: Option<String>,
    until: Option<String>,
    project: Option<String>,
) -> Result<analyze::Summary, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::summary(&db, since.as_deref(), until.as_deref(), project.as_deref()).map_err(map_err)
}

#[tauri::command]
pub fn daily_breakdown(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::DayRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::daily_breakdown(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn sessions(
    state: State<'_, AppState>,
    since: Option<String>,
    until: Option<String>,
) -> Result<Vec<analyze::SessionRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::sessions(&db, since.as_deref(), until.as_deref()).map_err(map_err)
}

#[tauri::command]
pub fn session_detail(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<analyze::SessionDetail, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::session_detail(&db, &session_id).map_err(map_err)
}

#[tauri::command]
pub fn recommendations(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::Recommendation>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::recommendations(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn health_signals(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::HealthSignal>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::health_signals(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn tool_usage(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::ToolUsageRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::tool_usage(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn skill_usage(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::SkillUsageRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::skill_usage(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn mcp_usage(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::McpUsageRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::mcp_usage(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn slash_command_usage(
    state: State<'_, AppState>,
    since: String,
    until: String,
) -> Result<Vec<analyze::SlashCommandRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::slash_command_usage(&db, &since, &until).map_err(map_err)
}

#[tauri::command]
pub fn top_commands(
    state: State<'_, AppState>,
    since: String,
    until: String,
    limit: i64,
) -> Result<Vec<analyze::CommandRow>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::top_commands(&db, &since, &until, limit).map_err(map_err)
}

#[tauri::command]
pub fn cache_stats(state: State<'_, AppState>) -> Result<analyze::CacheStats, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::cache_stats(&db).map_err(map_err)
}

#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::clear_cache(&db).map_err(map_err)
}

#[tauri::command]
pub fn pricing_table() -> Vec<analyze::PricingRow> {
    analyze::pricing_table()
}

#[tauri::command]
pub fn set_pricing(
    state: State<'_, AppState>,
    rows: Vec<analyze::PricingRow>,
) -> Result<usize, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::set_pricing(&db, rows).map_err(map_err)
}

#[tauri::command]
pub fn utilization(
    state: State<'_, AppState>,
    day: String,
) -> Result<analyze::Utilization, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::utilization(&db, &day).map_err(map_err)
}

#[tauri::command]
pub fn block_usage(state: State<'_, AppState>) -> Result<analyze::BlockUsage, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::block_usage(&db).map_err(map_err)
}

#[tauri::command]
pub fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::get_setting(&db, &key).map_err(map_err)
}

#[tauri::command]
pub fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(map_err)?;
    analyze::set_setting(&db, &key, &value).map_err(map_err)
}
