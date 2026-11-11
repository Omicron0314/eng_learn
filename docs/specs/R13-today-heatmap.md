# R13 · 今日与学习热力图

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G3。依赖：R07、R09、R11、R12。

## 目标与范围

query_today(now)→queue、metrics、books、inboxPreview、weeklyActivity、heatmap；使用统一统计规则。

设计追踪：screen-today、screen-dark、screen-mobile首页。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：首页按最终设计展示复习、指标、词书与辅助栏，不含生成器或故事列表。
- **AC2**：开始复习数与实际可取队列一致；零队列、无词书、等待到期各有合理入口。
- **AC3**：词书带筛选跳词库，生词预览与badge同步，手机保留复习／整理／热力图。
- **AC4**：13周×7格按学习日聚合，未来格不可选，鼠标／键盘可看日期与计数。
- **AC5**：浅深主题和窄屏布局与稿对照，截图由 CI 产出；活动数字来自事件而非示例常量。

## 验证与交付

固定日界／DST／未来格数据、跨视图对账、浅深1440与412截图、空库流。

## 边界与风险

audio_play不代表听完；旧数据没有completion时应少计而不编造。
