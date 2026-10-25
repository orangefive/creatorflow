# 测试与验收

## 1. 自动化测试清单

### 1.1 Rust（`cargo test`）

单元测试位于各自模块内：

| 模块 | 覆盖内容 |
|---|---|
| `domain::work` | 作品名称合法/非法（空值、首尾空格、非法字符、保留名、长度、结尾句点）；状态解析与用户可选状态 |
| `domain::section` | 四个分区 key 稳定、目录名与排序、未知 key 拒绝、扩展名 → 分区映射、分区与媒体类型对应 |
| `domain::asset` | 媒体类型与素材状态往返解析、MIME 映射 |
| `domain::import` | 导入状态往返解析、终态/运行中判定 |
| `infrastructure::filesystem` | 相对路径规范化（`\` 与 `/`、`.//`）、拒绝 `..` 与盘符、`resolve_within` 限制在根目录内、唯一文件名递增后缀、无扩展名去重、流式复制后源文件保留且大小一致、目标存在时拒绝覆盖、取消标志生效 |
| `infrastructure::database` | 打开数据库自动建表（含 `schema_migrations`）、外键约束生效 |

集成测试 `src-tauri/tests/core_flow.rs`（全部使用临时目录与临时数据库）：

| 测试 | 验证点 |
|---|---|
| `create_work_builds_directories_manifest_and_sections` | 作品目录、四个分区目录、manifest 字段、4 条 sections 及顺序、作品列表 |
| `create_work_rejects_duplicate_directory` | 同名目录冲突返回 `conflict` |
| `rename_work_moves_directory_and_updates_index` | 目录改名、分区随之移动、`root_path` 与 manifest 同步 |
| `archive_work_keeps_directory_and_marks_status` | 归档只改状态，目录保留；默认列表隐藏归档作品 |
| `import_copies_file_and_registers_asset` | 复制完成、目标内容一致、**源文件保留**、asset 记录与大小正确 |
| `import_never_overwrites_existing_file` | 同名目标保留原内容，新文件落为 `脚本 (1).md` |
| `import_marks_job_failed_when_source_missing` | 源文件缺失时任务失败且有可读错误 |
| `deleted_file_is_marked_missing_not_removed_from_index` | 删除磁盘文件后记录保留并标记 `missing`；恢复文件后回到 `ready` |
| `startup_recovery_marks_interrupted_jobs_safely` | 目标完整 → 恢复为 completed 并补登记；仅有临时文件 → 清理并置 failed(`interrupted`) |
| `sqlite_rejects_duplicate_sections_and_paths` | 分区 key 与相对路径唯一约束生效 |
| `updating_work_status_persists_and_rejects_system_status` | 状态持久化；`archived` 不能走通用状态接口 |

### 1.2 Rust 集成测试（第二阶段，`tests/phase2_flow.rs`）

| 测试 | 验证点 |
|---|---|
| `migrates_phase_one_database_without_data_loss` | 只应用过 0001 的旧库升级到 0003：旧作品/素材保留，新字段默认 `pending`，迁移记录为 1/2/3，标签表存在 |
| `bounded_queue_never_exceeds_concurrency_limit` | 6 个文件、并发配置 2 时，峰值并发 ≤ 2，全部完成，素材数正确 |
| `imports_empty_file` | 0 字节文件可导入，大小记录为 0 |
| `cancelling_running_job_leaves_no_asset_or_temp_file` | 复制中取消：状态为 cancelled、无素材记录、无 `.part` 残留、源文件保留 |
| `failed_job_can_be_retried_without_duplicate_assets` | 源文件缺失失败后补回文件重试成功，只产生 1 条素材 |
| `search_filters_sorts_and_paginates_in_database` | 关键词/类型/状态筛选、按大小排序、分页 total 与 offset 正确 |
| `section_and_media_filters_combine` | 分区与类型筛选可组合，互斥结果为空 |
| `tags_are_reused_and_filterable` | 同名标签复用为一条记录、计数正确、按标签筛选、移除关联后计数下降 |
| `extracts_image_metadata_and_thumbnail` | 图片宽高写入，缩略图写入缓存目录，列表返回绝对缩略图路径 |
| `extracts_wav_duration_and_sample_rate` | WAV 时长（约 200ms）、采样率 44.1kHz、单声道 |
| `corrupt_image_is_metadata_failed_but_asset_stays_usable` | 损坏图片元数据失败但素材仍为 ready，列表正常返回 |
| `extracts_mp4_metadata_and_cover_when_ffmpeg_available` | 本机存在 ffmpeg 时用 lavfi 生成 2 秒视频，校验时长/分辨率与封面文件；无 ffmpeg 时跳过 |
| `archiving_remembers_previous_status_and_can_be_restored` | 归档记录原状态并从默认列表隐藏；恢复后回到原状态并清空归档时间；再次恢复报 `invalid_input` |
| `unarchive_falls_back_to_idea_when_previous_status_is_unknown` | 旧数据没有归档前状态时，恢复回落到「构思中」 |
| `deleting_work_recycles_directory_and_hides_it` | 删除作品后目录从磁盘消失（进入回收站），默认与含归档列表均为空，按作品查素材报 `not_found` |
| `deleting_asset_recycles_file_and_removes_it_from_lists` | 删除素材后文件消失，列表与分区计数归零，记录不可见 |
| `deleting_missing_asset_still_removes_the_record` | 文件已不在时仍可清理记录 |

### 1.3 第三阶段集成测试（`tests/phase3_flow.rs`）

| 测试 | 验证点 |
|---|---|
| `global_search_returns_assets_from_multiple_works` | 跨两个作品检索返回 2 条，`work_name` / `section_key` / 绝对路径均正确；按作品筛选只剩目标作品 |
| `global_search_matches_work_name_and_section` | 关键词命中作品名；分区筛选只返回该分区素材 |
| `global_search_tag_filter_does_not_leak_between_works` | 标签过滤只返回关联素材，另一作品素材无该标签 |
| `global_search_pagination_total_counts_all_works` | 跨作品 total = 3，分页每页 2 条，第二页 1 条 |
| `global_search_marks_missing_files` | 文件被删后搜索结果状态变为 `missing` |
| `batch_delete_removes_files_and_reports_failures` | 批量删除 2 个成功、1 个不存在的 id 记为失败；文件离开原位、素材计数归零 |
| `batch_tags_apply_and_remove_for_all_selected_assets` | 批量加标签 3 条全部成功并可筛选；批量移除后标签清空 |
| `cache_stats_and_cleanup_do_not_touch_assets` | 缓存统计正确；清理只删缓存文件，原素材与索引保留 |
| `measure_large_library_queries`（`#[ignore]`） | 生成 5000 条素材并测量检索/排序/分页耗时，需显式运行 |
| `relocation_requires_confirmation_when_content_differs` | 大小不一致时需确认、不写库；确认后更新路径/大小并重置元数据 |
| `relocation_accepts_matching_content_without_confirmation` | 哈希一致时直接关联，`hashMatches = true` |
| `relocation_rejects_files_outside_work_directory` | 作品目录外的文件被拒绝（`invalid_input`） |

### 1.4 第四阶段集成测试（`tests/phase4_flow.rs`）

| 测试 | 验证点 |
|---|---|
| `upgrades_phase_three_database_and_keeps_data` | 由 0001—0004 构造的 0.3.0 库升级到 5，作品保留、三个内置模板就位 |
| `task_lifecycle_and_completion_time` | 创建 → 完成写入 completed_at → 重开清空 → 编辑 → 删除 |
| `rejects_invalid_task_input_and_missing_work` | 空标题、非法状态、`work_id` 不存在均失败 |
| `progress_ignores_cancelled_and_reports_no_tasks` | done/cancelled 口径；只有已取消任务时 completionRate 为 null |
| `overdue_only_counts_open_tasks_with_past_due_date` | 逾期判定与「只看逾期」筛选；已完成任务不算逾期 |
| `tasks_follow_manual_order_and_move` | 手工顺序、上移、顶部上移不变 |
| `filters_by_status_and_priority` | 状态与优先级筛选 |
| `builtin_templates_exist_with_initial_tasks` | 三个内置模板及任务数量与内容 |
| `copying_editing_deleting_and_default_template` | 复制/编辑/删除/设为默认；内置模板禁止改删；删除默认后回落 |
| `creating_work_with_template_writes_snapshot_tasks` | 新建作品写入 5 条任务；改模板不影响已有作品 |
| `creating_work_with_unknown_template_fails_without_leftovers` | 未知模板直接失败，无作品记录、无目录 |
| `dashboard_aggregates_tasks_and_recent_assets` | 工作台聚合；归档作品拒绝新增任务 |

### 1.5 前端（`vitest`）

| 文件 | 覆盖内容 |
|---|---|
| `src/utils/format.spec.ts` | 字节/时间/进度/时长/分辨率/帧率格式化、Windows 与 POSIX 路径取文件名与扩展名、异常输入 |
| `src/utils/validation.spec.ts` | 与 Rust 一致的作品名称校验规则 |
| `src/stores/assets.spec.ts` | 查询参数与分页拼装、缺失数量统计、失败时清空并记录错误、翻页边界、重置筛选、标签增删与错误回传 |
| `src/stores/imports.spec.ts` | 进度事件应用到对应任务、未知任务忽略、完成信号只触发一次、活动/失败任务分类、移除记录 |
| `src/stores/tasks.spec.ts` | 加载列表与统计、失败时报错、创建后刷新、创建失败不报成功、状态切换 |
| `src/stores/globalSearch.spec.ts` | 筛选变更重置分页、分页边界与页数计算、失败时清空结果并记录错误、重置全部筛选 |
| `src/stores/works.spec.ts` | 删除后从列表移除、删除失败保留记录并回传错误、删除当前作品时清空上下文、恢复归档后替换记录、恢复失败回传后端文案 |
| `src/components/WorkStatusTag.spec.ts` | 状态文案与标签类型渲染 |

### 1.6 测试分层与说明

| 层级 | 覆盖内容 | 能否代表桌面端 |
|---|---|---|
| 单元测试（Rust `#[cfg(test)]`） | 迁移、路径、复制、回收站、备份、缓存、标签规范化 | 否 |
| Rust 集成测试（`tests/*.rs`） | 创建/导入/搜索/元数据/删除/备份恢复等真实数据库与磁盘流程 | 否（无界面） |
| 前端单元测试（Vitest） | store 逻辑、格式化与校验函数、组件渲染 | 否 |
| 浏览器 mock 渲染（截图脚本） | 前端页面在 Edge 中的渲染与交互布局，IPC 由 mock 提供 | **否** |
| 桌面手工验收 | 真实 Tauri 窗口、系统文件对话框、回收站、托盘行为 | 是（人工执行） |
| 原生 E2E（tauri-driver） | 未建立，见第 5 节 | — |

## 2. 质量门槛命令

```bash
npm run typecheck     # vue-tsc --noEmit
npm run lint          # eslint . --max-warnings 0
npm run test          # vitest run
npm run rust:fmt      # cargo fmt --check
npm run rust:clippy   # cargo clippy --all-targets -- -D warnings
npm run rust:test     # cargo test
npm run tauri:build   # 生产构建 + 安装包
```

## 3. 实际执行结果（0.4.0，2026-10-25）

| 命令 | 结果 |
|---|---|
| `npm run typecheck` | 通过（0 error） |
| `npm run lint` | 通过（0 error / 0 warning） |
| `npm run test` | 8 个测试文件、45 个用例全部通过 |
| `npm run rust:fmt` | 通过 |
| `npm run rust:clippy` | 通过（`--all-targets -- -D warnings`，无告警） |
| `npm run rust:test` | 55 单元 + 11 集成（一阶段）+ 20 集成（二阶段）+ 8 集成（三阶段，另 1 项 `#[ignore]`）+ 12 集成（四阶段）= **106 通过** |
| `npm run tauri:build` | 通过：`src-tauri/target/release/bundle/nsis/CreatorFlow_0.4.0_x64-setup.exe` |

历史结果：0.3.0 为 40 个前端用例、91 个 Rust 测试；0.2.1 为 36 / 72；0.2.0 为 31 / 65；0.1.0 为 23 / 38。

**性能测量（debug 构建，5000 条素材，5 个作品）**

| 场景 | 耗时 |
|---|---|
| 跨作品首屏（50 条） | 17 ms |
| 关键词搜索（`素材49`，111 条命中） | 19 ms |
| 按大小排序 | 23 ms |
| 末页（offset 4950） | 40 ms |
| 单作品列表 | 12 ms |

命令：`cargo test --test phase3_flow -- --ignored --nocapture`。数字为未优化 debug 构建，release 构建会更快；此处记录原始测量结果。

**回收站测试说明**：`deleting_work_*` 与 `deleting_asset_*` 会真实调用系统回收站，每次运行会在回收站留下几 KB 的临时文件。

**应用启动验证**：实际运行后成功创建 `%APPDATA%\com.creatorflow.desktop\creatorflow.db`（含迁移记录）、
`%LOCALAPPDATA%\com.creatorflow.desktop\logs\creatorflow.log.*` 与 WebView2 数据目录，日志中可见
「打开本地数据库」「已应用数据库迁移 version=1」。

**关于测试二进制的 Windows 清单**：Rust 集成测试可执行文件默认不带应用清单，Windows 会加载
comctl32 v5 并在启动时报 `0xc0000139`。`src-tauri/build.rs` 通过
`cargo:rustc-link-arg-tests` 为测试目标声明 Common-Controls 6.0 依赖，解决该问题；
应用本体由 Tauri 注入清单，不受影响。

## 4. 手动验收流程

前置：准备四个样本文件（`脚本.md`、`配乐.mp3`、`片段.mp4`、`封面.png`），建议其中包含一个 > 100 MB 的视频用于观察进度。

1. **首次启动**：应用打开，首页提示未设置作品根目录。
2. **设置根目录**：设置页 →「更改目录」→ 选择空目录（例如 `D:\CreatorFlow-Works`）→ 保存成功。
3. **创建作品**：首页 →「新建作品」→ 名称填 `我的第一支视频`，描述可选 → 选择目录后创建。
   - 磁盘上应出现 `D:\CreatorFlow-Works\我的第一支视频\{01_文档,02_音频,03_视频,04_图片}` 与 `creatorflow.manifest.json`。
4. **名称校验**：再建一个作品，输入 `CON` 或 `a/b`，应给出中文提示且不创建目录。
5. **导入单个文件**：进入作品 → 文档分区 →「导入文件」→ 选择 `脚本.md` → 确认对话框显示大小为 `.md`、目标分区「文档」→ 开始导入。
   - 队列面板出现任务，状态从排队 → 复制中 → 校验中 → 已完成。
   - 列表出现 `脚本.md`；`01_文档` 下出现同名文件；原始文件仍在原位置。
6. **拖拽导入**：把 `封面.png` 拖入窗口 → 确认对话框默认分区「图片」（按扩展名建议）→ 导入。
   - 图片分区的行点击「预览」能看到图片。
7. **同名不覆盖**：再次导入同一个 `脚本.md`。
   - 目标出现 `脚本 (1).md`；原 `脚本.md` 内容未被覆盖。
8. **大文件**：导入 > 100 MB 的视频，观察进度条推进、界面仍可切换分区与滚动。
9. **打开与定位**：对任意素材点「打开」（系统默认程序启动）、「定位」（资源管理器选中文件）。
10. **状态切换**：作品页把状态改为「制作中」，刷新后仍为「制作中」。
11. **重命名**：首页 → 重命名 → `我的第一支视频-改名`。
    - 磁盘目录同步改名，分区与文件仍在，manifest 中 `name` 已更新。
12. **重启恢复**：完全关闭应用 → 重新启动 → 作品列表、素材数量、状态、分区内容与关闭前一致。
13. **缺失检测**：在资源管理器中把 `01_文档/脚本.md` 改名 → 回到应用点「刷新」。
    - 该行显示「文件缺失」红色标记，记录未消失；顶部出现缺失提示。
14. **归档**：首页 →「归档」→ 默认列表不再显示；打开「包含已归档」可见，磁盘目录未被移动或删除。
15. **中断恢复（可选）**：导入大文件时用任务管理器结束进程 → 重启应用。
    - 应看到启动检查提示；该任务不在「复制中」悬挂，而是完成或失败可重试，`.part` 临时文件被清理。
16. **并发上限**：一次选择 6 个较大文件导入 → 观察任务面板，同时处于「复制中」的任务不超过 2 个。
17. **搜索与筛选**：在搜索框输入文件名片段 → 结果仅包含匹配项；切换「类型 / 状态 / 标签」筛选 → 列表随之收窄；点击排序按钮 → 顺序变化；清空筛选 → 恢复全部。
18. **分页**：把每页数量改为 20，导入超过 20 个文件后翻页，确认总数与页码正确。
19. **图片元数据与缩略图**：导入 `封面.png` → 列表出现缩略图与 `宽 × 高`；缩略图文件出现在 `%LOCALAPPDATA%\com.creatorflow.desktop\thumbnails\<作品ID>\`。
20. **音频元数据**：导入 `配乐.mp3` → 列表显示时长与采样率；若文件有内嵌封面，应显示封面缩略图。
21. **视频元数据**：导入 `片段.mp4` → 显示时长、分辨率、帧率。未安装 ffmpeg 时，视频分区顶部出现提示，且不生成封面（属预期）。
22. **标签**：在某一行的标签框输入「重点」→ 回车；再给另一行添加「重点」→ 应复用同一标签；在筛选栏选择「重点」→ 只剩这两条；移除其中一条 → 计数降为 1。
23. **重新定位**：把 `01_文档/脚本.md` 在资源管理器里改名为 `脚本-新.md` → 应用刷新后该行显示「文件缺失」→ 点「重新定位」选择新文件 → 关联成功且状态恢复「正常」。
    - 若选择内容不同的文件，应弹出「文件内容不一致」确认框；取消则不修改记录。
24. **归档与恢复**：把某作品改为「制作中」→ 归档 → 在首页打开「包含已归档」→ 点「恢复」→ 状态应回到「制作中」，归档时间清空；作品页顶部此时显示「该作品已归档」提示，可直接点「立即恢复」。
25. **删除素材**：在任一素材行点「删除」→ 确认框写明「移到系统回收站」→ 确认后该行消失、分区计数减一；打开系统回收站应能看到该文件，且可还原。
26. **删除作品**：首页在作品卡片上悬停，右下角出现删除图标（日期淡出）→ 点击确认后卡片消失，磁盘上的作品文件夹不再存在；系统回收站中应能找到整个文件夹。同样操作在「所有作品」表格的操作列也可完成。
27. **删除失败场景**：用播放器占用某个视频文件后再删除该素材 → 应提示「无法将文件移入回收站」，列表中的记录保持不变。
28. **迁移升级**：保留 0.1.0 的数据库直接安装 0.2.0 并启动 → 旧作品、素材、导入历史均在，日志中出现 `已应用数据库迁移 version=2`、`version=3` 与 `version=4`。

29. **全局搜索**：侧栏进入「全局搜索」→ 不选作品直接搜索某素材片段 → 结果列出多个作品的匹配项，作品列正确；点「打开作品」跳到对应作品与分区；对 `missing` 素材，「定位」为禁用。
30. **全局筛选组合**：选择作品 + 类型 + 标签 → 结果收窄且总数正确；清除筛选后恢复全部。
31. **数据库备份**：设置页点「立即备份」→ 列表出现新备份，状态「可恢复」，结构版本与当前一致；备份目录中应同时有 `.db` 与 `.db.json`。
32. **备份校验**：对同一备份点「校验」→ 提示结构版本与完整性问题均正常。
33. **恢复**：先新建一个作品，再恢复刚才的备份 → 应用自动刷新，新作品消失，原有作品仍在；备份目录出现 `pre-restore-*.db`。
34. **拒绝无效备份**：把一个文本文件改名为 `.db` 放进备份目录 → 刷新后状态为「不可用」，「恢复」按钮禁用。
35. **缓存管理**：设置页查看缓存占用与文件数；把上限改小后保存 → 提示生效值；点「清理全部缓存」→ 占用归零，回到作品页浏览，缩略图会重新生成。
36. **批量操作**：在素材列表勾选多条 → 出现「已选 N 项」；批量添加标签后所有选中项出现该标签；批量移除后消失；批量移入回收站后列表移除且系统回收站中有对应文件。

37. **工作台**：首页打开任意作品应直接进入「工作台」标签；核对完成率、总数/未完成/逾期、待办列表与最近素材；四个分区入口只出现在顶部标签，工作台底部不应再有重复卡片。
38. **新建任务**：工作台点「新建任务」→ 填标题、优先级、截止时间 → 保存后待办列表与统计立即更新。
39. **任务完成/重开**：任务页点「完成」→ 状态变已完成、完成率变化、completed_at 有值；点「重新打开」恢复。
40. **逾期**：把一个未完成任务的截止时间设为昨天 → 行内出现「逾期」标记，统计里的逾期数 +1；勾选「只看逾期」只剩它。
41. **任务排序与筛选**：切换「按优先级」「按截止时间」；在手工顺序下用「更多 → 上移/下移」调整顺序，刷新后顺序保持。
42. **模板新建作品**：首页「新建作品」→ 选择「播客 / 音频节目」→ 预览显示 5 个初始任务 → 创建后任务页应有 5 条待办。
43. **空白作品**：模板选「空白作品」→ 创建后任务页为空，工作台显示「尚无任务」而不是 0%。
44. **模板管理**：设置页「作品模板」→ 复制内置模板得到自定义模板 → 编辑其任务 → 设为默认 → 删除；内置模板的「编辑/删除」应为禁用。
45. **模板隔离**：修改自定义模板的任务后，之前用该模板创建的作品任务数量不变。

## 5. 已知未覆盖

- 无 Playwright/Tauri 端到端自动化：当前以手动验收步骤 + 单元/集成测试覆盖。
- 未覆盖真实磁盘写满、只读目录、网络盘断开等场景的自动化测试。
- 视频封面生成依赖本机 ffmpeg，未在自动化测试中验证（无稳定测试样本）。
- 回收站被文件占用而失败的分支未自动化（需要独占打开文件），只做了手动验收步骤。
- **原生桌面 E2E 不可用（0.3.0 与 0.4.0 各实测一次）**：`tauri-driver 2.1.0` 可安装、`msedgedriver 155.0.4283.62` 与 WebView2 版本匹配、`POST /session` 能返回 sessionId，但应用在自动化模式下没有创建 `msedgewebview2.exe`，随后命令全部返回 `invalid session id`。浏览器 mock 截图只验证前端渲染，**不能**视为桌面端验收通过。
- 备份恢复的「切换文件失败」「迁移失败回滚」分支只有代码路径与设计说明，未能在自动化中构造（需要注入文件系统故障）。
- 原生窗口内容无法自动截图（WebView2 合成层限制），界面验证使用浏览器渲染截图。
