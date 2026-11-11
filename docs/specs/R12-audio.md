# R12 · 音频、朗读与费用护栏

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G2。依赖：R10、R05。

## 目标与范围

ensure_audio、AudioPlayer、SystemSpeech、PlaybackCapabilities、audio sessions；远端音频＋本机朗读两路径。

设计追踪：screen-listen、screen-audio-states、browser-audio、悬浮播放器。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：首次明确生成、生成中禁重复、失败可重试／本机朗读，已有缓存先命中不扣额度。
- **AC2**：缓存key区分voice/model/endpoint/format；缺blob重新合成先预留；音频预留并发不超额。
- **AC3**：调用前预留全文字符，部分失败／取消后状态不确定／存储失败不退额度；0额度仍播缓存。
- **AC4**：整篇播放支持seek、后退5秒和五档速度；本机按capability显示，禁用不支持的操作。
- **AC5**：切文章／切来源不会两路播放，速度恢复，先听原文默认收起，阅读工具栏不遮挡内容。
- **AC6**：自然结束会话去重并显示听力反馈，停止不记完成；read/listen反馈独立。

## 验证与交付

FakeTTS、本地 WAV 样例、播放器状态机、并发预留／崩溃blob、适配器测试；人工听感不在 AC，CI 只验证解码与播放状态机。

## 边界与风险

不能将静音stub当音质验证；系统voice可能不存在，浏览器背景播放限制单列。
