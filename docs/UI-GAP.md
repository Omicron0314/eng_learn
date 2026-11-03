# UI 现状与设计稿差距（交接待续）

用户要求界面最终与 `docs/cijian-m3.html` 一致。当前 Rust/GPUI 功能链路基本可用，但视觉仍是功能性堆叠布局，差距很大；本文件只记录目标和下一步，不代表完成。

## 目标画板

- 原始画板：`docs/cijian-m3.html`。画板帧固定 `1440×900`；移动端为 `412×892`。
- 设计 token、字号、圆角、按钮/卡片/导航样式在 HTML 的 `.m3` 样式段（约 208–380 行）。
- 已验证的截图目标可用本地服务器＋Playwright 生成：
  `python3 -m http.server 8765 --bind 127.0.0.1`（在 `docs/` 下），打开
  `http://127.0.0.1:8765/cijian-m3.html`；隐藏 `.viewer` 后截取 `#screen-* .frame`。
- 字体：HTML 内嵌 Roboto Flex / Roboto Mono WOFF2。已解出 `/tmp/font-RobotoFlex.woff2`、`/tmp/font-RobotoMono.woff2`，并转成 TTF；Rust 侧需随二进制加载或放入受许可证约束的 assets，中文回退 Noto Sans CJK SC。

## 目标结构

- 左侧 96px 导航轨：顶部非对称圆角“词”logo，六项主入口（今日/复习/文章/词库/生词箱/统计），设置固定底部；选中项为 `secondary-container` 胶囊图标。
- 主区为 28px 圆角浅色 pane，Today/Stats 再分主栏和 280px 辅助栏；内容间距 20–24px。
- 关键组件：紫/粉色 tonal hero、24px 卡片、32/40px 大标题、连接式分段按钮、圆角 chip/tag、24–28px 大按钮、悬浮播放器、右侧 side sheet、移动端底栏与 bottom sheet。

## 当前差距（按优先级）

1. **Today**：现为“一张大卡 + 全宽按钮”；目标是问候标题、紫色 Today Review hero（17 大数字/三类数量/大按钮/快捷键提示）、三张指标卡、三张词书卡，以及右侧生词箱、本周节奏、13 周热力图。
2. **Shell/导航**：现有 rail 只有图标和文字，缺少 logo、选中胶囊、badge 位置和设计间距；页面缺少 pane 背景与主辅双栏。
3. **Review**：现为简单正面/按钮列；目标是顶部进度+关闭、左侧大卡（词/释义/例句）、四段彩色评分按钮、右侧队列/快捷键/无释义提示。
4. **Articles**：现为按钮列表；目标是左侧生成面板（难度分段、体裁 chip、生成按钮）和右侧文章卡片列表。
5. **Library**：现为搜索+按钮堆叠；目标是左侧词书筛选、中间表格、右侧编辑 side sheet。
6. **Inbox**：现为纵向行；目标是 3 列上下文卡片、顶部多选操作栏、来源链接。
7. **Stats**：现为文字列表；目标是指标卡、覆盖率折线、粉色对照实验、每周活跃、词表分页。
8. **Settings**：现为单列表单；目标是左侧分类 rail + 右侧分区卡片、滑块/单选卡、底部保存栏。
9. **Reader**：已有正文/侧栏基础，但缺少设计中的顶部模式分段、文章 meta、行内词高亮、底部悬浮播放器、反馈分段和右侧语境卡样式。
10. **Mobile**：现有 360 布局只有底栏折叠；目标 412 画板的 hero、指标、热力图、复习大卡和点词 bottom sheet。

## 实施约束

- 保留现有命令、状态机和测试用的稳定 ID（如 `start-review`、`search-library`、`save-settings`、`article-body`、`word-sheet` 等），视觉重构不要破坏行为测试。
- 颜色使用 `crates/cijian-ui/src/theme.rs` 与 `ColorRole`（与画板 token 基本一一对应）；不要另造一套硬编码颜色。
- 组件优先用 Material（按钮/对话框/sheet/输入）但版式、字号、间距、层级需按画板重写；不要继续堆默认全宽按钮。
- 字体文件涉及 OFL，保留许可；不要提交用户原有 Web/provider 改动或暂存重命名。

当前功能实现与验证记录见 `docs/IMPLEMENTATION.md`；HEAD 之后的视觉工作应单独原子提交，并在提交前补充 1440/412、浅深色截图对照。
