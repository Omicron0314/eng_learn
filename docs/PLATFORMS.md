# 平台矩阵与能力适配

当前交付范围仅限 **Linux 桌面端**；Windows/macOS 本轮不做，Web/WASM 与移动原生属远期规划（见 [D47](DECISIONS.md)、[D54](DECISIONS.md)）。上游可行性调研见 [GPUI-FEASIBILITY](spikes/GPUI-FEASIBILITY.md)，消费者实测见 [R00 平台记录](spikes/R00-platform.md)。

## 平台交付矩阵

| 平台 | 当前路线 | 验证方式 |
| --- | --- | --- |
| Linux (Wayland / X11) | **唯一交付目标** | CI（构建、Xvfb 软件渲染 UI 测试、SQLite、音频解码、打包与启动冒烟） |
| Windows / macOS | 本轮不做 | 仅保留跨平台代码结构，不做实机验收 |
| Web / WASM | 远期（[R18](archive/R18-wasm-web.md)） | 仅保留共享 UI 的 `wasm32` 编译检查 |
| Android / iOS 原生 | 远期（[R19](archive/R19-mobile-native-spike.md) / [R20](archive/R20-mobile-native.md)） | 见 [R19 调研记录](archive/R19-mobile.md) |

## 桌面平台接口与降级策略

| 能力 | 桌面实现 | 能力缺失或异常时的降级表现 |
| --- | --- | --- |
| 数据仓储 (`Repository`) | 本地 SQLite（WAL 模式） | 数据库损坏或版本未知时进入恢复/拒绝提示，不静默替换为空库 |
| 音频文件 (`BlobStore`) | 本地文件缓存（SHA-256 寻址） | 缓存缺失时重新合成或降级，保留核心学习数据 |
| 文件选择 | 系统原生文件对话框 (`rfd`) | 取消选择不触发操作；格式错误展示可读诊断 |
| 远端 LLM / TTS | 原生 HTTP 客户端 | 未配置或断网时仅阻止对应在线动作，离线复习与阅读照常可用 |
| 密钥存储 (`SecretStore`) | 优先系统凭据库（Secret Service） | 凭据服务不可用时明确回退至会话内存，界面如实展示保存来源 |
| 整篇与单词音频 (`AudioPlayer`) | 原生音频播放 (`rodio`)，文章与单词独立 sink | 按实际能力展示暂停/定位/倍速 |
| 本机免费朗读 (`SystemSpeech`) | 探测系统英语语音（`espeak-ng`） | 未安装时提示不可用，不影响已缓存音频与单词录音播放 |
| 主题与窗口 | 跟随系统 / 浅色 / 深色，48dp 自绘标题栏 | 窗口间主题独立，关闭窗口前执行未保存草稿保护 |

## 多设备数据边界

每台设备拥有独立 `profile` 与 `device_id`。逻辑备份（`.json` / 含音频包）恢复时默认建立新 profile，避免覆盖当前活跃学习数据。跨端增量同步草案见 [SYNC-PROTOCOL](archive/SYNC-PROTOCOL.md)。
