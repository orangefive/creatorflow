use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::domain::asset::{mime_for_extension, AssetStatus, MediaKind};
use crate::domain::ids::new_id;
use crate::domain::import::{
    ImportCandidate, ImportJob, ImportProgressEvent, ImportStatus, StartupRecoveryReport,
};
use crate::domain::section::{suggest_section_for_extension, Section, SectionKey};
use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult, ErrorCode};
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem;
use crate::repositories::{activity, assets, import_jobs, sections, works};

pub const IMPORT_PROGRESS_EVENT: &str = "import://progress";
/// 进度写库与事件节流间隔，避免大文件复制期间频繁打断 IO。
const PROGRESS_INTERVAL: Duration = Duration::from_millis(180);

/// 查询导入任务列表；`work_id` 为空时返回全部作品的任务。
pub fn list_jobs(
    connection: &rusqlite::Connection,
    work_id: Option<&str>,
) -> AppResult<Vec<ImportJob>> {
    import_jobs::list(connection, work_id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSource {
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareImportResult {
    pub candidates: Vec<ImportCandidate>,
    pub skipped: Vec<SkippedSource>,
}

/**
 * 导入任务编排器。
 *
 * - 每个任务一个后台线程，文件复制不阻塞 UI 线程，也不持有数据库事务。
 * - 取消通过 `AtomicBool` 传递，复制循环每个缓冲区检查一次。
 * - `app` 为 `None` 时不发送前端事件（集成测试与故障排查使用）。
 */
pub struct ImportManager {
    app: Option<AppHandle>,
    database: Arc<Database>,
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ImportManager {
    pub fn new(app: AppHandle, database: Arc<Database>) -> Self {
        Self {
            app: Some(app),
            database,
            cancellations: Mutex::new(HashMap::new()),
        }
    }

    #[doc(hidden)]
    pub fn headless(database: Arc<Database>) -> Self {
        Self {
            app: None,
            database,
            cancellations: Mutex::new(HashMap::new()),
        }
    }

    pub fn database(&self) -> &Arc<Database> {
        &self.database
    }

    fn emit_progress(&self, job: &ImportJob) {
        if let Some(app) = &self.app {
            let payload = ImportProgressEvent::from(job);
            if let Err(error) = app.emit(IMPORT_PROGRESS_EVENT, payload) {
                tracing::warn!(error = %error, "发送导入进度事件失败");
            }
        }
    }

    fn load_job(&self, job_id: &str) -> AppResult<ImportJob> {
        self.database.with_conn(|connection| {
            import_jobs::find_by_id(connection, job_id)?
                .ok_or_else(|| AppError::not_found("导入任务不存在"))
        })
    }

    fn mark_status(
        &self,
        job_id: &str,
        status: ImportStatus,
        bytes: Option<i64>,
        error: Option<(&str, &str)>,
        asset_id: Option<&str>,
    ) -> AppResult<ImportJob> {
        let updated_at = now_iso();
        let completed_at = matches!(status, ImportStatus::Completed).then(|| updated_at.clone());
        self.database.with_conn(|connection| {
            import_jobs::update_status(
                connection,
                job_id,
                &import_jobs::JobStatusUpdate {
                    status,
                    bytes_copied: bytes,
                    error_code: error.map(|(code, _)| code),
                    error_message: error.map(|(_, message)| message),
                    asset_id,
                    completed_at: completed_at.as_deref(),
                    updated_at: &updated_at,
                },
            )?;
            Ok(())
        })?;
        let job = self.load_job(job_id)?;
        self.emit_progress(&job);
        Ok(job)
    }

    /// 生成导入候选：只返回存在的文件，其余记录跳过原因，不静默丢弃。
    pub fn prepare(
        &self,
        work_id: &str,
        source_paths: &[String],
    ) -> AppResult<PrepareImportResult> {
        self.database.with_conn(|connection| {
            works::find(connection, work_id)?
                .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?;
            Ok::<(), AppError>(())
        })?;

        let mut candidates = Vec::new();
        let mut skipped = Vec::new();
        for source_path in source_paths {
            let path = PathBuf::from(source_path);
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    let file_name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| source_path.clone());
                    let extension = path
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default();
                    candidates.push(ImportCandidate {
                        source_path: source_path.clone(),
                        file_name,
                        size_bytes: metadata.len() as i64,
                        suggested_section_key: suggest_section_for_extension(extension),
                    });
                }
                Ok(_) => skipped.push(SkippedSource {
                    source_path: source_path.clone(),
                    reason: "不是文件（可能选择了文件夹）".to_string(),
                }),
                Err(_) => skipped.push(SkippedSource {
                    source_path: source_path.clone(),
                    reason: "源文件不存在或无法访问".to_string(),
                }),
            }
        }
        Ok(PrepareImportResult {
            candidates,
            skipped,
        })
    }

    /// 创建导入任务记录（不启动执行），供后台线程与同步测试共用。
    fn create_job_rows(
        &self,
        work_id: &str,
        section_key: &str,
        source_paths: &[String],
    ) -> AppResult<Vec<ImportJob>> {
        let section_key = SectionKey::parse(section_key)?;
        if source_paths.is_empty() {
            return Err(AppError::validation("请选择要导入的文件"));
        }

        let (work, section) = self.database.with_conn(|connection| {
            let work = works::find(connection, work_id)?
                .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?;
            let section = sections::find_by_key(connection, work_id, section_key)?
                .ok_or_else(|| AppError::not_found("目标分区不存在"))?;
            Ok((work, section))
        })?;

        let mut created_jobs = Vec::new();
        for source_path in source_paths {
            let now = now_iso();
            let job_id = new_id();
            let result = self.database.with_conn(|connection| {
                import_jobs::insert(
                    connection,
                    &import_jobs::NewImportJob {
                        id: job_id.clone(),
                        work_id: work.id.clone(),
                        section_id: section.id.clone(),
                        source_path: source_path.clone(),
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    },
                )
            });
            match result {
                Ok(()) => created_jobs.push(self.load_job(&job_id)?),
                Err(error) => {
                    tracing::error!(
                        source = %source_path,
                        message = %error.message,
                        "创建导入任务失败"
                    );
                    return Err(error);
                }
            }
        }
        Ok(created_jobs)
    }

    /// 创建导入任务并启动后台线程。
    pub fn start(
        self: &Arc<Self>,
        work_id: &str,
        section_key: &str,
        source_paths: &[String],
    ) -> AppResult<Vec<ImportJob>> {
        let jobs = self.create_job_rows(work_id, section_key, source_paths)?;
        for job in &jobs {
            self.spawn_job(job.id.clone());
        }
        Ok(jobs)
    }

    fn spawn_job(self: &Arc<Self>, job_id: String) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut map) = self.cancellations.lock() {
            map.insert(job_id.clone(), Arc::clone(&cancel_flag));
        }

        let manager = Arc::clone(self);
        let thread_job_id = job_id.clone();
        std::thread::spawn(move || {
            manager.run_job(&thread_job_id, &cancel_flag);
            if let Ok(mut map) = manager.cancellations.lock() {
                map.remove(&thread_job_id);
            }
        });
    }

    /// 同步执行一个任务（线程内调用；也用于集成测试）。
    pub fn run_job(&self, job_id: &str, cancel: &AtomicBool) {
        if let Err(error) = self.execute_job(job_id, cancel) {
            self.finish_with_error(job_id, &error);
        }
    }

    fn execute_job(&self, job_id: &str, cancel: &AtomicBool) -> AppResult<()> {
        let job = self.load_job(job_id)?;
        let (work, section) = self.database.with_conn(|connection| {
            let work = works::find(connection, &job.work_id)?
                .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?;
            let section = sections::find_by_id(connection, &job.section_id)?
                .ok_or_else(|| AppError::not_found("目标分区不存在"))?;
            Ok((work, section))
        })?;

        let source = PathBuf::from(&job.source_path);
        if !source.is_file() {
            return Err(
                AppError::not_found("源文件不存在或已被移动").with_detail(job.source_path.clone())
            );
        }
        let source_size = filesystem::file_size(&source)? as i64;
        let source_name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| AppError::validation("源文件名称无效"))?;

        self.mark_status(job_id, ImportStatus::Copying, Some(0), None, None)?;

        let root = PathBuf::from(&work.root_path);
        let target_dir = filesystem::resolve_within(&root, &section.relative_path)?;
        filesystem::ensure_dir(&target_dir)?;

        let unique_name = filesystem::unique_file_name(&target_dir, &source_name)?;
        let target_relative_path = format!("{}/{}", section.relative_path, unique_name);
        let temporary_name = format!(".{}.{}.part", unique_name, &new_id()[..8]);
        let temporary_relative_path = format!("{}/{}", section.relative_path, temporary_name);
        let target_path = target_dir.join(&unique_name);
        let temporary_path = target_dir.join(&temporary_name);

        self.database.with_conn(|connection| {
            import_jobs::update_paths(
                connection,
                job_id,
                &target_relative_path,
                &temporary_relative_path,
                source_size,
                &now_iso(),
            )?;
            Ok(())
        })?;
        self.emit_progress(&self.load_job(job_id)?);

        let last_emit = Mutex::new(Instant::now());
        let copy_result = filesystem::copy_streaming(&source, &temporary_path, cancel, |copied| {
            let should_emit = last_emit
                .lock()
                .map(|last| last.elapsed() >= PROGRESS_INTERVAL)
                .unwrap_or(false);
            if !should_emit {
                return;
            }
            if let Ok(mut last) = last_emit.lock() {
                *last = Instant::now();
            }
            let updated = self.database.with_conn(|connection| {
                import_jobs::update_progress(connection, job_id, copied as i64, &now_iso())
            });
            if let Err(error) = updated {
                tracing::warn!(message = %error.message, "更新导入进度失败");
                return;
            }
            if let Ok(current) = self.load_job(job_id) {
                self.emit_progress(&current);
            }
        });

        if let Err(error) = copy_result {
            let _ = filesystem::remove_file_if_exists(&temporary_path);
            return Err(error);
        }

        self.mark_status(
            job_id,
            ImportStatus::Verifying,
            Some(source_size),
            None,
            None,
        )?;

        let copied_size = filesystem::file_size(&temporary_path)? as i64;
        if copied_size != source_size {
            let _ = filesystem::remove_file_if_exists(&temporary_path);
            return Err(AppError::internal(format!(
                "文件校验失败：源文件 {source_size} 字节，副本 {copied_size} 字节"
            )));
        }

        if target_path.exists() {
            let _ = filesystem::remove_file_if_exists(&temporary_path);
            return Err(AppError::conflict("目标文件已存在，已阻止覆盖"));
        }
        fs::rename(&temporary_path, &target_path).map_err(|error| {
            let _ = filesystem::remove_file_if_exists(&temporary_path);
            AppError::io_context("无法将临时文件重命名为正式文件", error)
        })?;

        let asset_id = register_asset_and_complete(
            &self.database,
            &job,
            &section,
            &target_relative_path,
            &unique_name,
            &source_name,
            source_size,
        )?;

        let job = self.mark_status(
            job_id,
            ImportStatus::Completed,
            Some(source_size),
            None,
            Some(&asset_id),
        )?;
        self.emit_progress(&job);
        Ok(())
    }

    fn finish_with_error(&self, job_id: &str, error: &AppError) {
        let status = if error.code == ErrorCode::Cancelled {
            ImportStatus::Cancelled
        } else {
            ImportStatus::Failed
        };

        let job = match self.load_job(job_id) {
            Ok(job) => job,
            Err(load_error) => {
                tracing::error!(message = %load_error.message, "无法读取失败的导入任务");
                return;
            }
        };

        // 清理临时文件；清理失败要留下记录，不能假装干净。
        if let Some(temp_relative) = &job.temporary_relative_path {
            let work = self
                .database
                .with_conn(|connection| works::find(connection, &job.work_id))
                .ok()
                .flatten();
            if let Some(work) = work {
                match filesystem::resolve_within(Path::new(&work.root_path), temp_relative) {
                    Ok(path) => match filesystem::remove_file_if_exists(&path) {
                        Ok(true) => tracing::info!(path = %path.display(), "已清理导入临时文件"),
                        Ok(false) => {}
                        Err(cleanup_error) => tracing::warn!(
                            message = %cleanup_error.message,
                            path = %path.display(),
                            "清理导入临时文件失败"
                        ),
                    },
                    Err(resolve_error) => tracing::warn!(
                        message = %resolve_error.message,
                        "临时文件路径校验失败，未执行清理"
                    ),
                }
            }
        }

        let code = error.code.as_str();
        let message = if status == ImportStatus::Cancelled {
            "导入已取消".to_string()
        } else {
            error.message.clone()
        };
        match self.mark_status(job_id, status, None, Some((code, message.as_str())), None) {
            Ok(updated) => {
                self.database
                    .with_conn(|connection| {
                        activity::record_best_effort(
                            connection,
                            Some(&updated.work_id),
                            None,
                            if status == ImportStatus::Cancelled {
                                "import.cancelled"
                            } else {
                                "import.failed"
                            },
                            &format!("导入「{}」失败：{message}", updated.file_name),
                            None,
                        );
                        Ok::<(), AppError>(())
                    })
                    .ok();
            }
            Err(update_error) => tracing::error!(
                message = %update_error.message,
                "无法写入导入失败状态"
            ),
        }
    }

    pub fn cancel(&self, job_id: &str) -> AppResult<()> {
        let job = self.load_job(job_id)?;
        if job.status.is_terminal() {
            return Err(AppError::validation("该导入任务已结束，无需取消"));
        }

        if let Ok(map) = self.cancellations.lock() {
            if let Some(flag) = map.get(job_id) {
                flag.store(true, Ordering::SeqCst);
                return Ok(());
            }
        }

        // 尚未进入复制阶段的任务直接置为取消。
        self.mark_status(
            job_id,
            ImportStatus::Cancelled,
            None,
            Some(("cancelled", "导入已取消")),
            None,
        )?;
        Ok(())
    }

    pub fn retry(self: &Arc<Self>, job_id: &str) -> AppResult<ImportJob> {
        let job = self.load_job(job_id)?;
        if !matches!(job.status, ImportStatus::Failed | ImportStatus::Cancelled) {
            return Err(AppError::validation("只有失败或已取消的任务可以重试"));
        }
        if !Path::new(&job.source_path).is_file() {
            return Err(AppError::not_found(
                "源文件已不存在，无法重试。请重新选择文件导入。",
            ));
        }

        self.database
            .with_conn(|connection| import_jobs::reset_for_retry(connection, job_id, &now_iso()))?;
        let reset = self.load_job(job_id)?;
        self.spawn_job(reset.id.clone());
        Ok(reset)
    }

    /// 测试与故障排查用的同步入口：创建任务并立即逐个执行，不启动后台线程。
    #[doc(hidden)]
    pub fn start_inline(
        self: &Arc<Self>,
        work_id: &str,
        section_key: &str,
        source_paths: &[String],
    ) -> AppResult<Vec<ImportJob>> {
        let jobs = self.create_job_rows(work_id, section_key, source_paths)?;
        for job in &jobs {
            let flag = AtomicBool::new(false);
            self.run_job(&job.id, &flag);
        }
        jobs.iter()
            .map(|job| self.load_job(&job.id))
            .collect::<AppResult<Vec<_>>>()
    }
}

#[allow(clippy::too_many_arguments)]
fn register_asset_and_complete(
    database: &Database,
    job: &ImportJob,
    section: &Section,
    relative_path: &str,
    display_name: &str,
    original_name: &str,
    size_bytes: i64,
) -> AppResult<String> {
    let now = now_iso();
    let extension = Path::new(display_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let media_kind: MediaKind = section.key.media_kind();
    let mime = extension
        .as_deref()
        .map(mime_for_extension)
        .unwrap_or("application/octet-stream")
        .to_string();

    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启素材登记事务", error))?;

        let existing = assets::find_by_relative_path(&transaction, &job.work_id, relative_path)?;
        let asset_id = match existing {
            Some(asset) => {
                assets::update_status(&transaction, &asset.id, AssetStatus::Ready, &now)?;
                asset.id
            }
            None => {
                let asset_id = new_id();
                assets::insert(
                    &transaction,
                    &assets::NewAsset {
                        id: asset_id.clone(),
                        work_id: job.work_id.clone(),
                        section_id: section.id.clone(),
                        display_name: display_name.to_string(),
                        original_name: original_name.to_string(),
                        relative_path: relative_path.to_string(),
                        source_path: Some(job.source_path.clone()),
                        extension,
                        detected_mime_type: Some(mime),
                        media_kind,
                        size_bytes,
                        status: AssetStatus::Ready,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    },
                )?;
                asset_id
            }
        };

        import_jobs::update_status(
            &transaction,
            &job.id,
            &import_jobs::JobStatusUpdate {
                status: ImportStatus::Completed,
                bytes_copied: Some(size_bytes),
                error_code: None,
                error_message: None,
                asset_id: Some(&asset_id),
                completed_at: Some(&now),
                updated_at: &now,
            },
        )?;

        activity::record_best_effort(
            &transaction,
            Some(&job.work_id),
            Some(&asset_id),
            "import.completed",
            &format!("导入素材「{display_name}」到{}分区", section.name),
            None,
        );

        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交素材登记事务失败", error))?;
        Ok(asset_id)
    })
}

/// 启动恢复：检查非终态任务，依据磁盘实际状态决定完成、失败或清理临时文件。
/// 绝不在未验证文件的情况下把任务标记为成功。
pub fn recover_incomplete_jobs(database: &Database) -> AppResult<StartupRecoveryReport> {
    let jobs = database.with_conn(|connection| import_jobs::list_unfinished(connection))?;
    let mut report = StartupRecoveryReport::default();

    for job in jobs {
        report.examined_jobs += 1;
        let work = match database.with_conn(|connection| works::find(connection, &job.work_id))? {
            Some(work) => work,
            None => {
                mark_interrupted(database, &job, "作品记录不存在")?;
                report.failed_jobs += 1;
                continue;
            }
        };
        let root = PathBuf::from(&work.root_path);

        if let Some(temp_relative) = &job.temporary_relative_path {
            if let Ok(path) = filesystem::resolve_within(&root, temp_relative) {
                match filesystem::remove_file_if_exists(&path) {
                    Ok(true) => report.cleaned_temporary_files += 1,
                    Ok(false) => {}
                    Err(error) => report
                        .messages
                        .push(format!("临时文件清理失败：{}", error.message)),
                }
            }
        }

        let target_relative = match &job.target_relative_path {
            Some(value) => value.clone(),
            None => {
                mark_interrupted(database, &job, "导入在写入目标文件前中断")?;
                report.failed_jobs += 1;
                report
                    .messages
                    .push(format!("「{}」导入中断，可重试", job.file_name));
                continue;
            }
        };

        let target_path = match filesystem::resolve_within(&root, &target_relative) {
            Ok(path) => path,
            Err(error) => {
                mark_interrupted(database, &job, &error.message)?;
                report.failed_jobs += 1;
                continue;
            }
        };

        if target_path.is_file() {
            let actual_size = filesystem::file_size(&target_path)? as i64;
            let expected_ok = job
                .source_size_bytes
                .map(|expected| expected == actual_size)
                .unwrap_or(true);
            if expected_ok {
                let section = database
                    .with_conn(|connection| sections::find_by_id(connection, &job.section_id))?;
                if let Some(section) = section {
                    let display_name = Path::new(&target_relative)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| job.file_name.clone());
                    let original_name = Path::new(&job.source_path)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| job.file_name.clone());
                    register_asset_and_complete(
                        database,
                        &job,
                        &section,
                        &target_relative,
                        &display_name,
                        &original_name,
                        actual_size,
                    )?;
                    report.completed_jobs += 1;
                    report.messages.push(format!(
                        "「{}」在中断前已完成复制，已恢复为完成状态",
                        job.file_name
                    ));
                    continue;
                }
            }
        }

        mark_interrupted(database, &job, "上次导入被中断，目标文件不完整或缺失")?;
        report.failed_jobs += 1;
        report
            .messages
            .push(format!("「{}」导入中断，可重试", job.file_name));
    }

    Ok(report)
}

fn mark_interrupted(database: &Database, job: &ImportJob, reason: &str) -> AppResult<()> {
    database.with_conn(|connection| {
        import_jobs::update_status(
            connection,
            &job.id,
            &import_jobs::JobStatusUpdate {
                status: ImportStatus::Failed,
                bytes_copied: None,
                error_code: Some("interrupted"),
                error_message: Some(reason),
                asset_id: None,
                completed_at: None,
                updated_at: &now_iso(),
            },
        )?;
        Ok(())
    })?;
    Ok(())
}
