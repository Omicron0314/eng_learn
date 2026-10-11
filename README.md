# 词间 · English learning MVP

私有、自用、中文界面的英语学习 PWA：导入自己的词表 → FSRS 复习 → 今日文章 → 点词 / 生词箱 → 听力 → 反馈与统计。SQLite 与音频文件保留在本机。没有内置词书、账号或同步；没有离线 service worker（PWA 安装入口不等于离线学习）。

## 快速体验（完全免费、明确假服务）

Node 24、pnpm 12.11.2：

```sh
pnpm install --frozen-lockfile
pnpm dev:demo
# http://localhost:3000
```

演示使用 FakeLLM / FakeTTS，文本和释义标记为演示，不证明真实生成质量。数据库在 `data/demo.sqlite`，音频在 `data/demo-blobs`。在词库导入 UTF-8 CSV（`word,meaning,example`，后两列可选），复习后在今日页生成文章。基线 TXT / CSV 不建卡，学习词书建卡；兼容旧版 `.apkg` 只取前两字段，忽略媒体与调度。

## 真实 LLM（BYOK，可能产生费用）

复制 `.env.example` 为 `apps/web/.env.local`，替换 LLM 的端点、服务端密钥和模型占位符，填写单价；将数据库与文件目录设为绝对路径。不要把密钥放在客户端变量或提交进仓库。

```sh
pnpm dev
# 或 pnpm build && pnpm start
```

真实模式缺少 `LLM_BASE_URL` / `LLM_API_KEY` / `LLM_MODEL` 时会立即拒绝初始化，绝不会自动降级到假服务。设置 `APP_PASSWORD` 保护页面、API 和音频。公网使用 HTTPS / 反向代理，cookie 在 HTTPS 下使用 Secure；这只是单用户密码门禁，不是完整账号体系。跨站写入拒绝，CSV/TXT 上限 20 MiB，.apkg 上限 64 MiB（解压集合上限 100 MiB）。

直接部署以请求 Host 验证浏览器 Origin，而不是监听地址；不会读取 Forwarded/X-Forwarded-* 作为同源凭据。HTTPS 反向代理须设置 `APP_ORIGIN=https://你的公开域名`（http/https origin，可含端口，不含账号、路径或查询），启动时校验；HTTPS APP_ORIGIN 同时启用 Secure cookie。代理需保留正确 Host、限制允许访问的域名并禁止后端绕过；此变量是你明确配置的信任边界。

默认 TTS 为 `none`（浏览器 SpeechSynthesis）；真实服务端语音须设置 `TTS_PROVIDER=openai-compatible` 和全部 TTS_* 配置，日字符配额在设置页调整。每次合成前持久化完整文本的保守预留；部分合成、provider / blob / 缓存事务失败不退款，避免重试绕过护栏（显示用量不等于真实账单）。浏览器朗读不支持拖动进度，不产生服务端语音费用。

## 验证与工具

```sh
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm --filter @eng-learn/web exec playwright install chromium
pnpm e2e
pnpm exec tsx tools/verify-standalone.ts  # 先 build，假服务临时数据部署回归
pnpm eval:generation --fake
pnpm report:experiment --db ./data/demo.sqlite --window 7 --json
```

E2E 只用 fake provider、内存 SQLite 与本地 Chromium。`report:experiment` 不调用 AI，输出出现组 / 对照组样本数和 Again 率，空样本为 null。

**手动真实评测，本次没有运行：** 在 shell 导出真实 LLM 配置、正数单价、可选 `EVAL_BASELINE_PATH` 和 `EVAL_MAX_COST_USD`（正数且 ≤1）后运行 `pnpm eval:generation --allow-paid`。每次底层调用（包括修复）先预留保守费用上界，输出 token 上限为 2048；预算不够就不发请求。上界依赖配置单价准确以及服务端遵守 token 上限，未知额外收费无法保证。结果写 `tools/eval/results/`（不提交），记录模型、版本、baseline 来源和质量指标。假评测不算真实质量验收。

## Docker（单进程 + 数据卷）

将 `.env.example` 复制为根 `.env` 并替换占位符；**必须设置 APP_PASSWORD**。准备可由容器 node 用户（uid 1000）写入的 `data` 目录。

```sh
mkdir -p data
# 确保 data 的属主或权限允许 uid 1000 写入
 docker compose up --build
```

默认仅监听 `127.0.0.1:3000`，SQLite / blobs 在 `./data` 持久化。备份整个目录时先停止容器。单进程锁不支持多副本预算协调。

## 边界与未验证事项

`packages/core` 无 IO；所有 AI 调用在 providers；业务写入通过仓储和事件事务。迁移保持不变。CI 包括 lint、类型、单测、构建、standalone 回归、E2E、假评测和 Docker 构建。当前机器没有 Docker，容器构建和重启验证尚未执行；真实 LLM / TTS 质量、真实 Anki 导出兼容性、完整 spike 语料和 2–4 周留存仍需手动验证。详见 `docs/MVP-EXECUTION.md`、`docs/SERVICES-HANDOFF.md` 与 `docs/WEB-VALIDATION.md`。

评测调用同时写 `llm_calls`：fake 模式使用内存 SQLite，真实手动模式使用 `DATABASE_PATH`（默认根目录 `./data/app.sqlite`），并在结果 JSON 保留调用记录。不做 TTS 调用。
