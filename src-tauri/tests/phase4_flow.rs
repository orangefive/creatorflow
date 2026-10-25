//! 第四阶段集成测试：工作台聚合、任务生命周期、模板与旧库升级。

use std::fs;
use std::sync::Arc;

use creatorflow_lib::domain::task::{TaskInput, TaskPriority, TaskQuery, TaskStatus};
use creatorflow_lib::domain::template::TemplateInput;
use creatorflow_lib::domain::work::Work;
use creatorflow_lib::infrastructure::database::Database;
use creatorflow_lib::services::work_service::{self, CreateWorkInput};
use creatorflow_lib::services::{settings_service, task_service, template_service};
use tempfile::TempDir;

struct TestEnv {
    _temp: TempDir,
    _root: std::path::PathBuf,
    cache: std::path::PathBuf,
    database: Arc<Database>,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().expect("创建临时目录失败");
        let root = temp.path().join("works");
        fs::create_dir_all(&root).unwrap();
        let cache = temp.path().join("thumbnails");
        fs::create_dir_all(&cache).unwrap();
        let database =
            Arc::new(Database::open(temp.path().join("creatorflow.db")).expect("打开数据库失败"));
        settings_service::set_workspace_root(&database, root.to_string_lossy().as_ref())
            .expect("设置根目录失败");
        Self {
            _temp: temp,
            _root: root,
            cache,
            database,
        }
    }

    fn create_work(&self, name: &str) -> Work {
        self.create_work_with_template(name, None)
    }

    fn create_work_with_template(&self, name: &str, template_id: Option<&str>) -> Work {
        work_service::create_work(
            &self.database,
            CreateWorkInput {
                name: name.to_string(),
                description: None,
                root_directory: None,
                template_key: template_id.map(str::to_string),
            },
        )
        .expect("创建作品失败")
    }

    fn tasks(&self, work_id: &str) -> Vec<creatorflow_lib::domain::task::Task> {
        task_service::list_tasks(&self.database, work_id, &TaskQuery::default()).unwrap()
    }

    fn add_task(&self, work_id: &str, title: &str) -> creatorflow_lib::domain::task::Task {
        task_service::create_task(
            &self.database,
            work_id,
            &TaskInput {
                title: title.to_string(),
                description: None,
                priority: Some("normal".to_string()),
                due_at: None,
                status: None,
            },
        )
        .expect("创建任务失败")
    }
}

#[test]
fn upgrades_phase_three_database_and_keeps_data() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("legacy.db");

    // 构造一个 0.3.0 状态的库：0001—0004 迁移 + 迁移记录
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
        for (version, sql) in [
            (1, include_str!("../migrations/0001_initial.sql")),
            (2, include_str!("../migrations/0002_asset_metadata.sql")),
            (3, include_str!("../migrations/0003_tags.sql")),
            (4, include_str!("../migrations/0004_archive_restore.sql")),
        ] {
            connection.execute_batch(sql).unwrap();
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at)
                     VALUES (?1, ?2, '2026-01-01T00:00:00.000Z')",
                    rusqlite::params![version, format!("m{version}")],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO works (id, name, description, status, root_path, template_key,
                                    created_at, updated_at, archived_at, deleted_at)
                 VALUES ('w1', '旧作品', NULL, 'idea', '/tmp/w1', NULL,
                         '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', NULL, NULL)",
                [],
            )
            .unwrap();
    }

    let database = Database::open(&path).expect("升级失败");
    assert_eq!(database.schema_version().unwrap(), 5);

    let works = work_service::list_works(&database, true).unwrap();
    assert_eq!(works.len(), 1);
    assert_eq!(works[0].name, "旧作品");

    let templates = template_service::list_templates(&database).unwrap();
    assert_eq!(templates.len(), 3, "升级后应带上三个内置模板");
}

#[test]
fn task_lifecycle_and_completion_time() {
    let env = TestEnv::new();
    let work = env.create_work("任务生命周期");

    let task = env.add_task(&work.id, "写脚本");
    assert_eq!(task.status, TaskStatus::Todo);
    assert!(task.completed_at.is_none());

    let done = task_service::set_task_status(&env.database, &task.id, "done").unwrap();
    assert_eq!(done.status, TaskStatus::Done);
    assert!(done.completed_at.is_some(), "完成时应记录 completed_at");

    let reopened = task_service::set_task_status(&env.database, &task.id, "todo").unwrap();
    assert!(reopened.completed_at.is_none(), "重新打开应清空完成时间");

    let updated = task_service::update_task(
        &env.database,
        &task.id,
        &TaskInput {
            title: "写脚本 v2".into(),
            description: Some("补充开场".into()),
            priority: Some("urgent".into()),
            due_at: Some("2026-12-01T10:00:00+08:00".into()),
            status: Some("in_progress".into()),
        },
    )
    .unwrap();
    assert_eq!(updated.title, "写脚本 v2");
    assert_eq!(updated.priority, TaskPriority::Urgent);
    assert_eq!(updated.status, TaskStatus::InProgress);
    assert!(updated.due_at.unwrap().ends_with('Z'));

    task_service::delete_task(&env.database, &task.id).unwrap();
    assert!(env.tasks(&work.id).is_empty());
}

#[test]
fn rejects_invalid_task_input_and_missing_work() {
    let env = TestEnv::new();
    let work = env.create_work("校验");

    let blank = task_service::create_task(
        &env.database,
        &work.id,
        &TaskInput {
            title: "   ".into(),
            description: None,
            priority: None,
            due_at: None,
            status: None,
        },
    );
    assert!(blank.is_err());

    let bad_status = task_service::create_task(
        &env.database,
        &work.id,
        &TaskInput {
            title: "合法的标题".into(),
            description: None,
            priority: None,
            due_at: None,
            status: Some("archived".into()),
        },
    );
    assert!(bad_status.is_err());

    let missing_work = task_service::create_task(
        &env.database,
        "not-a-work",
        &TaskInput {
            title: "孤儿任务".into(),
            description: None,
            priority: None,
            due_at: None,
            status: None,
        },
    );
    let error = missing_work.expect_err("作品不存在时应失败");
    assert_eq!(error.code, creatorflow_lib::errors::ErrorCode::NotFound);
}

#[test]
fn progress_ignores_cancelled_and_reports_no_tasks() {
    let env = TestEnv::new();
    let work = env.create_work("进度");

    let empty = task_service::task_summary(&env.database, &work.id).unwrap();
    assert_eq!(empty.total, 0);
    assert_eq!(empty.completion_rate, None, "没有任务时不应给出 0%");

    let first = env.add_task(&work.id, "一");
    let second = env.add_task(&work.id, "二");
    let third = env.add_task(&work.id, "三");
    task_service::set_task_status(&env.database, &first.id, "done").unwrap();
    task_service::set_task_status(&env.database, &second.id, "cancelled").unwrap();
    let _ = third;

    let summary = task_service::task_summary(&env.database, &work.id).unwrap();
    assert_eq!(summary.total, 3);
    assert_eq!(summary.done, 1);
    assert_eq!(summary.cancelled, 1);
    assert_eq!(summary.todo, 1);
    // done / (todo + in_progress + done) = 1 / 2
    assert_eq!(summary.completion_rate, Some(0.5));

    let only_cancelled = TestEnv::new();
    let work = only_cancelled.create_work("只有取消");
    let task = only_cancelled.add_task(&work.id, "取消掉");
    task_service::set_task_status(&only_cancelled.database, &task.id, "cancelled").unwrap();
    let summary = task_service::task_summary(&only_cancelled.database, &work.id).unwrap();
    assert_eq!(summary.completion_rate, None);
}

#[test]
fn overdue_only_counts_open_tasks_with_past_due_date() {
    let env = TestEnv::new();
    let work = env.create_work("逾期");

    let past = task_service::create_task(
        &env.database,
        &work.id,
        &TaskInput {
            title: "已逾期".into(),
            description: None,
            priority: None,
            due_at: Some("2020-01-01T00:00:00.000Z".into()),
            status: None,
        },
    )
    .unwrap();
    assert!(past.is_overdue, "截止时间已过且未完成应标记逾期");

    let done = task_service::create_task(
        &env.database,
        &work.id,
        &TaskInput {
            title: "逾期但已完成".into(),
            description: None,
            priority: None,
            due_at: Some("2020-01-01T00:00:00.000Z".into()),
            status: Some("done".into()),
        },
    )
    .unwrap();
    assert!(!done.is_overdue, "已完成任务不算逾期");

    let overdue = task_service::list_tasks(
        &env.database,
        &work.id,
        &TaskQuery {
            overdue_only: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(overdue.len(), 1);
    assert_eq!(overdue[0].title, "已逾期");

    let summary = task_service::task_summary(&env.database, &work.id).unwrap();
    assert_eq!(summary.overdue, 1);
}

#[test]
fn tasks_follow_manual_order_and_move() {
    let env = TestEnv::new();
    let work = env.create_work("排序");
    let a = env.add_task(&work.id, "A");
    let b = env.add_task(&work.id, "B");
    let c = env.add_task(&work.id, "C");

    let order: Vec<String> = env.tasks(&work.id).into_iter().map(|t| t.title).collect();
    assert_eq!(order, vec!["A", "B", "C"]);

    task_service::move_task(&env.database, &c.id, "up").unwrap();
    let order: Vec<String> = env.tasks(&work.id).into_iter().map(|t| t.title).collect();
    assert_eq!(order, vec!["A", "C", "B"]);

    task_service::move_task(&env.database, &a.id, "up").unwrap();
    let order: Vec<String> = env.tasks(&work.id).into_iter().map(|t| t.title).collect();
    assert_eq!(order, vec!["A", "C", "B"], "已在顶部时不应改变顺序");

    let _ = b;
}

#[test]
fn filters_by_status_and_priority() {
    let env = TestEnv::new();
    let work = env.create_work("筛选");
    env.add_task(&work.id, "普通");
    let urgent = task_service::create_task(
        &env.database,
        &work.id,
        &TaskInput {
            title: "紧急".into(),
            description: None,
            priority: Some("urgent".into()),
            due_at: None,
            status: Some("in_progress".into()),
        },
    )
    .unwrap();

    let only_urgent = task_service::list_tasks(
        &env.database,
        &work.id,
        &TaskQuery {
            priority: Some("urgent".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(only_urgent.len(), 1);
    assert_eq!(only_urgent[0].id, urgent.id);

    let only_todo = task_service::list_tasks(
        &env.database,
        &work.id,
        &TaskQuery {
            status: Some("todo".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(only_todo.len(), 1);
    assert_eq!(only_todo[0].title, "普通");
}

#[test]
fn builtin_templates_exist_with_initial_tasks() {
    let env = TestEnv::new();
    let templates = template_service::list_templates(&env.database).unwrap();
    assert_eq!(templates.len(), 3);
    assert!(templates.iter().all(|template| template.is_builtin));

    let video = templates
        .iter()
        .find(|template| template.id == "builtin-video")
        .unwrap();
    assert_eq!(video.name, "通用视频");
    assert_eq!(video.tasks.len(), 6);
    assert_eq!(video.tasks[0].title, "确定选题与脚本");

    let clip = templates
        .iter()
        .find(|template| template.id == "builtin-clip")
        .unwrap();
    assert_eq!(clip.tasks.len(), 5);

    let podcast = templates
        .iter()
        .find(|template| template.id == "builtin-podcast")
        .unwrap();
    assert_eq!(podcast.tasks.len(), 5);

    let default = template_service::default_template(&env.database).unwrap();
    assert_eq!(default.id, "builtin-video");
}

#[test]
fn copying_editing_deleting_and_default_template() {
    let env = TestEnv::new();
    let copy = template_service::copy_template(&env.database, "builtin-video", Some("我的流程"))
        .expect("复制模板失败");
    assert!(!copy.is_builtin);
    assert_eq!(copy.name, "我的流程");
    assert_eq!(copy.tasks.len(), 6);

    let edited = template_service::update_template(
        &env.database,
        &copy.id,
        &TemplateInput {
            name: "我的流程 v2".into(),
            description: Some("精简版".into()),
            tasks: vec![
                creatorflow_lib::domain::template::TemplateTaskInput {
                    title: "选题".into(),
                    description: None,
                    priority: Some("high".into()),
                },
                creatorflow_lib::domain::template::TemplateTaskInput {
                    title: "拍摄".into(),
                    description: None,
                    priority: None,
                },
            ],
        },
    )
    .expect("编辑自定义模板失败");
    assert_eq!(edited.name, "我的流程 v2");
    assert_eq!(edited.tasks.len(), 2);

    template_service::set_default_template(&env.database, &copy.id).unwrap();
    let default = template_service::default_template(&env.database).unwrap();
    assert_eq!(default.id, copy.id);

    template_service::delete_template(&env.database, &copy.id).unwrap();
    let default = template_service::default_template(&env.database).unwrap();
    assert_eq!(default.id, "builtin-video", "删除默认模板后应回落");

    assert!(template_service::delete_template(&env.database, "builtin-video").is_err());
    assert!(template_service::update_template(
        &env.database,
        "builtin-video",
        &TemplateInput {
            name: "改名".into(),
            description: None,
            tasks: vec![],
        }
    )
    .is_err());
}

#[test]
fn creating_work_with_template_writes_snapshot_tasks() {
    let env = TestEnv::new();
    let work = env.create_work_with_template("带模板的作品", Some("builtin-podcast"));

    let tasks = env.tasks(&work.id);
    assert_eq!(tasks.len(), 5, "应写入模板的 5 个初始任务");
    assert_eq!(tasks[0].title, "确定话题与嘉宾");
    assert!(tasks.iter().all(|task| task.status == TaskStatus::Todo));

    // 修改模板不得影响已创建作品
    let copy =
        template_service::copy_template(&env.database, "builtin-podcast", Some("改版")).unwrap();
    template_service::update_template(
        &env.database,
        &copy.id,
        &TemplateInput {
            name: "改版".into(),
            description: None,
            tasks: vec![],
        },
    )
    .unwrap();
    assert_eq!(env.tasks(&work.id).len(), 5);
    let builtin = template_service::get_template(&env.database, "builtin-podcast").unwrap();
    assert_eq!(builtin.tasks.len(), 5, "内置模板内容不应被自定义模板影响");
}

#[test]
fn creating_work_with_unknown_template_fails_without_leftovers() {
    let env = TestEnv::new();
    let result = work_service::create_work(
        &env.database,
        CreateWorkInput {
            name: "不该创建".into(),
            description: None,
            root_directory: None,
            template_key: Some("not-a-template".into()),
        },
    );
    let error = result.expect_err("未知模板应直接失败");
    assert_eq!(error.code, creatorflow_lib::errors::ErrorCode::NotFound);
    assert!(
        work_service::list_works(&env.database, true)
            .unwrap()
            .is_empty(),
        "失败时不应留下作品记录"
    );
    assert!(!env._root.join("不该创建").exists(), "失败时不应留下目录");
}

#[test]
fn dashboard_aggregates_tasks_and_recent_assets() {
    let env = TestEnv::new();
    let work = env.create_work("工作台");
    let a = env.add_task(&work.id, "待办任务");
    task_service::set_task_status(&env.database, &a.id, "in_progress").unwrap();

    let dashboard = task_service::work_dashboard(&env.database, &work.id, &env.cache).unwrap();
    assert_eq!(dashboard.work.id, work.id);
    assert_eq!(dashboard.summary.total, 1);
    assert_eq!(dashboard.upcoming.len(), 1);
    assert!(dashboard.recent_assets.is_empty());

    // 已归档作品不再接受新任务
    work_service::archive_work(&env.database, &work.id).unwrap();
    let blocked = task_service::create_task(
        &env.database,
        &work.id,
        &TaskInput {
            title: "归档后新增".into(),
            description: None,
            priority: None,
            due_at: None,
            status: None,
        },
    );
    assert!(blocked.is_err(), "归档作品不应接受新任务");
}
