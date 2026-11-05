# 平台矩阵与交付条件

核实日期：2026-11-05（[D54](DECISIONS.md)）。资料来源和限制见 [GPUI-FEASIBILITY](spikes/GPUI-FEASIBILITY.md)。**上游列出支持 ≠ 词间已经构建／实机验收。** 本轮只交付 Linux 桌面；Windows／macOS 不做，Web 与移动端为远期规划。

## 交付路线

| 平台 | 上游可见证据 | 词间路线 | 本轮证据 |
| --- | --- | --- | --- |
| Linux | Kit 声明支持；Material 有 CI | 唯一目标平台，Wayland／X11 同一 CI 通道 | CI：构建、软件渲染启动、SQLite、音频解码、包构建与启动冒烟 |
| Windows | Kit 声明支持 | 本轮不做 | 无（不要求，不记为通过） |
| macOS | Kit 声明支持 | 本轮不做 | 无（不要求，不记为通过） |
| 桌面浏览器 / 手机浏览器 | Kit／Material 有共享 WASM gallery | 远期（R18，暂不实施） | 无 |
| Android / iOS 原生 | 未从本次来源确认受支持的 GPUI 后端 | 远期（R19 验证 → R20 条件实施，暂不实施） | 无 |

Windows／macOS 与浏览器、移动行的上游事实只用于未来评估，不是本轮承诺。Linux 证据一律来自 CI 作业：Xvfb 下属软件渲染，不得写作“GPU 已验收”。

R17 只对 Linux 给出通过／阻塞。Web 适配归远期 R18；原生移动归 R19–R20，不能自行换成 React Native／Tauri／Flutter。

## 平台接口与降级

“Web/WASM默认”列属于远期（R18）设计约束；本轮只实现桌面列，不因该列已写定而视为已交付。

| 能力 | 桌面默认 | Web/WASM默认 | 无能力时 |
| --- | --- | --- | --- |
| Repository | SQLite＋本地用户目录 | IndexedDB事务实现；若 R00 证明 SQLite WASM/OPFS 更适合可决策替换 | 明确不支持持久化，不能静默用易失空库冒充保存 |
| BlobStore | 文件缓存 | IndexedDB/OPFS blob，依据实测 | 存储满时可清缓存并保留学习数据 |
| 文件选择 | 系统选择器 | File API／文件下载 | 提供错误与替代格式说明 |
| 远端 LLM/TTS | 原生 HTTP transport | fetch；仅连允许 CORS 的自带端点 | 给出 CORS／连接说明；可配置用户自有代理，不内置托管网关 |
| 凭据 | 系统凭据存储；不可用时会话内 | 默认会话内；不宣称浏览器能隐藏 BYOK | 重启需重新输入，明确状态 |
| 整篇音频 | 原生 AudioPlayer | HTML media/Web Audio适配 | 降级显示实际可用操作 |
| 免费朗读 | 系统英语 voice，按能力检测 | SpeechSynthesis，按能力检测 | 提示未安装／不支持，已有音频仍可播 |
| 快捷键／IME | Kit focus/actions＋系统输入 | Kit web input bridge＋IME | 不以键盘事件模拟中文输入完成验收 |
| 主题／生命周期 | OS主题、窗口与电源事件 | matchMedia、visibility、page lifecycle | 用户显式主题仍可用；回来重新取队列 |

浏览器中的密钥存在客户端运行时，不能沿用设计稿“只在服务端、不下发浏览器”的字面承诺。桌面也不能假称有服务端：应显示“由本机凭据存储保存”；来源说明随平台变化。保存凭据不做测试调用。

桌面已下载内容可离线使用；Web 离线需额外完成入口／字体／图标／WASM 资源缓存与存储恢复测试。PWA 安装图标本身不构成离线能力。云端 AI 操作在离线时即时提示，不能靠旧服务工作线程隐式重试。

## R00 / R19 验证记录模板

记录 OS／架构、渲染后端（Xvfb 需注明软件渲染）、引擎和Material完整revision、依赖图、命令、截图、失败日志和可重现步骤。需要实际运行的项目不能仅勾选“cargo check通过”。

R00 只做 Linux 消费者：窗口、数据库契约、音频解码与 FSRS 状态转换。WASM、中文输入法与其他 OS 不再作为门槛。R19 保留远期记录，本轮不执行；狭窄桌面窗口和浏览器设备模拟不能抵作移动原生验证。

## 多端数据边界

每个设备独立 profile 与设备ID。备份恢复使用显式版本化包，恢复到新 profile，避免覆盖活跃数据。UUID、append-only 事件和时间戳为未来同步提供素材，但它们不解决冲突：同步需要幂等、删除传播、同卡并发评分的确定性重放、凭据排除和恢复协议，归 R21。
