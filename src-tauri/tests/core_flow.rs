//! 第一阶段核心闭环的集成测试：
//! 创建作品 → 建目录与 manifest → 登记分区 → 导入素材 → 列表校验 → 缺失检测 → 中断恢复
//!
//! 所有测试使用临时目录与临时数据库，绝不触碰真实素材目录。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use creatorflow_lib::domain::asset::AssetStatus;
use creatorflow_lib::domain::import::ImportStatus;
use creatorflow_lib::domain::section::SectionKey;
use creatorflow_lib::domain::work::{Work, WorkStatus};
use creatorflow_lib::infrastructure::database::Database;
use creatorflow_lib::repositories::{import_jobs, sections};
use creatorflow_lib::services::import_service::{self, ImportManager};
use creatorflow_lib::services::work_service::CreateWorkInput;
use creatorflow_lib::services::{asset_service, settings_service, work_service};
use tempfile::TempDir;

struct TestEnv {
    _temp: TempDir,
    root: PathBuf,
    database: Arc<Database>,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().expect("创建临时目录失败");
        let root = temp.path().join("CreatorFlow-Works");
        fs::create_dir_all(&root).expect("创建作品根目录失败");
        let database = Arc::new(
            Database::open(temp.path().join("creatorflow.db")).expect("打开临时数据库失败"),
        );
        settings_service::set_workspace_root(&database, root.to_string_lossy().as_ref())
            .expect("设置作品根目录失败");
        Self {
            _temp: temp,
            root,
            database,
        }
    }

    fn create_work(&self, name: &str) -> Work {
        work_service::create_work(
            &self.database,
            CreateWorkInput {
                name: name.to_string(),
                description: Some("集成测试作品".to_string()),
                root_directory: None,
                template_key: Some("builtin-video".to_string()),
            },
        )
        .expect("创建作品失败")
    }

    fn thumbnail_cache(&self) -> PathBuf {
        let directory = self._temp.path().join("thumbnails");
        fs::create_dir_all(&directory).expect("创建缩略图目录失败");
        directory
    }

    fn manager(&self) -> Arc<ImportManager> {
        Arc::new(ImportManager::headless(Arc::clone(&self.database)))
    }

    fn write_source(&self, name: &str, content: &[u8]) -> PathBuf {
        let source_dir = self._temp.path().join("sources");
        fs::create_dir_all(&source_dir).expect("创建来源目录失败");
        let path = source_dir.join(name);
        fs::write(&path, content).expect("写入来源文件失败");
        path
    }
}

#[test]
fn create_work_builds_directories_manifest_and_sections() {
    let env = TestEnv::new();
    let work = env.create_work("我的第一支视频");

    assert_eq!(work.status, WorkStatus::Idea);
    assert_eq!(work.name, "我的第一支视频");
    assert!(work.asset_count == 0);

    let work_dir = env.root.join("我的第一支视频");
    assert!(work_dir.is_dir(), "作品目录应存在");
    for folder in ["01_文档", "02_音频", "03_视频", "04_图片"] {
        assert!(work_dir.join(folder).is_dir(), "分区目录 {folder} 应存在");
    }

    let manifest_path = work_dir.join("creatorflow.manifest.json");
    assert!(manifest_path.is_file(), "manifest 应存在");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["workId"], work.id);
    assert_eq!(manifest["name"], "我的第一支视频");
    assert_eq!(manifest["templateKey"], "builtin-video");

    let stored = env
        .database
        .with_conn(|connection| sections::list_by_work(connection, &work.id))
        .expect("查询分区失败");
    assert_eq!(stored.len(), 4);
    let keys: Vec<&str> = stored.iter().map(|section| section.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["documents", "audio", "video", "images"],
        "分区顺序应为 文档/音频/视频/图片"
    );
    assert!(stored
        .iter()
        .all(|section| section.relative_path == section.key.folder_name()));

    let listed = work_service::list_works(&env.database, false).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, work.id);
}

#[test]
fn create_work_rejects_duplicate_directory() {
    let env = TestEnv::new();
    env.create_work("重复名称");

    let error = work_service::create_work(
        &env.database,
        CreateWorkInput {
            name: "重复名称".to_string(),
            description: None,
            root_directory: None,
            template_key: None,
        },
    )
    .expect_err("同名作品应被拒绝");
    assert_eq!(error.code, creatorflow_lib::errors::ErrorCode::Conflict);
}

#[test]
fn rename_work_moves_directory_and_updates_index() {
    let env = TestEnv::new();
    let work = env.create_work("旧名称");

    let renamed = work_service::rename_work(&env.database, &work.id, "新名称").expect("重命名失败");
    assert_eq!(renamed.name, "新名称");

    let old_dir = env.root.join("旧名称");
    let new_dir = env.root.join("新名称");
    assert!(!old_dir.exists(), "旧目录应已改名");
    assert!(new_dir.is_dir(), "新目录应存在");
    assert!(new_dir.join("01_文档").is_dir(), "分区目录应随作品移动");
    assert_eq!(renamed.root_path, new_dir.to_string_lossy().to_string());

    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(new_dir.join("creatorflow.manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["name"], "新名称");
}

#[test]
fn archive_work_keeps_directory_and_marks_status() {
    let env = TestEnv::new();
    let work = env.create_work("待归档作品");

    let archived = work_service::archive_work(&env.database, &work.id).expect("归档失败");
    assert_eq!(archived.status, WorkStatus::Archived);
    assert!(archived.archived_at.is_some());
    assert!(env.root.join("待归档作品").is_dir(), "归档不得删除目录");
    assert!(env.root.join("待归档作品").join("04_图片").is_dir());

    let visible = work_service::list_works(&env.database, false).unwrap();
    assert!(visible.is_empty(), "默认列表应隐藏已归档作品");
    let all = work_service::list_works(&env.database, true).unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn import_copies_file_and_registers_asset() {
    let env = TestEnv::new();
    let work = env.create_work("导入测试");
    let content = b"# script\nhello creatorflow\n";
    let source = env.write_source("脚本.md", content);

    let manager = env.manager();
    let jobs = manager
        .start_inline(
            &work.id,
            "documents",
            &[source.to_string_lossy().to_string()],
        )
        .expect("启动导入失败");

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, ImportStatus::Completed);
    assert!(jobs[0].asset_id.is_some());
    assert_eq!(jobs[0].bytes_copied, content.len() as i64);

    let target = env.root.join("导入测试").join("01_文档").join("脚本.md");
    assert!(target.is_file(), "目标文件应存在");
    assert_eq!(fs::read(&target).unwrap(), content);

    assert!(source.is_file(), "源文件必须保留");
    assert_eq!(fs::read(&source).unwrap(), content, "源文件内容不得改变");

    let assets = asset_service::list_assets(
        &env.database,
        &work.id,
        Some(SectionKey::Documents),
        &env.thumbnail_cache(),
    )
    .expect("查询素材失败");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].status, AssetStatus::Ready);
    assert_eq!(assets[0].size_bytes, content.len() as i64);
    assert_eq!(assets[0].display_name, "脚本.md");
}

#[test]
fn import_never_overwrites_existing_file() {
    let env = TestEnv::new();
    let work = env.create_work("冲突测试");

    let target_dir = env.root.join("冲突测试").join("01_文档");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("脚本.md"), b"existing content").unwrap();

    let source = env.write_source("脚本.md", b"new content");
    let manager = env.manager();
    let jobs = manager
        .start_inline(
            &work.id,
            "documents",
            &[source.to_string_lossy().to_string()],
        )
        .expect("启动导入失败");

    assert_eq!(jobs[0].status, ImportStatus::Completed);
    assert_eq!(
        fs::read(target_dir.join("脚本.md")).unwrap(),
        b"existing content",
        "既有文件不得被覆盖"
    );
    assert_eq!(
        fs::read(target_dir.join("脚本 (1).md")).unwrap(),
        b"new content",
        "新文件应使用不冲突的名称"
    );
}

#[test]
fn import_marks_job_failed_when_source_missing() {
    let env = TestEnv::new();
    let work = env.create_work("缺失来源");
    let missing = env._temp.path().join("not-exists.mp4");

    let manager = env.manager();
    let jobs = manager
        .start_inline(&work.id, "video", &[missing.to_string_lossy().to_string()])
        .expect("任务应被创建");

    assert_eq!(jobs[0].status, ImportStatus::Failed);
    let message = jobs[0].error_message.clone().unwrap_or_default();
    assert!(
        message.contains("源文件"),
        "错误信息应说明源文件问题，实际：{message}"
    );
    assert!(jobs[0].asset_id.is_none());
}

#[test]
fn deleted_file_is_marked_missing_not_removed_from_index() {
    let env = TestEnv::new();
    let work = env.create_work("缺失检测");
    let source = env.write_source("封面.png", b"fake-png-bytes");

    let manager = env.manager();
    let jobs = manager
        .start_inline(&work.id, "images", &[source.to_string_lossy().to_string()])
        .expect("启动导入失败");
    assert_eq!(jobs[0].status, ImportStatus::Completed);

    let target = env.root.join("缺失检测").join("04_图片").join("封面.png");
    fs::remove_file(&target).unwrap();

    let assets = asset_service::list_assets(
        &env.database,
        &work.id,
        Some(SectionKey::Images),
        &env.thumbnail_cache(),
    )
    .expect("查询素材失败");
    assert_eq!(assets.len(), 1, "记录不得被静默删除");
    assert_eq!(assets[0].status, AssetStatus::Missing);

    // 文件恢复后，列表应重新校准为 ready。
    fs::write(&target, b"fake-png-bytes").unwrap();
    let restored = asset_service::list_assets(
        &env.database,
        &work.id,
        Some(SectionKey::Images),
        &env.thumbnail_cache(),
    )
    .expect("查询素材失败");
    assert_eq!(restored[0].status, AssetStatus::Ready);
}

#[test]
fn startup_recovery_marks_interrupted_jobs_safely() {
    let env = TestEnv::new();
    let work = env.create_work("恢复测试");
    let section = env
        .database
        .with_conn(|connection| sections::find_by_key(connection, &work.id, SectionKey::Documents))
        .unwrap()
        .expect("分区应存在");

    // 场景一：目标文件已完整写入，但任务状态停留在 copying。
    let completed_target = env.root.join("恢复测试").join("01_文档").join("已完成.md");
    fs::write(&completed_target, b"done").unwrap(); // 4 字节
    let completed_job_id = "job-completed";
    env.database
        .with_conn(|connection| {
            import_jobs::insert(
                connection,
                &import_jobs::NewImportJob {
                    id: completed_job_id.to_string(),
                    work_id: work.id.clone(),
                    section_id: section.id.clone(),
                    source_path: "C:\\sources\\已完成.md".to_string(),
                    created_at: "2026-01-01T00:00:00.000Z".to_string(),
                    updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                },
            )?;
            import_jobs::update_paths(
                connection,
                completed_job_id,
                "01_文档/已完成.md",
                "01_文档/.已完成.md.part",
                4,
                "2026-01-01T00:00:00.000Z",
            )?;
            Ok(())
        })
        .unwrap();

    // 场景二：只有临时文件，正式文件不存在。
    let temp_target = env
        .root
        .join("恢复测试")
        .join("01_文档")
        .join(".中断.md.part");
    fs::write(&temp_target, b"partial").unwrap();
    let interrupted_job_id = "job-interrupted";
    env.database
        .with_conn(|connection| {
            import_jobs::insert(
                connection,
                &import_jobs::NewImportJob {
                    id: interrupted_job_id.to_string(),
                    work_id: work.id.clone(),
                    section_id: section.id.clone(),
                    source_path: "C:\\sources\\中断.md".to_string(),
                    created_at: "2026-01-01T00:00:00.000Z".to_string(),
                    updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                },
            )?;
            import_jobs::update_paths(
                connection,
                interrupted_job_id,
                "01_文档/中断.md",
                "01_文档/.中断.md.part",
                100,
                "2026-01-01T00:00:00.000Z",
            )?;
            Ok(())
        })
        .unwrap();

    let report = import_service::recover_incomplete_jobs(&env.database).expect("恢复失败");
    assert_eq!(report.examined_jobs, 2);
    assert_eq!(report.completed_jobs, 1);
    assert_eq!(report.failed_jobs, 1);
    assert!(report.cleaned_temporary_files >= 1);
    assert!(!temp_target.exists(), "临时文件应被清理");
    assert!(!report.messages.is_empty(), "应给出可读的恢复提示");

    let completed_job = env
        .database
        .with_conn(|connection| import_jobs::find_by_id(connection, completed_job_id))
        .unwrap()
        .unwrap();
    assert_eq!(completed_job.status, ImportStatus::Completed);
    assert!(completed_job.asset_id.is_some());

    let interrupted_job = env
        .database
        .with_conn(|connection| import_jobs::find_by_id(connection, interrupted_job_id))
        .unwrap()
        .unwrap();
    assert_eq!(interrupted_job.status, ImportStatus::Failed);
    assert_eq!(interrupted_job.error_code.as_deref(), Some("interrupted"));

    let assets = asset_service::list_assets(
        &env.database,
        &work.id,
        Some(SectionKey::Documents),
        &env.thumbnail_cache(),
    )
    .unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].display_name, "已完成.md");
}

#[test]
fn sqlite_rejects_duplicate_sections_and_paths() {
    let env = TestEnv::new();
    let work = env.create_work("约束测试");
    let existing = env
        .database
        .with_conn(|connection| sections::list_by_work(connection, &work.id))
        .unwrap();

    let duplicate_key = env.database.with_conn(|connection| {
        sections::insert(
            connection,
            &creatorflow_lib::domain::section::Section {
                id: "dup-key".to_string(),
                work_id: work.id.clone(),
                key: SectionKey::Documents,
                name: "文档".to_string(),
                relative_path: "99_重复".to_string(),
                sort_order: 9,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                asset_count: 0,
            },
        )
    });
    assert!(duplicate_key.is_err(), "同一作品的分区 key 必须唯一");

    let duplicate_path = env.database.with_conn(|connection| {
        sections::insert(
            connection,
            &creatorflow_lib::domain::section::Section {
                id: "dup-path".to_string(),
                work_id: work.id.clone(),
                key: SectionKey::Audio,
                name: "音频".to_string(),
                relative_path: existing[0].relative_path.clone(),
                sort_order: 10,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                asset_count: 0,
            },
        )
    });
    assert!(duplicate_path.is_err(), "同一作品的相对路径必须唯一");
}

#[test]
fn updating_work_status_persists_and_rejects_system_status() {
    let env = TestEnv::new();
    let work = env.create_work("状态测试");

    let updated = work_service::update_status(&env.database, &work.id, "in_production")
        .expect("更新状态失败");
    assert_eq!(updated.status, WorkStatus::InProduction);

    let reloaded = work_service::get_work(&env.database, &work.id).unwrap();
    assert_eq!(reloaded.status, WorkStatus::InProduction);

    let error = work_service::update_status(&env.database, &work.id, "archived")
        .expect_err("归档状态不能通过通用状态接口设置");
    assert_eq!(error.code, creatorflow_lib::errors::ErrorCode::InvalidInput);
}
