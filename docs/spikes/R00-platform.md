# R00 平台验证执行记录

消费者及复现命令见 [原型README](../../spikes/r00/README.md)，任务状态仅在 [BACKLOG](../BACKLOG.md)。当前为进行中，以下分别记录通过和未证明的能力。

## 依赖与环境

Linux x86_64、niri/Wayland。Rust **1.99.1** 和 wasm32-unknown-unknown 为本机已有工具链。Material `2e13169fceb4ac328e1361aa143632025598872f`，原Kit `bb85882e1faab845d9e7be33b69a152879a90f69`。Material当前revision要求1.99.1，不能采用本机原默认1.97.1。

[根manifest](../../spikes/r00/Cargo.toml)包含上游消费者九项patch：crates.io的gpui-base、gpui-pre、gpui-pre-apple、gpui-pre-web、gpui-fast、gpui-fast-apple、gpui-fast-web，以及Kit Git源的gpui-kit、gpui-base，统一到Material revision。应用只用gpui_kit facade，显式Fast，锁文件无相邻checkout路径依赖。

`python3 spikes/r00/check-dependencies.py` 通过：Kit/Base/Pre/Fast各一个锁定包、九项patch来源一致、无外部路径依赖。真实源码构建不是上游workspace内构建。

## 实际证据

| 验证 | 结果 |
| --- | --- |
| 原生build / clippy | 原型已通过；原生中文窗口实际运行 |
| WASM | check、build、wasm-bindgen 0.2.130生成浏览器模块，Headless Chromium 156实际显示；当前浏览器无可用WebGPU，回退WebGL2，subpixel抗锯齿不可用 |
| 浏览器主题/sheet | 真实canvas点击切换深色，打开释义sheet，Escape关闭并显示触发按钮焦点；不是DOM替代UI |
| 字体 | 中文fallback可显示；发现全角标点缺字后添加有授权且改名的Noto标点子集，修复后的sheet截图显示完整标点 |
| SQLite | rusqlite 0.38.0 bundled；临时库业务/事件/回执提交、事件失败回滚、重开、两个连接版本冲突通过 |
| IndexedDB | 随机测试库中业务/事件/回执提交、事件重复失败整批回滚、重开保留通过；finally删除测试库 |
| CORS | 本机双端口fixture：无允许头被拒绝，允许来源返回成功；不调用真实provider |
| 原生音频 | rodio 0.21.1打开默认输出流；2秒原创静音buffer seek至1秒成功、倍速1.25；未测试可听质量 |
| 文件选择 | rfd 0.15.4返回取消，无文件被读取；仅证明API返回取消，未证明真实文件选取成功 |
| 浏览器朗读 | SpeechSynthesis存在，voice列表为空，须呈现不可用状态 |
| 原生系统朗读 | 未找到spd-say/espeak-ng；当前未证明可用英语voice |
| 系统凭据 | D-Bus可见org.freedesktop.secrets服务；未访问任何凭据，也不以服务存在宣称读写已通过 |
| FSRS | fsrs 6.6.2使用旧参数；16种原创状态/间隔×四档共64组记忆状态通过，容差0.0001。正式domain另完成due、状态、步长、次数等全字段对照 |

截图：[Linux浅色](evidence/r00/linux-light.png)、[WASM浅色](evidence/r00/wasm-light.png)、[WASM深色](evidence/r00/wasm-dark.png)、[WASM sheet](evidence/r00/wasm-sheet.png)。前两张早期WASM主题图含修复前标点缺字，用于保留问题证据；sheet为修复后。

固定输入：[TS向量](../../spikes/r00/fsrs-vectors.json)；[Rust记忆状态对照](../../spikes/r00/fsrs-comparison.json)；[正式调度全字段测试](../../crates/cijian-domain/tests/legacy_vectors.rs)。不读取旧用户卡片。原始运行日志位于 `/tmp/cijian-r00-*.log`，临时路径不作为永久证据链接。

## 逐项AC与剩余限制

- AC1：消费者构建、工具链、patch、版本唯一性已验证。
- AC2：原生中文绘制、输入框和主题/字体/sheet控件存在；真实IME composition仍未通过。wtype拼音尝试显示的是拉丁字母，不能冒充中文输入法成功。选词、复制、字体缩放和原生焦点验收继续进行。
- AC3：WASM实际运行、IndexedDB原子事务、CORS探针已完成；浏览器IME、真实GPU/移动浏览器与重载持久化完整产品合同尚未完成。
- AC4：候选库与固定向量验证已具备，领域适配沿用旧步骤和间隔顺序，不重置状态。更多旧库样例在迁移任务验证。
- AC5：SQLite和静音音频接口已验证；文件选择取消路径、系统语音无voice、凭据仅服务检测均按真实范围记录，其他操作未验证。
- AC6：工程候选确定；性能、触摸、读屏、Windows/macOS/移动原生仍未验证。不能将Linux原型当全部平台支持。

已验证工程结论用于准备正式workspace，但R00不标全部完成，最终验收仍保留缺口。需要人工设备操作的项目统一放最终交付清单，不在实施中要求用户逐项配合。
