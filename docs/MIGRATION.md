# 重构与数据迁移

实现已获授权；预检、转换、备份与隔离恢复工具已建立，只用原创临时库验证。没有打开真实学习库、读取真实密钥或执行真实数据迁入。旧代码保留至对应新端验收通过。

## 资产处理

| 现有资产 | 处理方式 |
| --- | --- |
| `packages/core` | 抽取纯规则／固定向量，在Rust重实现并做差异测试；不把TS运行时嵌进原生产品 |
| `packages/db` | schema、迁移、仓储作为输入格式事实；旧迁移冻结，新库独立迁移序列 |
| `packages/providers` | 复用协议行为、失败回归、成本计数要求，Rust适配器重写 |
| `apps/web/src/server/services` | 用例与历史修复作为语义参考，最终由cijian-app承接 |
| `apps/web` UI | 保留可运行对照；新UI按设计稿重建，不直接移植DOM/CSS布局 |
| `prompts/*.v1.md` | 原样保留并嵌入新程序；语义变更另建版本与评测 |
| `fixtures` | 保留原创样例，扩充跨语言向量；不复制私有词书 |
| `tools` 与现有CI | 维持旧实现验证；Rust工具到对应任务逐步建立 |
| `docs/cijian-m3.html` | 新功能／UI基线，原文件不改 |
| `docs/neko.html` | 辅助风格参考，冲突时词间稿优先 |
| 旧文档／旧spec／spike | 保留在 git 历史（提交 46f499a），不再维护副本；新有效规格用R编号 |

## 迁移阶段

1. **R00–R03 建基线**：用原创种子库／导出向量验证FSRS、时间、词形与事件；锁定新schema与转换器版本。真实用户库此时不参与测试。
2. **R04–R15 做纵向闭环**：从导入到复习、文章、点词、整理、音频、统计。每一块都有独立测试，旧Web持续可运行。
3. **R16 做显式迁入／备份工具**：预检、只读快照、转换到新文件、验证、汇总报告。用户选中源文件后才操作。
4. **R17 对照验收**：新程序重启后队列、释义、文章与日志一致，失败能回到旧版本。达标后再决定何时归档旧代码。

## 旧库 → 新库映射

| 旧表／字段 | 新表示 | 规则 |
| --- | --- | --- |
| users.id | profiles.id | 保留ID；不创建云账号 |
| cards.deck | word_books＋book_entries | 按profile和有效名称建立学习词书，关联词；卡ID／调度不变 |
| word_lists / word_list_items | word_books / book_entries | 保留baseline/study用途，和同名同类deck合并，报告冲突 |
| cards.back_json | meaning/example/text_source | 按实际版本解析，不用空值覆盖已有内容 |
| review_logs | review_logs＋legacy metadata | 保留时间、rating、状态、due与原ID；旧记录缺少after字段则nullable，不能编造 |
| events | legacy版本events | payload原样保留，迁移本身另追加一个事件 |
| articles / article_targets | articles / article_targets | 保留body、model、prompt_version、分析快照；补body hash和legacy实验版本 |
| inbox / lexeme_marks | 同域对象 | pending去重有冲突时保留全部原记录，按确定规则投影并报告 |
| audios / blobs | audio_assets＋links | 校验存在性与hash；旧缓存缺model／endpoint指纹则标legacy，仅供已链接文章播放，不命中新配置合成缓存 |
| provider_config | provider_configs＋SecretStore | 非敏感配置可迁，明文key只在显式迁移凭据步骤写系统存储；不可用则要求重输 |
| settings | settings＋device_preferences | 合法保存值优先；新键用新默认，不重置旧长度／重试值 |
| gloss_cache / llm_calls | legacy缓存／调用记录 | 保留可读字段；旧调用的自由格式error文本去除，避免历史错误携带凭据。旧缓存缺新key维度时不冒充新缓存命中 |

旧 cards.suspended 不能证明“删书后永久暂停”。只有可靠的 deck_delete／card事件可以判定归档；无法判定的卡保持暂停并在报告中标歧义，不能自动恢复。未知枚举、损坏JSON、重复ID与不兼容FSRS状态是转换错误，不静默丢行。

## 一次迁入流程

- 用户选源库／音频目录；当前转换器以只读SQLite事务固定一致性快照，包含WAL。provider仅选择非敏感字段，不把含明文key的整库复制到临时文件；直接复制主文件而遗漏WAL不合格。
- 检查已知schema版本、字段、外键、完整性和目标空间，计算source fingerprint；旧格式没有保存迁移checksum，报告明确说明，不能声称验证了不存在的校验和。先在内存目标运行同一转换器进行dry-run。
- 转换到临时的新库，所有profile数据和日志保留映射；provider密钥不进报告、临时明文导出或事件。
- 检查各实体计数、外键、评分总数、时间边界、调度字段、body hash、blob checksum，记录差异清单。
- 验证通过后原子发布为新profile；失败只清理本次临时产物，源库和原音频不改。
- 保存legacy_imports记录，同源／同映射版本再次执行返回既有结果，不能重复评分与额度事件。

配额从旧audio_reserve与旧成功事件确定性归并，关联reservation的success不重复计数；跨日设置变化按新口径计算但保留原时刻。无法归因的旧成功记录保守计入并标注。

## 回退与备份

新程序使用新数据目录，不与旧Web同时写同一个SQLite文件。回退到旧程序时使用原数据快照；新程序产生的学习增量**不会自动出现在旧Web**，需要额外导出转换，首轮不承诺双向回退同步。

当前v3逻辑备份包括格式版本、profile、稳定JSON checksum、领域数据和历史迁移档案；用户可选择携带经逐个SHA-256校验的音频缓存，恢复时写入新库并在失败时不发布；v1/v2包仍可恢复。备份始终不含密钥、活动配置或未完成job。恢复先预检到新profile，不覆盖当前profile。坏包、高版本、非空目标、重复迁入、缺文件、事务失败和无覆盖发布已有测试；真实磁盘写满、进程中断矩阵仍需补齐。备份／恢复由用户明确发起，不在启动时自动导入。
