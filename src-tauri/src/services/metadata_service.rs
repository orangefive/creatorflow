use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::domain::asset::{AssetStatus, MetadataStatus, ThumbnailStatus};
use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem;
use crate::repositories::{assets, works};
use crate::services::media_service::{MediaService, ProbeResult};

pub const ASSET_METADATA_EVENT: &str = "asset://metadata";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMetadataEvent {
    pub asset_id: String,
    pub work_id: String,
    pub metadata_status: MetadataStatus,
    pub thumbnail_status: ThumbnailStatus,
}

/// 元数据提取服务：单后台线程按顺序处理，避免解码图片或调用 ffmpeg 抢占 CPU。
pub struct MetadataService {
    app: Option<AppHandle>,
    database: Arc<Database>,
    media: Arc<MediaService>,
    sender: Mutex<Option<Sender<String>>>,
    started: AtomicBool,
}

impl MetadataService {
    pub fn new(app: Option<AppHandle>, database: Arc<Database>, cache_root: PathBuf) -> Self {
        Self {
            app,
            database,
            media: Arc::new(MediaService::new(cache_root)),
            sender: Mutex::new(None),
            started: AtomicBool::new(false),
        }
    }

    pub fn ffmpeg_available(&self) -> bool {
        self.media.ffmpeg_available()
    }

    fn ensure_worker(&self) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let (sender, receiver) = mpsc::channel::<String>();
        match self.sender.lock() {
            Ok(mut slot) => *slot = Some(sender),
            Err(_) => {
                tracing::error!("元数据队列不可用");
                return;
            }
        }

        let app = self.app.clone();
        let database = Arc::clone(&self.database);
        let media = Arc::clone(&self.media);
        std::thread::spawn(move || {
            while let Ok(asset_id) = receiver.recv() {
                if let Err(error) = process_asset(&app, &database, &media, &asset_id) {
                    tracing::warn!(
                        asset_id = %asset_id,
                        message = %error.message,
                        "提取素材元数据失败"
                    );
                }
            }
        });
    }

    pub fn enqueue(&self, asset_id: String) {
        self.ensure_worker();
        match self.sender.lock() {
            Ok(slot) => {
                if let Some(sender) = slot.as_ref() {
                    if let Err(error) = sender.send(asset_id.clone()) {
                        tracing::warn!(asset_id = %asset_id, error = %error, "无法提交元数据任务");
                    }
                }
            }
            Err(_) => tracing::error!("元数据队列锁不可用"),
        }
    }

    /// 启动时补齐第一阶段素材（状态为 pending 的历史记录）。
    pub fn backfill(&self, limit: i64) {
        let ids = self
            .database
            .with_conn(|connection| assets::list_pending_metadata(connection, limit));
        match ids {
            Ok(ids) => {
                if !ids.is_empty() {
                    tracing::info!(count = ids.len(), "开始补齐历史素材元数据");
                }
                for id in ids {
                    self.enqueue(id);
                }
            }
            Err(error) => tracing::warn!(message = %error.message, "查询待处理素材失败"),
        }
    }

    /// 同步提取（命令重试与测试使用）。
    pub fn process_now(&self, asset_id: &str) -> AppResult<()> {
        process_asset(&self.app, &self.database, &self.media, asset_id)
    }
}

fn process_asset(
    app: &Option<AppHandle>,
    database: &Database,
    media: &MediaService,
    asset_id: &str,
) -> AppResult<()> {
    let asset = database.with_conn(|connection| {
        assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))
    })?;
    if asset.status != AssetStatus::Ready {
        return Ok(());
    }

    let work = database
        .with_conn(|connection| works::find(connection, &asset.work_id))?
        .ok_or_else(|| AppError::not_found("素材所属作品不存在"))?;
    let root = PathBuf::from(&work.root_path);

    let result = match filesystem::resolve_within(&root, &asset.relative_path) {
        Ok(path) => media.probe(&asset.id, &asset.work_id, &path, asset.media_kind),
        Err(_) => ProbeResult::failed("文件路径无效或已缺失".to_string()),
    };

    let now = now_iso();
    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启元数据事务", error))?;
        assets::update_metadata(
            &transaction,
            &asset.id,
            &assets::MetadataUpdate {
                width: result.width,
                height: result.height,
                duration_ms: result.duration_ms,
                frame_rate: result.frame_rate,
                sample_rate: result.sample_rate,
                channels: result.channels,
                codec: result.codec.clone(),
                status: result.status,
                error: result.error.clone(),
                updated_at: now.clone(),
            },
        )?;
        assets::update_thumbnail(
            &transaction,
            &asset.id,
            result.thumbnail.status,
            result.thumbnail.relative_path.as_deref(),
            result.thumbnail.error.as_deref(),
            &now,
        )?;
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交元数据事务失败", error))?;
        Ok::<(), AppError>(())
    })?;

    if let Some(app) = app {
        let event = AssetMetadataEvent {
            asset_id: asset.id.clone(),
            work_id: asset.work_id.clone(),
            metadata_status: result.status,
            thumbnail_status: result.thumbnail.status,
        };
        if let Err(error) = app.emit(ASSET_METADATA_EVENT, event) {
            tracing::warn!(error = %error, "发送素材元数据事件失败");
        }
    }

    tracing::debug!(
        asset_id = %asset.id,
        kind = asset.media_kind.as_str(),
        status = result.status.as_str(),
        thumbnail = result.thumbnail.status.as_str(),
        "素材元数据处理完成"
    );
    Ok(())
}
