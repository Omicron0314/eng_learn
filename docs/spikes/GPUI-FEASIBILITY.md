# GPUI与Material可行性：资料核实

核实于2026-11-02，仅阅读公开上游文档；没有clone、安装依赖、编译、运行原生程序或验证远程CI。资料通过TinyFish获取，引用事实采用自己的摘要，未复制上游实现。

## 来源

| 来源 | 本次读到的事实 | 对词间的影响 |
| --- | --- | --- |
| [GPUI官网](https://gpui.rs/) | Rust GPU UI框架，官方介绍与API入口 | GPUI是渲染／交互基础，不能从官网示例推导全平台产品能力 |
| [gpui-kit-material README](https://github.com/mizorewww/gpui-kit-material) | 基于Longbridge Kit，Material提供视觉／动效；pre-release 0.1，组件未达到生产标准 | 可采用，但必须逐组件／逐平台验收 |
| [Material架构](https://github.com/mizorewww/gpui-kit-material/blob/main/docs/ARCHITECTURE.md) | Kit拥有行为／焦点／窗口；使用facade保证引擎类型一致；消费者需要root patch集 | R00从消费者工程验证，不只在上游workspace运行gallery |
| [Material覆盖审计](https://github.com/mizorewww/gpui-kit-material/blob/main/docs/COVERAGE.md) | 47行中45实现、无组件生产就绪；缺独立审核、视觉证据、性能与真机触摸／读屏记录 | “有组件”不等于“词间的控件无需测试” |
| [GPUI Kit README](https://github.com/longbridge/gpui-kit) | 声明macOS/Windows/Linux和WASM，facade重导出GPUI/Base/Component；支持测试harness和AccessKit | 桌面与WASM有工程路径，原生手机需另验 |

来源是可变main页面的访问时快照，不是本仓库锁定版本。README当时示例Kit revision为`bb85882e1faab845d9e7be33b69a152879a90f69`、Material revision为`3df2920f17cecb765c96f23073fff5e4a2773e86`；仅作为查阅定位，不表示本项目已验证或采纳。R00需确认这些commit及其依赖一致，再记录最终完整revision。

## 消费者接入要求

上游说明依赖与patch要统一：`gpui-kit`、`gpui-base`、`gpui-pre`／`gpui-fast`及相应apple／web crates；patch只在workspace根生效。补丁涉及输入光标／IME、字体回退、主题、控件与渲染行为。只拷一个`gpui-kit-material`依赖可能出现不一致类型或行为缺口。

Material默认启用Fast，Kit另有其默认后端；应用要明确选取，不混用独立gpui依赖。字体、图标和gallery工具不是全部自动属于库的产品资产，应分别决定并查许可。

## 设计所需能力

| 能力 | 上游公开入口 | 尚需验证 |
| --- | --- | --- |
| 导航与面板 | NavigationRail/Bar/Suite、PaneScaffold | 六项紧凑导航、小窗口焦点、滚动 |
| 评分／难度组 | ConnectedButtonGroup、Button | 中文标签、不同权重、键盘／触摸 |
| 查词／编辑 | SideSheet、BottomSheet、Dialog | 不遮阅读、可访问性、IME与软键盘 |
| 文章词命中 | GPUI文本与应用自定义布局 | UTF-8范围、选词／复制共存、长文性能 |
| 输入与滑块 | TextField、Slider、Radio、Switch | 中文、双端范围、150/200%缩放 |
| 图表／热力图 | 自定义绘制／Kit能力 | 数据语义、可访问数值、交互与帧预算 |
| WASM | 共享gallery-web | 真实手机浏览器、存储、音频、CORS、离线 |
| Android/iOS原生 | 本次材料未确认 | 后端、构建、生命周期和全部平台适配 |

## 已知限制与结论

上游覆盖审计明确指出输入类型不能保证浏览器软键盘inputmode匹配；触摸系统取消有边界；跨组件字体放大、实际读屏、Linux/Windows gallery运行和WASM一致性仍缺记录。词间需有自己的最小消费者与功能验收，不能继承库的宣传指标。

结论：采用用户指定栈规划桌面与Web/WASM合理；Android/iOS原生作为条件式路线，R19得到明确结果才推进R20。本次资料阅读完成的是风险识别，不是R00原型任务完成。
