# R22 单词发音与词包音频

依赖：R06（词库导入）、R07（复习队列）、R12（音频与额度）。

## 目标

复习时能立刻听到当前词的真实英式或美式发音，音标由来源提供而非模型编造；
发音随词书成套分发，用户下载哪本词书就得到哪本的音频；无网络时不阻塞复习。

## 验收条件

- **AC1**：复习卡在词下方显示 `口音按钮 + 音标`，默认美式；点音标重播当前口音，点按钮在英／美之间切换并播放切换后的口音；两者都不触发翻面。
- **AC1c**：设置 ·“学习偏好”提供发音类型（英式／美式）与自动发音开关，写入本机设备偏好并立即生效；关闭自动发音后换卡不再朗读。
- **AC1b**：新词卡出现时自动朗读当前口音一次；同一张卡翻面、切换口音或重绘都不再自动重播，没有录音的卡静默且不重试。
- **AC2**：没有音频时按钮与音标不可点并明确显示“暂无”，卡片其余功能不受影响；有一个口音缺失时按另一个口音兜底，不静默失败。
- **AC3**：发音数据按 `(档案, 词元, 口音)` 唯一，重复补齐不产生重复行；同一字节内容只占一份磁盘。
- **AC4**：播放单词不改变文章播放状态、不写 `audio_sessions`、不计入阅读／听力完成，也不消耗远程 TTS 额度。
- **AC5**：音标取不到时留空，界面不显示占位 IPA；不调用模型补写音标。
- **AC6**：备份包含词音频时同时导出 `word_audio` 行与对应 blob；不含音频的备份不携带该表；v3 及更早备份仍可恢复。

## 已实现

- 迁移 `0007_word_audio.sql`：`lexemes.ipa_uk/ipa_us`、`word_audio` 表、`card_view` 视图。
- 领域 `Accent`／`Pronunciation`（含兜底与校验），端口 `CardView.pronunciation`、`WordAudioAsset`、`SaveWordAudio`。
- 存储：`words_missing_audio`（可断点续补）、`word_audio`、`save_word_audio`（音标只填空值，不覆盖已学内容）。
- 平台：`AudioCommand::PlayWord` 使用独立 sink；应用 `Command::PlayWord` → `Response::WordAudio`。
- UI：复习卡发音行（`review-ipa`／`review-accent`）、换卡自动朗读（`announce_review_card`）与设置页“学习偏好”卡片（`settings/pronunciation.rs`），测试覆盖点播、切换、“每卡只读一次”与“关闭自动发音后静默”。
- 维护工具（需先关闭应用，命令独占学习目录）：
  - `cijian-fill fill --data-dir <目录> [--limit N] [--interval 毫秒] [--dry-run]` 补齐录音与音标
  - `cijian-fill pack --data-dir <目录> --output <词包> ` 生成 `.cijianpack`
  - `cijian-fill import-pack --data-dir <目录> <词包>` 无头导入（应用内导入对话框等价）
- 无头验证：`apps/desktop/src/backend/e2e_tests.rs` 覆盖“打包 → 导入 → 复习队列带双口音与音标”和“损坏包整包拒绝”。

- 偏好：`DevicePreferences.pronunciation_accent`／`review_auto_play`（迁移 `0008_device_pronunciation.sql`），命令 `SetPronunciationAccent`／`SetReviewAutoPlay`，均为 quiet，不占忙碌状态。

## 代码缺口

- **体积压缩**：直接保存来源 MP3（实测 10–22 KB/词/口音，3070 词双口音 125 MiB）；
  若需压到 20 MB 量级，需在打包阶段转码为低码率单声道并在播放侧确认解码支持。
- **音标来源单一**：目前只有 `jsonapi` 一个来源；词书自带音标仍在导入时被丢弃，
  接入后应优先使用词书自带值（来源优先级需要在实现时确定并写进 D 决策）。

## 风险

- 有道接口未公开文档化，没有可用性承诺；返回格式不稳定（实测同一个词换 `type` 会返回裸 WAV）。
  调用点集中在 `crates/cijian-providers/src/youdao.rs`，换源不影响存储与 UI。
- 商业分发该音频的条款尚未确认；公开仓库与买断发行前必须核实，或替换为自有 TTS 产物。
- 音频是派生数据，因此 `save_word_audio` 不写事件；删除词元不级联删除 blob，孤儿文件由后续清理动作处理。
