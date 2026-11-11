# R06 · 词库、导入与词书管理

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G2。依赖：R02、R03、R04。

## 目标与范围

import_words(file, kind, target, operationId)→ImportReport；query_cards(filter,page)、edit_card、suspend_card、delete_book(previewRevision)。

设计追踪：screen-library、screen-import、screen-import-result、删除确认。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：类型／用途／目标词书／选文件／提交按顺序工作；未选文件禁用，取消不导入。
- **AC2**：CSV/TXT/APKG、中文字段别名、短语／重复、基线不建卡、同名同类合并符合SPEC-CORE。
- **AC3**：返回全部设计计数与行错误，合法行可导入，数据库异常整批回滚；重复不覆盖用户释义。
- **AC4**：词书／状态／暂停／搜索筛选能组合，列表有总数与分页；编辑保存／取消与草稿保护有效。
- **AC5**：删书预览区分共享卡、无历史卡和有历史卡；归档卡不通过普通开关恢复。
- **AC6**：大文件在后台流式处理，非法压缩与资源不足有错误，UI不冻结；归一化批次峰值内存有固定上限并由 CI 记录；重启后导入结果保留。

## 验证与交付

原创CSV/APKG种子、字段回归、事务故障、1万词列表、导入与编辑UI链路。

## 边界与风险

不含完整Anki调度／媒体导入，不分发用户词书；虚拟化不能导致编辑错行。
