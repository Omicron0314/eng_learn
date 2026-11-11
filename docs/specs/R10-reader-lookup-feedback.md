# R10 · 阅读、语境释义与反馈

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G2。依赖：R09。

## 目标与范围

open/read/complete session、lookup_word(articleId,bodyHash,range)、mark_word、submit_feedback、flag_sentences；所有句子位置绑定bodyHash。

设计追踪：screen-read、screen-flag、screen-mobile点词。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：正文与存储一致，阅读／先听切换和返回文章保持位置；桌面side sheet、窄屏bottom sheet符合稿。
- **AC2**：点词显示真实句／lemma／可用发音信息；快切词不被旧响应覆盖，缓存key含版本／上下文。
- **AC3**：认识／不认识／加入复习按模式工作，缺释义仍可标记，重复建卡与暂停／归档处理明确。
- **AC4**：阅读完成会话幂等，反馈按阅读／听力分别保存最后值，重新打开能恢复。
- **AC5**：不自然反馈支持多句与200字符备注，非法range拒绝；失败保留输入、提交去重。
- **AC6**：keyboard 焦点与文本选择不冲突，关闭sheet回原词，文章不存在有返回入口。

## 验证与交付

范围校验、UTF转换、缓存命中、竞争响应、markOnly/autoAdd事件、反馈UI和长正文滚动。

## 边界与风险

发音须通过平台能力接口；R12未就绪时只能显示明确不可用，最终桌面验收要补齐。
