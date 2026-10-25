pub mod commands;
pub mod domain;
pub mod errors;
pub mod infrastructure;
pub mod repositories;
pub mod services;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::domain::import::StartupRecoveryReport;
use crate::errors::AppResult;
use crate::infrastructure::database::Database;
use crate::infrastructure::{filesystem, logging};
use crate::services::import_service::{self, ImportManager};
use crate::services::metadata_service::MetadataService;

/// 全局应用状态：数据库、导入编排器与路径信息。
/// 所有字段均为 `Arc`/纯数据，命令可直接 clone 到阻塞线程。
#[derive(Clone)]
pub struct AppState {
    pub database: Arc<Database>,
    pub imports: Arc<ImportManager>,
    pub metadata: Arc<MetadataService>,
    pub thumbnail_cache: PathBuf,
    pub database_path: PathBuf,
    pub log_directory: PathBuf,
    pub app_version: String,
    pub tauri_version: String,
    pub startup_report: Arc<Mutex<Option<StartupRecoveryReport>>>,
}

fn initialize(app: &tauri::App) -> AppResult<AppState> {
    let data_directory = app.path().app_data_dir().map_err(|error| {
        errors::AppError::internal("无法定位应用数据目录").with_detail(error.to_string())
    })?;
    let log_directory = app.path().app_log_dir().map_err(|error| {
        errors::AppError::internal("无法定位日志目录").with_detail(error.to_string())
    })?;

    logging::init(&log_directory)?;

    let database_path = data_directory.join("creatorflow.db");
    tracing::info!(path = %database_path.display(), "打开本地数据库");
    let database = Arc::new(Database::open(&database_path)?);

    let cache_directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| {
            errors::AppError::internal("无法定位缓存目录").with_detail(error.to_string())
        })?
        .join("thumbnails");
    filesystem::ensure_dir(&cache_directory)?;

    let startup_report = match import_service::recover_incomplete_jobs(&database) {
        Ok(report) => {
            if report.examined_jobs > 0 {
                tracing::info!(
                    examined = report.examined_jobs,
                    completed = report.completed_jobs,
                    failed = report.failed_jobs,
                    cleaned = report.cleaned_temporary_files,
                    "已检查未完成的导入任务"
                );
            }
            Some(report)
        }
        Err(error) => {
            tracing::error!(
                message = %error.message,
                detail = error.detail.as_deref().unwrap_or(""),
                "启动检查导入任务失败"
            );
            None
        }
    };

    let metadata = Arc::new(MetadataService::new(
        Some(app.handle().clone()),
        Arc::clone(&database),
        cache_directory.clone(),
    ));
    metadata.backfill(500);

    let imports = Arc::new(ImportManager::new(
        app.handle().clone(),
        Arc::clone(&database),
        Some(Arc::clone(&metadata)),
    ));

    Ok(AppState {
        database,
        imports,
        metadata,
        thumbnail_cache: cache_directory,
        database_path,
        log_directory,
        app_version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION.to_string(),
        startup_report: Arc::new(Mutex::new(startup_report)),
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = initialize(app).map_err(|error| {
                tracing::error!(
                    message = %error.message,
                    detail = error.detail.as_deref().unwrap_or(""),
                    "应用初始化失败"
                );
                Box::<dyn std::error::Error>::from(error)
            })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::work_commands::create_work,
            commands::work_commands::list_works,
            commands::work_commands::get_work,
            commands::work_commands::rename_work,
            commands::work_commands::archive_work,
            commands::work_commands::unarchive_work,
            commands::work_commands::delete_work,
            commands::work_commands::update_work_status,
            commands::work_commands::list_sections,
            commands::asset_commands::list_assets,
            commands::asset_commands::search_assets,
            commands::asset_commands::relocate_asset,
            commands::asset_commands::list_work_tags,
            commands::asset_commands::list_asset_tags,
            commands::asset_commands::add_asset_tag,
            commands::asset_commands::remove_asset_tag,
            commands::asset_commands::retry_asset_metadata,
            commands::asset_commands::delete_asset,
            commands::asset_commands::batch_delete_assets,
            commands::asset_commands::batch_add_tag,
            commands::asset_commands::batch_remove_tag,
            commands::asset_commands::get_media_capabilities,
            commands::asset_commands::list_all_tags,
            commands::asset_commands::get_asset_preview,
            commands::asset_commands::open_asset,
            commands::asset_commands::reveal_asset,
            commands::import_commands::prepare_import,
            commands::import_commands::start_import,
            commands::import_commands::list_import_jobs,
            commands::import_commands::cancel_import,
            commands::import_commands::retry_import,
            commands::import_commands::get_startup_recovery_report,
            commands::settings_commands::get_settings,
            commands::settings_commands::set_workspace_root,
            commands::settings_commands::set_last_opened_work,
            commands::settings_commands::get_app_info,
            commands::backup_commands::get_backup_overview,
            commands::backup_commands::create_database_backup,
            commands::backup_commands::verify_database_backup,
            commands::backup_commands::restore_database_backup,
            commands::backup_commands::set_backup_directory,
            commands::cache_commands::get_cache_stats,
            commands::cache_commands::set_cache_limit,
            commands::cache_commands::clear_thumbnail_cache,
        ])
        .run(tauri::generate_context!())
        .expect("CreatorFlow 启动失败");
}
