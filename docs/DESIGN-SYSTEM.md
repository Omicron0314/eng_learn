# 词间设计系统

目标是复现 [cijian-m3](cijian-m3.html) 的 Material 3 Expressive 语言。产品 token 覆盖通用库默认值；不因组件库默认主题不同就改稿。组件行为复用 Kit，业务组合与品牌视觉留在词间 UI 层。

## 色彩角色

以下直接来自 `.m3` / `.m3.dark`。在 `cijian-ui` 的主题映射统一维护，业务视图只引用语义角色。

| 角色 | 浅色 | 深色 | 用途 |
| --- | --- | --- | --- |
| primary / onPrimary | `#4b49c8` / `#ffffff` | `#c3c0ff` / `#1f1a8a` | 主操作、焦点 |
| primaryContainer / on | `#e2dfff` / `#2c2891` | `#3a36ab` / `#e2dfff` | 今日重点、选词 |
| secondary / onSecondary | `#5d5c72` / `#ffffff` | `#c6c4dd` / `#1a1a2c` | 选中按钮的对比文本；on 为补充 |
| secondaryContainer / on | `#e2e0f9` / `#1a1a2c` | `#45455a` / `#e2e0f9` | 导航选中、tonal |
| tertiary / container / onContainer | `#8a4a60` / `#ffd8e4` / `#3e1020` | `#efb8c8` / `#6b3a4b` / `#ffd8e4` | AI、生成 |
| error / container / onContainer | `#ba1a1a` / `#ffdad6` / `#410002` | `#ffb4ab` / `#93000a` / `#ffdad6` | 重来、失败 |
| success / container / onContainer | `#246b3a` / `#b4f1c0` / `#00210b` | `#8dd89b` / `#0f5223` / `#b4f1c0` | 轻松、完成 |
| warning / container / onContainer | `#7a5900` / `#ffdf9e` / `#261a00` | `#f5bf48` / `#5c4300` / `#ffdf9e` | 困难、提醒 |
| background | `#f4f1fa` | `#121218` | 应用背景 |
| surface c0 / c1 / c2 | `#ffffff` / `#f9f6fe` / `#f0ecf6` | `#0e0e14` / `#1c1b22` / `#201f27` | 卡片、分区、次级背景 |
| surface c3 / c4 | `#eae6f0` / `#e4e1ea` | `#2a2931` / `#35343c` | 控件、弹层层次 |
| onSurface / variant | `#1c1b22` / `#47464f` | `#e5e1ec` / `#c8c5d0` | 正文、次要文字 |
| outline / variant | `#787680` / `#c8c5d0` | `#928f9a` / `#47464f` | 边框、分隔 |
| inverse / onInverse | `#313038` / `#f3eff7` | `#e5e1ec` / `#313038` | Snackbar |

明暗主题均逐角色映射，不能把亮色常量复制到暗色状态。画板少数硬编码白字／透明白层只作为参照；暗色可读性由语义 on-color 和对比度验收补足。颜色不是唯一状态提示，必须有文字／图标。

## 排版、尺寸与形状

- 英文／标题：Roboto Flex；中文：Noto Sans SC／系统中文回退；数值、端点、快捷键：Roboto Mono。字体来源见 [licenses](licenses.md)。嵌入完整授权子集或可用系统字体，不依赖在线字体请求。
- Display 52/58，字重 750；H1 40/48；H2 24/32，700；H3 20/28，700；普通正文 16/24；辅助 13/18、12/16；英文阅读 20/34，窄屏 18/30。
- 宽体标题目标宽轴约 106–115%；若所选引擎不支持字体变轴，使用静态字体实例并记录差异，不能仅加大字重假称等效。
- 参考桌面画板 1440×900；移动 412×892。尺寸用逻辑 dp，DPR 不改变布局值。画板固定高度不是实际应用的裁切高度，长页面应滚动。
- 96dp rail；区块间距 12–24；pane 圆角 28，card 24，主卡 32，复习卡 40；弹窗 32；不对称叶片／水滴用于少量重点。
- 按钮可视高度 40 / 56 / 64，触摸目标至少 48×48；IconButton 可视 40，紧凑控件扩大 hit target 并检查不重叠。
- 连接式按钮组外角 20–32、内角 6–10、间隔 2–4。Good 的视觉权重大于其他评分，但键盘与触控能力相同。
- 阅读栏目标宽约 700dp；释义侧栏 384dp，编辑栏 340dp；小窗口不强撑固定宽度。

## 组件对应

名称来自上游公开文档，具体构造 API 在 R00 锁定的 revision 验证；此表不是可直接复制的 Rust 调用。

| 词间组合 | 优先复用 |
| --- | --- |
| 导航轨／底栏与自适应 | NavigationRail、NavigationBar、NavigationSuite、PaneScaffold |
| 主按钮／图标／评分和难度组 | MaterialButton、IconButton、ConnectedButtonGroup |
| 搜索／表单／下拉 | SearchField、TextField、ExposedDropdown |
| 暂停／模式／筛选 | MaterialSwitch、MaterialRadioGroup、FilterChip |
| 数值／长度范围 | MaterialSlider；验证 range 与输入组合 |
| 语境／编辑面板 | MaterialSideSheet、MaterialBottomSheet |
| 导入／删除／反馈 | MaterialDialog、MaterialCheckbox |
| 播放器 | Toolbar floating＋应用 AudioPlayer 状态 |
| 信息／进度 | Badge、Snackbar、LoadingIndicator、LinearProgress；复习头部与音频时间轴用波浪进度条（`design::wave_progress`） |
| 页面卡片／列表 | MaterialCard、ListItem；大列表接 Kit 虚拟化 |
| 热力图／覆盖率／波形 | 应用专用绘制；复用色彩、文本、焦点和可访问语义 |

不为“看起来像 M3”重新实现输入法、焦点、菜单或滚动。上游存在的行为缺口要以复现记录和测试说明，再做最小适配。正文词命中需同时支持文本选择、滚动与键盘访问。

## 动效与可访问性

使用 Expressive motion 表达展开、选中和状态切换；减少动态效果时关闭波形摆动、形变与装饰性循环，保留清晰状态。没有变化的窗口不持续请求帧；等待网络的进度动画限可见区域。

复习头部、先听卡片与阅读工具栏共用一条波浪进度条（`design::wave_progress`）：已完成的区间画成流动的正弦波，未完成区间保持 4dp 直线，尾端圆点标示轨道终点。波浪只表示**进度**，不表示音频采样，因此不与“波形不作播放定位”冲突；先听卡片不再画装饰柱状波形。波浪摆动是装饰：队列做满、音频暂停或停止时传入 `moving = false` 直接画静态帧，不再请求帧；摆动上限 30fps；系统要求减少动态效果时由 `AnimationExt` 渲染起始帧并不排帧。轨道颜色成对传入（`WaveColors`），落在 `PRIMARY_CONTAINER` 上的卡片用 on-container 色，免得次级容器色在该背景上消失。

焦点边框至少与背景可辨；正文对比度目标 4.5:1，大文字与非文本控件 3:1。支持 100–200% 文本缩放、Tab／Shift+Tab／Escape，图标有中文可访问名称，图表提供可读数值摘要。中文 IME 与读屏不在本轮 AC（[D54](DECISIONS.md)）；可访问名称与焦点行为仍需自动化验证。

视觉验收使用固定原创数据，覆盖浅／深、1440×900／1024×768／412×892／360×800、长中文词书、长英文词、空列表与大字体。基准来自原设计的最终 DOM，按区域对照布局和层级，不把不同平台字体抗锯齿作为业务失败。
