//! 第二阶段核心能力集成测试：
//! 迁移兼容、有界并发队列、搜索筛选排序、标签、元数据/缩略图、缺失素材重新定位。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use creatorflow_lib::domain::asset::{AssetStatus, MetadataStatus, ThumbnailStatus};
use creatorflow_lib::domain::import::ImportStatus;
use creatorflow_lib::domain::query::{AssetQuery, AssetSortKey, SortDirection};
use creatorflow_lib::domain::section::SectionKey;
use creatorflow_lib::domain::work::Work;
use creatorflow_lib::errors::ErrorCode;
use creatorflow_lib::infrastructure::database::Database;
use creatorflow_lib::repositories::{assets, import_jobs};
use creatorflow_lib::services::import_service::ImportManager;
use creatorflow_lib::services::metadata_service::MetadataService;
use creatorflow_lib::services::work_service::{self, CreateWorkInput};
use creatorflow_lib::services::{asset_service, settings_service, tag_service};
use tempfile::TempDir;

struct TestEnv {
    _temp: TempDir,
    root: PathBuf,
    cache: PathBuf,
    database: Arc<Database>,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().expect("创建临时目录失败");
        let root = temp.path().join("works");
        fs::create_dir_all(&root).expect("创建作品根目录失败");
        let cache = temp.path().join("thumbnails");
        fs::create_dir_all(&cache).expect("创建缓存目录失败");
        let database =
            Arc::new(Database::open(temp.path().join("creatorflow.db")).expect("打开数据库失败"));
        settings_service::set_workspace_root(&database, root.to_string_lossy().as_ref())
            .expect("设置根目录失败");
        Self {
            _temp: temp,
            root,
            cache,
            database,
        }
    }

    fn create_work(&self, name: &str) -> Work {
        work_service::create_work(
            &self.database,
            CreateWorkInput {
                name: name.to_string(),
                description: None,
                root_directory: None,
                template_key: Some("video-basic".to_string()),
            },
        )
        .expect("创建作品失败")
    }

    fn manager(&self) -> Arc<ImportManager> {
        Arc::new(ImportManager::headless(Arc::clone(&self.database)))
    }

    fn source_dir(&self) -> PathBuf {
        let directory = self._temp.path().join("sources");
        fs::create_dir_all(&directory).expect("创建来源目录失败");
        directory
    }

    fn write_source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.source_dir().join(name);
        fs::write(&path, bytes).expect("写入来源文件失败");
        path
    }

    fn import(&self, work: &Work, section: &str, files: &[PathBuf]) {
        let paths: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let jobs = self
            .manager()
            .start_inline(&work.id, section, &paths)
            .expect("导入失败");
        assert!(
            jobs.iter().all(|job| job.status == ImportStatus::Completed),
            "测试准备阶段应全部导入成功"
        );
        let _ = paths;
    }

    fn search(&self, query: AssetQuery) -> creatorflow_lib::domain::query::AssetPage {
        asset_service::search(&self.database, &query, &self.cache).expect("查询素材失败")
    }

    fn base_query(&self, work_id: &str) -> AssetQuery {
        AssetQuery {
            work_id: Some(work_id.to_string()),
            section_key: None,
            media_kind: None,
            status: None,
            tag_id: None,
            keyword: None,
            sort_key: None,
            sort_direction: None,
            limit: None,
            offset: None,
        }
    }
}

fn total_assets(database: &Database, work_id: &str) -> i64 {
    database
        .with_conn(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM assets WHERE work_id = ?1 AND deleted_at IS NULL",
                    [work_id],
                    |row| row.get(0),
                )
                .map_err(creatorflow_lib::errors::AppError::from)
        })
        .unwrap()
}

#[test]
fn migrates_phase_one_database_without_data_loss() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("legacy.db");

    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO schema_migrations (version, name, applied_at)
                    VALUES (1, '0001_initial', '2026-01-01T00:00:00.000Z');
                 INSERT INTO works (id, name, status, root_path, created_at, updated_at)
                    VALUES ('w-legacy', '旧作品', 'idea', 'C:\\Legacy', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
                 INSERT INTO sections (id, work_id, key, name, relative_path, sort_order, created_at)
                    VALUES ('s-legacy', 'w-legacy', 'documents', '文档', '01_文档', 1, '2026-01-01T00:00:00.000Z');
                 INSERT INTO assets (id, work_id, section_id, display_name, original_name, relative_path,
                        media_kind, size_bytes, status, created_at, updated_at)
                    VALUES ('a-legacy', 'w-legacy', 's-legacy', '旧素材.md', '旧素材.md', '01_文档/旧素材.md',
                        'document', 12, 'ready', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');",
            )
            .unwrap();
    }

    let database = Database::open(&path).expect("升级旧数据库失败");
    let asset = database
        .with_conn(|connection| assets::find_by_id(connection, "a-legacy"))
        .unwrap()
        .expect("旧素材应保留");

    assert_eq!(asset.display_name, "旧素材.md");
    assert_eq!(asset.size_bytes, 12);
    assert_eq!(asset.status, AssetStatus::Ready);
    assert_eq!(asset.metadata_status, MetadataStatus::Pending);
    assert_eq!(asset.thumbnail_status, ThumbnailStatus::Pending);
    assert_eq!(asset.tags.len(), 0);

    let applied: Vec<i64> = database
        .with_conn(|connection| {
            let mut statement = connection
                .prepare("SELECT version FROM schema_migrations ORDER BY version")
                .map_err(creatorflow_lib::errors::AppError::from)?;
            let rows = statement
                .query_map([], |row| row.get(0))
                .map_err(creatorflow_lib::errors::AppError::from)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(creatorflow_lib::errors::AppError::from)
        })
        .unwrap();
    assert_eq!(applied, vec![1, 2, 3, 4]);

    let tag_count: i64 = database
        .with_conn(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
                .map_err(creatorflow_lib::errors::AppError::from)
        })
        .unwrap();
    assert_eq!(tag_count, 0);
}

#[test]
fn bounded_queue_never_exceeds_concurrency_limit() {
    let env = TestEnv::new();
    let work = env.create_work("并发测试");
    settings_service::set_import_concurrency(&env.database, 2).expect("设置并发失败");

    let files: Vec<PathBuf> = (0..6)
        .map(|index| {
            let bytes = vec![(index + 1) as u8; 2 * 1024 * 1024];
            env.write_source(&format!("素材{index}.bin"), &bytes)
        })
        .collect();

    let manager = env.manager();
    let paths: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let jobs = manager
        .start(&work.id, "documents", &paths)
        .expect("创建任务失败");
    assert_eq!(jobs.len(), 6);
    assert!(
        manager.wait_for_idle(Duration::from_secs(30)),
        "导入队列未在超时内完成"
    );

    assert_eq!(manager.concurrency(), 2);
    assert!(
        manager.peak_concurrency() <= 2,
        "并发峰值 {} 超过上限 2",
        manager.peak_concurrency()
    );
    assert!(manager.peak_concurrency() >= 1);

    let stored = env
        .database
        .with_conn(|connection| import_jobs::list(connection, Some(&work.id)))
        .unwrap();
    assert_eq!(stored.len(), 6);
    assert!(stored
        .iter()
        .all(|job| job.status == ImportStatus::Completed));
    assert_eq!(total_assets(&env.database, &work.id), 6);
}

#[test]
fn imports_empty_file() {
    let env = TestEnv::new();
    let work = env.create_work("空文件");
    let source = env.write_source("空.txt", &[]);
    env.import(&work, "documents", std::slice::from_ref(&source));

    let page = env.search(env.base_query(&work.id));
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].size_bytes, 0);
    assert_eq!(page.items[0].status, AssetStatus::Ready);
}

#[test]
fn cancelling_running_job_leaves_no_asset_or_temp_file() {
    let env = TestEnv::new();
    let work = env.create_work("取消测试");
    settings_service::set_import_concurrency(&env.database, 1).unwrap();

    let payload = vec![9_u8; 256 * 1024 * 1024];
    let source = env.write_source("大文件.bin", &payload);

    let manager = env.manager();
    let jobs = manager
        .start(
            &work.id,
            "documents",
            &[source.to_string_lossy().to_string()],
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(20));
    manager.cancel(&jobs[0].id).expect("取消失败");
    assert!(manager.wait_for_idle(Duration::from_secs(60)));

    let stored = env
        .database
        .with_conn(|connection| import_jobs::find_by_id(connection, &jobs[0].id))
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, ImportStatus::Cancelled);
    assert!(stored.asset_id.is_none());
    assert_eq!(total_assets(&env.database, &work.id), 0);

    let target_dir = env.root.join("取消测试").join("01_文档");
    let leftovers: Vec<_> = fs::read_dir(&target_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "取消后不应残留临时文件");
    assert!(source.exists(), "源文件必须保留");
}

#[test]
fn failed_job_can_be_retried_without_duplicate_assets() {
    let env = TestEnv::new();
    let work = env.create_work("重试测试");
    let source = env.source_dir().join("稍后出现.md");
    let _ = fs::remove_file(&source);

    let manager = env.manager();
    let jobs = manager
        .start(
            &work.id,
            "documents",
            &[source.to_string_lossy().to_string()],
        )
        .unwrap();
    assert!(manager.wait_for_idle(Duration::from_secs(20)));
    let failed = env
        .database
        .with_conn(|connection| import_jobs::find_by_id(connection, &jobs[0].id))
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, ImportStatus::Failed);

    fs::write(&source, b"# recovered").unwrap();
    manager.retry(&jobs[0].id).expect("重试失败");
    assert!(manager.wait_for_idle(Duration::from_secs(20)));

    let completed = env
        .database
        .with_conn(|connection| import_jobs::find_by_id(connection, &jobs[0].id))
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, ImportStatus::Completed);
    assert_eq!(total_assets(&env.database, &work.id), 1);
}

#[test]
fn search_filters_sorts_and_paginates_in_database() {
    let env = TestEnv::new();
    let work = env.create_work("搜索测试");
    let script = env.write_source("脚本_v1.md", b"script");
    let audio = env.write_source("配乐.mp3", &[1_u8; 4096]);
    let image = env.write_source("封面.png", &[2_u8; 2048]);
    let video = env.write_source("片段.mp4", &[3_u8; 8192]);
    env.import(
        &work,
        "documents",
        &[script.clone(), audio.clone(), image.clone(), video.clone()],
    );

    let keyword = env.search(AssetQuery {
        keyword: Some("脚本".to_string()),
        ..env.base_query(&work.id)
    });
    assert_eq!(keyword.total, 1);
    assert!(keyword.items[0].display_name.contains("脚本"));

    let audio_only = env.search(AssetQuery {
        media_kind: Some(creatorflow_lib::domain::asset::MediaKind::Audio),
        ..env.base_query(&work.id)
    });
    assert_eq!(audio_only.total, 1);

    let sorted = env.search(AssetQuery {
        sort_key: Some(AssetSortKey::Size),
        sort_direction: Some(SortDirection::Asc),
        ..env.base_query(&work.id)
    });
    let sizes: Vec<i64> = sorted.items.iter().map(|asset| asset.size_bytes).collect();
    let mut expected = sizes.clone();
    expected.sort();
    assert_eq!(sizes, expected);

    let page = env.search(AssetQuery {
        limit: Some(2),
        offset: Some(2),
        sort_key: Some(AssetSortKey::Name),
        ..env.base_query(&work.id)
    });
    assert_eq!(page.total, 4);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.limit, 2);
    assert_eq!(page.offset, 2);

    let png_asset = env
        .search(AssetQuery {
            keyword: Some("封面".to_string()),
            ..env.base_query(&work.id)
        })
        .items
        .remove(0);
    env.database
        .with_conn(|connection| {
            assets::update_status(
                connection,
                &png_asset.id,
                AssetStatus::Missing,
                "2026-01-01T00:00:00.000Z",
            )
        })
        .unwrap();
    let missing = env.search(AssetQuery {
        status: Some(AssetStatus::Missing),
        ..env.base_query(&work.id)
    });
    assert_eq!(missing.total, 1);
}

#[test]
fn tags_are_reused_and_filterable() {
    let env = TestEnv::new();
    let work = env.create_work("标签测试");
    let first = env.write_source("a.md", b"a");
    let second = env.write_source("b.md", b"b");
    env.import(&work, "documents", &[first, second]);

    let page = env.search(env.base_query(&work.id));
    assert_eq!(page.total, 2);

    let tags = tag_service::add_tag(&env.database, &page.items[0].id, "重点").unwrap();
    assert_eq!(tags.len(), 1);
    let tag_service_tags = tag_service::add_tag(&env.database, &page.items[1].id, "重点").unwrap();
    assert_eq!(tag_service_tags.len(), 1);

    let work_tags = tag_service::list_work_tags(&env.database, &work.id).unwrap();
    assert_eq!(work_tags.len(), 1, "同名标签应复用同一条记录");
    assert_eq!(work_tags[0].asset_count, 2);

    let filtered = env.search(AssetQuery {
        tag_id: Some(work_tags[0].id.clone()),
        ..env.base_query(&work.id)
    });
    assert_eq!(filtered.total, 2);

    tag_service::remove_tag(&env.database, &page.items[0].id, &work_tags[0].id).unwrap();
    let after_removal = tag_service::list_work_tags(&env.database, &work.id).unwrap();
    assert_eq!(after_removal[0].asset_count, 1);
}

#[test]
fn extracts_image_metadata_and_thumbnail() {
    let env = TestEnv::new();
    let work = env.create_work("元数据测试");
    let image_path = env.source_dir().join("封面.png");
    let image = image::RgbImage::from_fn(80, 40, |x, y| {
        image::Rgb([(x % 255) as u8, (y % 255) as u8, 120])
    });
    image.save(&image_path).unwrap();

    env.import(&work, "images", &[image_path]);
    let page = env.search(env.base_query(&work.id));
    let asset = &page.items[0];
    assert_eq!(asset.metadata_status, MetadataStatus::Pending);

    let metadata = MetadataService::new(None, Arc::clone(&env.database), env.cache.clone());
    metadata.process_now(&asset.id).expect("提取元数据失败");

    let refreshed = env
        .database
        .with_conn(|connection| assets::find_by_id(connection, &asset.id))
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.metadata_status, MetadataStatus::Ready);
    assert_eq!(refreshed.width, Some(80));
    assert_eq!(refreshed.height, Some(40));
    assert_eq!(refreshed.thumbnail_status, ThumbnailStatus::Ready);
    let relative = refreshed.thumbnail_relative_path.clone().unwrap();
    assert!(env.cache.join(relative).is_file(), "缩略图应写入缓存目录");

    let resolved = env.search(env.base_query(&work.id));
    assert!(resolved.items[0].thumbnail_path.is_some());
}

#[test]
fn extracts_wav_duration_and_sample_rate() {
    let env = TestEnv::new();
    let work = env.create_work("音频测试");

    let sample_rate = 44_100_u32;
    let samples = 8_820_usize; // 0.2 秒
    let data_size = (samples * 2) as u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend(std::iter::repeat_n(0_u8, data_size as usize));

    let source = env.write_source("测试音.wav", &wav);
    env.import(&work, "audio", &[source]);

    let page = env.search(env.base_query(&work.id));
    let asset = &page.items[0];
    let metadata = MetadataService::new(None, Arc::clone(&env.database), env.cache.clone());
    metadata.process_now(&asset.id).expect("提取音频元数据失败");

    let refreshed = env
        .database
        .with_conn(|connection| assets::find_by_id(connection, &asset.id))
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.metadata_status, MetadataStatus::Ready);
    assert_eq!(refreshed.sample_rate, Some(sample_rate as i64));
    assert_eq!(refreshed.channels, Some(1));
    let duration = refreshed.duration_ms.unwrap_or_default();
    assert!((150..=260).contains(&duration), "时长异常：{duration}ms");
}

#[test]
fn extracts_mp4_metadata_and_cover_when_ffmpeg_available() {
    let env = TestEnv::new();
    let service = creatorflow_lib::services::media_service::MediaService::new(env.cache.clone());
    let Some(ffmpeg) = service.ffmpeg_path().map(|path| path.to_path_buf()) else {
        eprintln!("未检测到 ffmpeg，跳过视频用例");
        return;
    };

    let video = env.source_dir().join("测试片段.mp4");
    let status = std::process::Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=25",
            "-t",
            "2",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&video)
        .status()
        .expect("调用 ffmpeg 生成测试视频失败");
    assert!(status.success(), "生成测试视频失败");

    let result = service.probe(
        "video-asset",
        "work-video",
        &video,
        creatorflow_lib::domain::asset::MediaKind::Video,
    );
    assert_eq!(result.status, MetadataStatus::Ready);
    assert_eq!(result.width, Some(320));
    assert_eq!(result.height, Some(240));
    assert!(result.duration_ms.unwrap_or_default() >= 1_900);
    assert_eq!(result.thumbnail.status, ThumbnailStatus::Ready);
    let relative = result.thumbnail.relative_path.expect("应生成视频封面");
    assert!(env.cache.join(relative).is_file());
}

#[test]
fn corrupt_image_is_metadata_failed_but_asset_stays_usable() {
    let env = TestEnv::new();
    let work = env.create_work("损坏图片");
    let source = env.write_source("坏图.png", b"this is not a png");
    env.import(&work, "images", &[source]);

    let page = env.search(env.base_query(&work.id));
    let asset = &page.items[0];
    let metadata = MetadataService::new(None, Arc::clone(&env.database), env.cache.clone());
    metadata.process_now(&asset.id).expect("不应抛出错误");

    let refreshed = env
        .database
        .with_conn(|connection| assets::find_by_id(connection, &asset.id))
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.metadata_status, MetadataStatus::Failed);
    assert!(refreshed.metadata_error.is_some());
    assert_eq!(refreshed.status, AssetStatus::Ready);

    let listed = env.search(env.base_query(&work.id));
    assert_eq!(listed.total, 1);
}

#[test]
fn relocation_requires_confirmation_when_content_differs() {
    let env = TestEnv::new();
    let work = env.create_work("重新定位");
    let source = env.write_source("脚本.md", b"original-content");
    env.import(&work, "documents", &[source]);

    let page = env.search(env.base_query(&work.id));
    let asset = &page.items[0];
    let original_relative = asset.relative_path.clone();
    assert!(asset.sha256.is_some(), "导入应记录内容哈希");

    let target = env.root.join("重新定位").join("01_文档").join("脚本.md");
    fs::remove_file(&target).unwrap();
    // 触发一次列表查询，让状态按磁盘实际情况校准为 missing
    let _ = env.search(env.base_query(&work.id));

    let moved = env
        .root
        .join("重新定位")
        .join("01_文档")
        .join("脚本-改名.md");
    fs::write(&moved, b"different-content!!").unwrap();

    let preview = asset_service::relocate_asset(
        &env.database,
        &asset.id,
        moved.to_string_lossy().as_ref(),
        false,
    )
    .unwrap();
    assert!(!preview.applied);
    assert!(preview.requires_confirmation);
    assert!(!preview.size_matches);

    let unchanged = env
        .database
        .with_conn(|connection| assets::find_by_id(connection, &asset.id))
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.relative_path, original_relative);
    assert_eq!(unchanged.status, AssetStatus::Missing);

    let applied = asset_service::relocate_asset(
        &env.database,
        &asset.id,
        moved.to_string_lossy().as_ref(),
        true,
    )
    .unwrap();
    assert!(applied.applied);
    assert_eq!(applied.relative_path, "01_文档/脚本-改名.md");

    let refreshed = env
        .database
        .with_conn(|connection| assets::find_by_id(connection, &asset.id))
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.status, AssetStatus::Ready);
    assert_eq!(refreshed.size_bytes, b"different-content!!".len() as i64);
    assert_eq!(refreshed.metadata_status, MetadataStatus::Pending);
}

#[test]
fn relocation_accepts_matching_content_without_confirmation() {
    let env = TestEnv::new();
    let work = env.create_work("哈希匹配");
    let source = env.write_source("封面.png", b"same-bytes");
    env.import(&work, "images", &[source]);

    let page = env.search(env.base_query(&work.id));
    let asset = &page.items[0];
    let target = env.root.join("哈希匹配").join("04_图片").join("封面.png");
    let moved = env
        .root
        .join("哈希匹配")
        .join("04_图片")
        .join("封面-备份.png");
    fs::copy(&target, &moved).unwrap();
    fs::remove_file(&target).unwrap();

    let result = asset_service::relocate_asset(
        &env.database,
        &asset.id,
        moved.to_string_lossy().as_ref(),
        false,
    )
    .unwrap();
    assert!(result.applied, "内容一致时无需确认");
    assert_eq!(result.hash_matches, Some(true));
}

#[test]
fn relocation_rejects_files_outside_work_directory() {
    let env = TestEnv::new();
    let work = env.create_work("外部路径");
    let source = env.write_source("内部.md", b"inside");
    env.import(&work, "documents", &[source]);

    let page = env.search(env.base_query(&work.id));
    let asset = &page.items[0];
    let outside = env._temp.path().join("outside.md");
    fs::write(&outside, b"outside").unwrap();

    let error = asset_service::relocate_asset(
        &env.database,
        &asset.id,
        outside.to_string_lossy().as_ref(),
        true,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert!(error.message.contains("作品目录内"));
}

#[test]
fn section_and_media_filters_combine() {
    let env = TestEnv::new();
    let work = env.create_work("组合筛选");
    let doc = env.write_source("说明.txt", b"doc");
    let image = env.write_source("图.png", &[1_u8; 64]);
    env.import(&work, "documents", &[doc]);
    env.import(&work, "images", &[image]);

    let by_section = env.search(AssetQuery {
        section_key: Some(SectionKey::Images),
        ..env.base_query(&work.id)
    });
    assert_eq!(by_section.total, 1);
    assert_eq!(
        by_section.items[0].media_kind,
        creatorflow_lib::domain::asset::MediaKind::Image
    );

    let combined = env.search(AssetQuery {
        section_key: Some(SectionKey::Images),
        media_kind: Some(creatorflow_lib::domain::asset::MediaKind::Document),
        ..env.base_query(&work.id)
    });
    assert_eq!(combined.total, 0);
}

#[test]
fn archiving_remembers_previous_status_and_can_be_restored() {
    let env = TestEnv::new();
    let work = env.create_work("归档恢复");
    work_service::update_status(&env.database, &work.id, "in_production").expect("设置状态失败");

    let archived = work_service::archive_work(&env.database, &work.id).expect("归档失败");
    assert_eq!(archived.status.as_str(), "archived");
    assert!(archived.archived_at.is_some());
    assert_eq!(
        archived.archived_from_status.as_deref(),
        Some("in_production")
    );

    assert!(
        work_service::list_works(&env.database, false)
            .unwrap()
            .is_empty(),
        "归档后不应出现在默认列表"
    );
    assert_eq!(
        work_service::list_works(&env.database, true).unwrap().len(),
        1,
        "包含归档时仍应可见"
    );

    let restored = work_service::unarchive_work(&env.database, &work.id).expect("取消归档失败");
    assert_eq!(restored.status.as_str(), "in_production");
    assert!(restored.archived_at.is_none());
    assert!(restored.archived_from_status.is_none());

    let error =
        work_service::unarchive_work(&env.database, &work.id).expect_err("未归档时不应成功");
    assert_eq!(error.code, ErrorCode::InvalidInput);
}

#[test]
fn unarchive_falls_back_to_idea_when_previous_status_is_unknown() {
    let env = TestEnv::new();
    let work = env.create_work("旧数据");
    work_service::update_status(&env.database, &work.id, "review").unwrap();
    work_service::archive_work(&env.database, &work.id).unwrap();

    // 模拟 0.2.0 之前归档的作品：没有记录归档前状态
    env.database
        .with_conn(|connection| {
            connection
                .execute(
                    "UPDATE works SET archived_from_status = NULL WHERE id = ?1",
                    [&work.id],
                )
                .map_err(creatorflow_lib::errors::AppError::from)?;
            Ok(())
        })
        .unwrap();

    let restored = work_service::unarchive_work(&env.database, &work.id).unwrap();
    assert_eq!(restored.status.as_str(), "idea");
}

#[test]
fn deleting_work_recycles_directory_and_hides_it() {
    let env = TestEnv::new();
    let work = env.create_work("待删除作品");
    let source = env.write_source("草稿.md", b"# draft");
    env.import(&work, "documents", &[source]);

    let directory = PathBuf::from(&work.root_path);
    assert!(directory.is_dir());
    assert_eq!(total_assets(&env.database, &work.id), 1);

    work_service::delete_work(&env.database, &work.id, &env.cache).expect("删除作品失败");

    assert!(!directory.exists(), "作品目录应已移入回收站");
    assert!(
        work_service::list_works(&env.database, true)
            .unwrap()
            .is_empty(),
        "删除后不应出现在任何列表"
    );
    let error = work_service::get_work(&env.database, &work.id).expect_err("记录应不可见");
    assert_eq!(error.code, ErrorCode::NotFound);
    assert_eq!(total_assets(&env.database, &work.id), 0);
    // 作品已不可见，按作品查询素材应直接报「不存在」
    let error = asset_service::search(&env.database, &env.base_query(&work.id), &env.cache)
        .expect_err("作品删除后不应还能查询素材");
    assert_eq!(error.code, ErrorCode::NotFound);
}

#[test]
fn deleting_asset_recycles_file_and_removes_it_from_lists() {
    let env = TestEnv::new();
    let work = env.create_work("单文件删除");
    let source = env.write_source("可删除.txt", b"bye");
    env.import(&work, "documents", &[source]);

    let page = env.search(env.base_query(&work.id));
    assert_eq!(page.total, 1);
    let asset = page.items.into_iter().next().unwrap();
    let path = PathBuf::from(&asset.absolute_path);
    assert!(path.is_file());

    asset_service::delete_asset(&env.database, &asset.id, &env.cache).expect("删除素材失败");

    assert!(!path.exists(), "素材文件应已移入回收站");
    assert_eq!(env.search(env.base_query(&work.id)).total, 0);
    assert_eq!(total_assets(&env.database, &work.id), 0);
    assert!(
        asset_service::get_asset(&env.database, &asset.id).is_err(),
        "记录应不可见"
    );

    // 分区计数应同步下降
    let counts = work_service::get_work(&env.database, &work.id).unwrap();
    assert_eq!(counts.asset_count, 0);
}

#[test]
fn deleting_missing_asset_still_removes_the_record() {
    let env = TestEnv::new();
    let work = env.create_work("缺失记录清理");
    let source = env.write_source("已消失.txt", b"gone");
    env.import(&work, "documents", &[source]);

    let asset = env
        .search(env.base_query(&work.id))
        .items
        .into_iter()
        .next()
        .unwrap();
    fs::remove_file(&asset.absolute_path).expect("模拟文件丢失");

    asset_service::delete_asset(&env.database, &asset.id, &env.cache)
        .expect("文件已不在时也应能清理记录");
    assert_eq!(env.search(env.base_query(&work.id)).total, 0);
}
