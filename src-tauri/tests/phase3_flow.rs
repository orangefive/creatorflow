//! 第三阶段集成测试：
//! 跨作品检索、批量操作、缓存与备份的端到端行为。
//!
//! 性能测量用例标了 `#[ignore]`，需要显式运行：
//! `cargo test --test phase3_flow -- --ignored --nocapture`

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use creatorflow_lib::domain::asset::{AssetStatus, MediaKind};
use creatorflow_lib::domain::import::ImportStatus;
use creatorflow_lib::domain::query::{AssetQuery, AssetSortKey, SortDirection};
use creatorflow_lib::domain::section::SectionKey;
use creatorflow_lib::domain::work::Work;
use creatorflow_lib::infrastructure::database::Database;
use creatorflow_lib::repositories::{assets, sections};
use creatorflow_lib::services::import_service::ImportManager;
use creatorflow_lib::services::work_service::{self, CreateWorkInput};
use creatorflow_lib::services::{asset_service, cache_service, settings_service, tag_service};
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
                template_key: None,
            },
        )
        .expect("创建作品失败")
    }

    fn write_source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let directory = self._temp.path().join("sources");
        fs::create_dir_all(&directory).expect("创建来源目录失败");
        let path = directory.join(name);
        fs::write(&path, bytes).expect("写入来源文件失败");
        path
    }

    fn import(&self, work: &Work, section: &str, files: &[PathBuf]) {
        let paths: Vec<String> = files
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        let manager = Arc::new(ImportManager::headless(Arc::clone(&self.database)));
        let jobs = manager
            .start_inline(&work.id, section, &paths)
            .expect("导入失败");
        assert!(
            jobs.iter().all(|job| job.status == ImportStatus::Completed),
            "测试准备阶段应全部导入成功"
        );
    }

    fn search(&self, query: AssetQuery) -> creatorflow_lib::domain::query::AssetPage {
        asset_service::search(&self.database, &query, &self.cache).expect("查询素材失败")
    }

    fn global_query(&self) -> AssetQuery {
        AssetQuery {
            work_id: None,
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
fn global_search_returns_assets_from_multiple_works() {
    let env = TestEnv::new();
    let first = env.create_work("第一期");
    let second = env.create_work("第二期");
    env.import(&first, "documents", &[env.write_source("脚本A.md", b"a")]);
    env.import(&second, "audio", &[env.write_source("配乐B.wav", b"b")]);

    assert!(env.root.join(&first.name).is_dir(), "作品目录应真实存在");

    let page = env.search(env.global_query());
    assert_eq!(page.total, 2);
    assert_eq!(page.items.len(), 2);

    let names: Vec<&str> = page
        .items
        .iter()
        .filter_map(|asset| asset.work_name.as_deref())
        .collect();
    assert!(names.contains(&"第一期") && names.contains(&"第二期"));

    for asset in &page.items {
        assert!(asset.absolute_path.ends_with(&asset.display_name));
        assert!(asset.section_key.is_some());
    }

    // 按作品筛选后只剩该作品
    let mut query = env.global_query();
    query.work_id = Some(first.id.clone());
    let filtered = env.search(query);
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.items[0].work_name.as_deref(), Some("第一期"));
}

#[test]
fn global_search_matches_work_name_and_section() {
    let env = TestEnv::new();
    let work = env.create_work("春日短片");
    let other = env.create_work("无关项目");
    env.import(&work, "video", &[env.write_source("clip.mp4", b"x")]);
    env.import(&other, "documents", &[env.write_source("笔记.md", b"y")]);

    let mut query = env.global_query();
    query.keyword = Some("春日".to_string());
    let page = env.search(query);
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].work_name.as_deref(), Some("春日短片"));

    let mut by_section = env.global_query();
    by_section.section_key = Some(SectionKey::Documents);
    let page = env.search(by_section);
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].display_name, "笔记.md");
}

#[test]
fn global_search_tag_filter_does_not_leak_between_works() {
    let env = TestEnv::new();
    let first = env.create_work("甲");
    let second = env.create_work("乙");
    env.import(&first, "documents", &[env.write_source("甲文件.md", b"1")]);
    env.import(&second, "documents", &[env.write_source("乙文件.md", b"2")]);

    let target = env
        .search(env.global_query())
        .items
        .into_iter()
        .find(|asset| asset.work_id == first.id)
        .expect("找不到甲作品素材");
    let tags = tag_service::add_tag(&env.database, &target.id, "重点").expect("添加标签失败");
    let tag_id = tags[0].id.clone();

    let mut query = env.global_query();
    query.tag_id = Some(tag_id);
    let page = env.search(query);
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].work_id, first.id);

    // 另一个作品的素材没有该标签
    let other_assets = env.search(AssetQuery {
        work_id: Some(second.id.clone()),
        ..env.global_query()
    });
    assert_eq!(other_assets.total, 1);
    assert!(other_assets.items[0].tags.is_empty());
}

#[test]
fn global_search_pagination_total_counts_all_works() {
    let env = TestEnv::new();
    let first = env.create_work("作品一");
    let second = env.create_work("作品二");
    env.import(
        &first,
        "documents",
        &[
            env.write_source("一甲.md", b"1"),
            env.write_source("一乙.md", b"2"),
        ],
    );
    env.import(&second, "documents", &[env.write_source("二甲.md", b"3")]);

    let mut query = env.global_query();
    query.limit = Some(2);
    query.offset = Some(0);
    query.sort_key = Some(AssetSortKey::Name);
    query.sort_direction = Some(SortDirection::Asc);
    let page = env.search(query);
    assert_eq!(page.total, 3, "总数应跨作品统计");
    assert_eq!(page.items.len(), 2);

    let mut second_page = env.global_query();
    second_page.limit = Some(2);
    second_page.offset = Some(2);
    second_page.sort_key = Some(AssetSortKey::Name);
    second_page.sort_direction = Some(SortDirection::Asc);
    assert_eq!(env.search(second_page).items.len(), 1);
}

#[test]
fn global_search_marks_missing_files() {
    let env = TestEnv::new();
    let work = env.create_work("缺失检查");
    env.import(&work, "documents", &[env.write_source("会消失.md", b"x")]);

    let asset = env
        .search(env.global_query())
        .items
        .into_iter()
        .next()
        .unwrap();
    fs::remove_file(&asset.absolute_path).expect("模拟文件丢失");

    let page = env.search(env.global_query());
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].status, AssetStatus::Missing);
}

#[test]
fn batch_delete_removes_files_and_reports_failures() {
    let env = TestEnv::new();
    let work = env.create_work("批量删除");
    env.import(
        &work,
        "documents",
        &[
            env.write_source("批一.md", b"1"),
            env.write_source("批二.md", b"2"),
        ],
    );

    let items = env.search(env.global_query()).items;
    assert_eq!(items.len(), 2);
    let ids: Vec<String> = items
        .iter()
        .map(|asset| asset.id.clone())
        .chain(std::iter::once("missing-id".to_string()))
        .collect();

    let outcome = asset_service::delete_assets(&env.database, &ids, &env.cache);
    assert_eq!(outcome.succeeded, 2);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].id, "missing-id");

    for asset in &items {
        assert!(
            !Path::new(&asset.absolute_path).exists(),
            "文件应已移入回收站"
        );
    }
    assert_eq!(total_assets(&env.database, &work.id), 0);
}

#[test]
fn batch_tags_apply_and_remove_for_all_selected_assets() {
    let env = TestEnv::new();
    let work = env.create_work("批量标签");
    env.import(
        &work,
        "documents",
        &[
            env.write_source("标一.md", b"1"),
            env.write_source("标二.md", b"2"),
            env.write_source("标三.md", b"3"),
        ],
    );

    let ids: Vec<String> = env
        .search(env.global_query())
        .items
        .iter()
        .map(|asset| asset.id.clone())
        .collect();
    assert_eq!(ids.len(), 3);

    let added =
        tag_service::add_tag_to_assets(&env.database, "待剪辑", &ids).expect("批量加标签失败");
    assert_eq!(added.succeeded, 3);
    assert!(added.failed.is_empty());

    let mut query = env.global_query();
    query.keyword = None;
    let tagged = env.search(query);
    assert!(tagged.items.iter().all(|asset| !asset.tags.is_empty()));
    let tag_id = tagged.items[0].tags[0].id.clone();
    assert_eq!(tagged.items[0].tags[0].name, "待剪辑");

    let removed = tag_service::remove_tag_from_assets(&env.database, &tag_id, &ids)
        .expect("批量移除标签失败");
    assert_eq!(removed.succeeded, 3);
    let cleared = env.search(env.global_query());
    assert!(cleared.items.iter().all(|asset| asset.tags.is_empty()));
}

#[test]
fn cache_stats_and_cleanup_do_not_touch_assets() {
    let env = TestEnv::new();
    let work = env.create_work("缓存检查");
    env.import(
        &work,
        "images",
        &[env.write_source("图.png", b"not-an-image")],
    );

    let asset = env
        .search(env.global_query())
        .items
        .into_iter()
        .next()
        .unwrap();
    // 手工放一个缩略图，模拟生成结果
    let thumb_dir = env.cache.join(&work.id);
    fs::create_dir_all(&thumb_dir).unwrap();
    fs::write(thumb_dir.join("fake.jpg"), vec![0_u8; 2048]).unwrap();

    let stats = cache_service::stats(&env.database, &env.cache).unwrap();
    assert_eq!(stats.file_count, 1);
    assert_eq!(stats.total_bytes, 2048);
    assert_eq!(stats.limit_bytes, cache_service::DEFAULT_CACHE_LIMIT);

    let result = cache_service::clear_all(&env.cache).unwrap();
    assert_eq!(result.removed_files, 1);
    assert!(
        Path::new(&asset.absolute_path).is_file(),
        "原素材不能被删除"
    );
    assert_eq!(total_assets(&env.database, &work.id), 1, "索引记录必须保留");
}

/// 需要显式运行的大素材库性能测量。
#[test]
#[ignore]
fn measure_large_library_queries() {
    let env = TestEnv::new();
    let mut work_ids = Vec::new();
    for index in 0..5 {
        work_ids.push(env.create_work(&format!("压测作品{index}")));
    }

    let kinds = [
        (SectionKey::Documents, "md", MediaKind::Document),
        (SectionKey::Audio, "mp3", MediaKind::Audio),
        (SectionKey::Video, "mp4", MediaKind::Video),
        (SectionKey::Images, "png", MediaKind::Image),
    ];

    let started = Instant::now();
    let now = "2026-01-01T00:00:00.000Z".to_string();
    for (work_index, work) in work_ids.iter().enumerate() {
        let section_rows = env
            .database
            .with_conn(|connection| sections::list_by_work(connection, &work.id))
            .unwrap();
        for (kind_index, (key, extension, media_kind)) in kinds.iter().enumerate() {
            let section = section_rows
                .iter()
                .find(|section| section.key == *key)
                .expect("分区缺失");
            env.database
                .with_conn(|connection| {
                    let transaction = connection.transaction()?;
                    for index in 0..250 {
                        let number = inserted_serial(work_index, kind_index, index);
                        assets::insert(
                            &transaction,
                            &assets::NewAsset {
                                id: format!("asset-{number}"),
                                work_id: work.id.clone(),
                                section_id: section.id.clone(),
                                display_name: format!("素材{number}.{extension}"),
                                original_name: format!("素材{number}.{extension}"),
                                relative_path: format!(
                                    "{}/素材{number}.{extension}",
                                    section.relative_path
                                ),
                                source_path: None,
                                extension: Some(extension.to_string()),
                                detected_mime_type: None,
                                media_kind: *media_kind,
                                size_bytes: (number as i64) * 1024,
                                sha256: None,
                                status: AssetStatus::Ready,
                                created_at: now.clone(),
                                updated_at: now.clone(),
                            },
                        )?;
                    }
                    transaction
                        .commit()
                        .map_err(creatorflow_lib::errors::AppError::from)
                })
                .unwrap();
        }
    }
    let seed_elapsed = started.elapsed();
    println!("插入 5000 条素材耗时: {seed_elapsed:?}");

    let measure = |label: &str, query: AssetQuery| {
        let started = Instant::now();
        let page = env.search(query);
        println!(
            "{label}: {} ms（total={}，返回 {} 条）",
            started.elapsed().as_millis(),
            page.total,
            page.items.len()
        );
    };

    measure("跨作品首屏（50 条）", {
        let mut query = env.global_query();
        query.limit = Some(50);
        query
    });

    measure("关键词搜索", {
        let mut query = env.global_query();
        query.keyword = Some("素材49".to_string());
        query.limit = Some(50);
        query
    });

    measure("按大小排序", {
        let mut query = env.global_query();
        query.sort_key = Some(AssetSortKey::Size);
        query.sort_direction = Some(SortDirection::Desc);
        query.limit = Some(50);
        query
    });

    measure("末页", {
        let mut query = env.global_query();
        query.limit = Some(50);
        query.offset = Some(4950);
        query
    });

    measure("单作品列表", {
        let mut query = env.global_query();
        query.work_id = Some(work_ids[0].id.clone());
        query.limit = Some(50);
        query
    });
}

fn inserted_serial(work_index: usize, kind_index: usize, index: usize) -> usize {
    work_index * 1000 + kind_index * 250 + index
}
