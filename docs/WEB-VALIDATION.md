# Web 阶段实施与证据

## 已实现（不等于真实效果验收）

- 基于 services 的 zod API：导入、复习、词卡编辑 / 暂停、设置、生成、阅读事件、点词、三种标记、inbox 批量处理、音频 Range、反馈和统计。统一错误 envelope；未知异常不返回内部信息。
- 七个响应式中文页面及登录页。自然色系、桌面侧栏 / 移动导航、清楚的空状态 / 加载 / 错误 / 禁用状态；键盘复习、可键盘触发的词 span、原生 modal 的焦点 / Escape 支持。假服务在全局界面明确标识。
- APP_PASSWORD：Next 16 `proxy.ts` 页面门禁，API 自身重复验权（包括音频）；常量时间比较、HMAC cookie、HttpOnly / SameSite / HTTPS Secure，单进程密码尝试限速。跨站写入拒绝。
- 导入 CSV/TXT 上限 20 MiB、.apkg 上限 64 MiB；声明长度提前拒绝，真实流限 64 MiB + 64 KiB，再按文件检查。Next proxy body buffer 65 MiB，避免截断有效导入。JSON 64 KiB 上限。
- 播放器：server `<audio>` / 浏览器 SpeechSynthesis、倍速与 localStorage、暂停 / 后退 / 停止、先听后看、播放结束反馈、超配额浏览器兜底。
- PWA manifest / SVG 图标、严格 TypeScript（含 exactOptionalPropertyTypes）、Tailwind、原生最小组件、standalone Docker / compose、CI、demo、报告与手动评测工具。

## 本机实际验证

开发机 Node v26.11.1、pnpm 12.11.2；项目 / CI / Docker 目标 Node 24。

| 命令 / 操作 | 结果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 通过 |
| `pnpm lint` | Biome 与边界检查通过 |
| `pnpm typecheck` | 四个 workspace 与 tools 全部通过 |
| `pnpm test` | 23 文件、237 测试通过；Fake providers、内存 SQLite 为业务测试默认 |
| `pnpm build` | Next 生产构建通过，七个业务页面、登录、API 与 proxy |
| `pnpm --filter @eng-learn/web exec playwright install chromium` | 用户缓存内安装成功；未改系统包 |
| `pnpm e2e` | Chromium 4 测试通过；未调用真实 AI |
| `pnpm eval:generation --fake` | 30/30 组完成，全部指标与 JSON 输出；不代表真实质量 |
| `pnpm report:experiment --db :memory: --json` | 空数据 n=0、rate/diff=null，JSON 可解析 |
| standalone 本地运行（假服务，临时文件库） | 密码登录 → 导入 3 词 → 评分 → 生成 → 音频 → stats；停进程后重启，文章与 3 条评分仍在 |
| standalone 缺少真实 LLM 三变量 | 启动退出 1，仅报告缺少变量名，未接触 provider / 存储 |
| 桌面 / 390px 移动截图 | 本机查看；无页面横向溢出。截图位于忽略的 `apps/web/test-results/` |
| `pnpm exec tsx tools/verify-standalone.ts`（build 后） | HOSTNAME=0.0.0.0，直接浏览器 Origin / 显式 HTTPS APP_ORIGIN 均登录与设置写入 200；跨站 403，HTTPS cookie Secure；21 MiB 合成无效 apkg 到达格式检查而非 413 |
| `docker --version` | 本机未找到 docker，容器检查未执行 |
| 已合并迁移、暂存区 | 无迁移改动；没有暂存文件 |

E2E 覆盖：页面 / API / 音频密码保护、错误密码、manifest、导入 / 搜索 / 编辑 / 暂停 / 恢复、复习评分、生成 / 体裁、正文文本与原文一致、键盘点词、三种标记、inbox 加入 / 忽略、音频缓存 / Range / 播放 / 倍速 / 结束、TTS 超配额与浏览器 mock、按钮 / 滚动读完、难度 / 不自然反馈、统计、设置刷新持久化、404 / 非法参数 / 跨站 / 大文件，以及 100 词导入后 20 张新卡键盘复习与日上限完成。

## 明确没有证明

- **Docker 不可用**：本机没有 docker 命令，未执行 `docker compose config` / `docker build` / 容器重启。Dockerfile 已按 standalone 实际目录检查；CI 有 build 步骤，但远程 CI 尚未运行。
- 真实 LLM / TTS 质量、付费费用实测、30 组真实生成评测、2–4 周 H1–H3 留存 **没有运行**。假评测的长度达标率为 0，不能据其 JSON / coverage 数字推断真实质量。
- S1 的 200 句与作者抽查、S2 的真实 Anki 双格式导出、S4 的用户实际词表与 500 词规模样本仍缺证据。S2 父审查已提供单一用户私有文件 4,028 笔记/9 字段的计数解析证据，但没有版本/双格式导出证明，未公开或自动导入文件。评测基线是原创的 189 条合成常用词样例，不是高中 3500 词书。
- 单进程内的密码 / 合成锁不是多副本协调；密码门禁不是完整账号系统。HTTP cookie 不设 Secure，公网必须 HTTPS。没有离线 service worker。
- 手动评测成本上界假定单价正确且 provider 遵守输出 token 上限；未知附加收费与部分失败的真实 TTS 成本仍不能由离线测试证明。

## 两轮审查后的具体回归

- TTS：调用前独立提交 audio_reserve，全文本保守预留；FakeTTS 部分合成错误、兼容适配器 fake fetch 第一段成功/第二段失败、blob put 失败、audio_generate 事务事件失败仍消耗额度，重试被拦截；预留失败不调用 provider。缓存命中不计费；旧成功记录兼容且 linked success 不重复计费。
- 评分：事务内按当前队列和配额验证，零/耗尽新词、超 review 名额、未来 due/旧标签页重提无调度/日志/事件变更；due learning 在零 review 配额仍可评。
- 生成：非法 JSON 回复与诊断中的 control 词被过滤，不泄漏到修复请求，仍保留 invalid_output/ok 日志。
- 导入：47 条规则覆盖组合/附着音标；原创 23-token 短文通过导入→KnownSet→分析的覆盖率为22/23，不能当作四级实语料验收；中途异常五表回滚；文件库评分重开后调度/日志/事件/计数/队列一致。
- 部署/UI：APP_ORIGIN 启动校验与 Host 同源策略；64 MiB apkg 全链路边界；E2E 验证阅读刚好选择折叠后保留、基线词/卡新增与合并统计、autoAdd 模式警告及实际自动卡片回显/无 pending inbox。

最初新增回归运行按预期暴露旧行为失败；修复后最终命令全部通过。一次 E2E 合并测试忘记重新填写被 React form action 重置的词表名，修正测试输入后完整四项通过；未修改导入合并产品语义。没有付费评测、提交、推送或暂存。

## 父 agent 最终独立验收

- 亲自重跑 lint、typecheck、test、build、四项 Chromium E2E、standalone 两种 Origin 回归、30 组假评测与空实验报告，全部通过。
- 补充 CLI 临时种子库测试：出现组 10 样本 / 2 Again（0.2），对照组 5 样本 / 2 Again（0.4），差值 −0.2；后续第二次 Again 不改变首次复习结果。该测试实际启动报告脚本并解析 JSON，不仅测试纯函数。
- 两位独立 reviewer 复核领域与安全/UI 修复，均未发现可执行阻塞项；命令由父 agent 运行，不把只读审查当作命令证据。
- 本地 `pnpm dev:demo --hostname 127.0.0.1` 已启动，浏览器访问 `/` 正确跳到 `/today`，标题“词间 · 英语学习”，控制台无 error/warning。已查看全页截图 `/tmp/eng-learn-mvp/today.png`；演示标识清楚。
- 演示仅绑定本机；使用 fake LLM + 浏览器朗读，无密码/真实密钥/付费调用。日志与 launcher PID 在 `/tmp/eng-learn-mvp/`，演示数据仍使用 README 的 `data/demo.sqlite`。

## 复核入口

`apps/web/src/server/routes.test.ts`、`http.test.ts`、`apps/web/e2e/mvp.spec.ts`、`tools/check-boundaries.test.ts`、`tools/scripts.test.ts`、`tools/eval/budget.test.ts`。后端领域证据见 `docs/SERVICES-HANDOFF.md` 与各服务测试。

评测调用同时写 `llm_calls`：fake 模式使用内存 SQLite，真实手动模式使用 `DATABASE_PATH`（默认根目录 `./data/app.sqlite`），并在结果 JSON 保留调用记录。不做 TTS 调用。
