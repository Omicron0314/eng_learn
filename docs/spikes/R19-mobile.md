# R19 移动原生可行性记录

日期：2026-11-03。结论只针对锁定的 GPUI 依赖与本机可用环境，不代表未来上游状态。

## 环境事实

- 主机：Arch Linux，`Linux 7.2.11-arch1-1 x86_64`；Rust `1.99.1`。
- 已安装 Rust target 只有 `x86_64-unknown-linux-gnu`、`wasm32-unknown-unknown`。
- 未安装 Android SDK/NDK、`adb`、`sdkmanager` 或模拟器；没有 Android/iOS 实机连接。
- 没有 macOS/Xcode 工具链；iOS 原生构建与真机安装在本机不可执行。
- 未创建或运行任何移动端最小包，因此没有触摸、中文 IME、软键盘、旋转、前后台、GPU 恢复或可访问性的实机证据。

## 依赖链核实

锁定 Material revision `2e13169fceb4ac328e1361aa143632025598872f` 的 vendored 平台实现包含：

- Linux、macOS、Windows、Web（`gpui-fast-linux` / `-macos` / `-windows` / `-web`）。
- `gpui-fast-apple` 面向 Apple 平台，理论上覆盖 iOS 的一部分底层能力。
- **没有 Android 平台实现 crate**；仅在通用枚举和键盘映射中出现 `android` 分支，不能构成可维护的窗口/输入/生命周期后端。

因此 Android 在锁定栈上没有可建立的 GPUI 原生入口。iOS 的 Apple 后端存在，但需要 macOS/Xcode 与真机，本机无法完成 AC2–AC4。

## 决定

- **Android：no-go（锁定 revisions）**。不进入 R20 Android 实现；若产品需要 Android 原生，必须先由上游提供可维护后端并重新做可行性任务，不能以浏览器页面冒充原生。
- **iOS：blocked，未判定 go**。保留 `gpui-fast-apple` 线索；需要 macOS 构建机和至少一台真机完成触摸、IME、生命周期、存储、音频与可访问性验证后才能决定。
- **移动浏览器**：不属于 R19 原生结论；是否交付由 R18 单独决定。手机浏览器截图不能作为原生通过证据。
- **R20**：不建立入口。当前没有通过 R19 的平台。

## 尚需证据（换环境后）

1. iOS 最小包在真机的启动、触摸、中文 IME、软键盘避让、旋转、后台恢复、GPU 重建。
2. 数据库/文件选择/凭据/远端音频/系统朗读的移动适配差异。
3. Android 若上游出现后端：相同矩阵加打包、权限与后台音频策略。

本记录不声称移动端功能可用，也不改变桌面与 Web 的任务状态。
