# 决策记录(DECISIONS)

来源:`docs/plan.md` §0、§0.2、§12。

## 状态标记

- **已定**:作者拍板,agent 不得改动。
- **默认**:建议值,可调整,但改动要记入本文件。
- **待定**:尚未决定,agent 遇到必须停下来问人。
- **暂缓**:有意推迟的事项。agent 不处理,也不必为此提问。

正文里未标记的规格视为“默认”。

## 决策表

| 项 | 决策 | 状态 |
| --- | --- | --- |
| 形态 | 先做 Web(PWA),手机端和桌面端留到 SaaS 阶段 | 已定 |
| 路线 | 先自用并开源验证,SaaS 在验证之后 | 已定 |
| LLM | DeepSeek(OpenAI 兼容接口);baseURL、密钥、模型名全部来自环境变量,不写死 | 已定 |
| 目标词书 | 大学英语四级(CET-4);作者当前水平约在高中 3500 词到四级之间 | 已定 |
| AI 接入 | MVP 自带 API Key(BYOK),密钥只存服务端环境变量 | 已定 |
| 复习 | 自研复习流程,调度直接用开源 FSRS 库;Anki 只作导入来源 | 已定 |
| 基线已知词 | 用户自行导入词表(作者:高中 3500 词作基线,四级词书作学习词书),MVP 不内置词表 | 默认 |
| MVP 不做 | 外刊匹配、词汇量初测、多端同步、原生 App、账号体系、SaaS | 已定 |
| TTS 服务商 | M5 前通过盲听选型(S5);服务商、模型、音色全部走配置 | 待定 |
| 开源许可证 | MVP 期间仓库保持私有,不添加 LICENSE 文件,也不在 package.json 里声明许可证 | 暂缓 |
| 模型配置 | LLM 与 TTS 的模型名都不写死(见 `docs/SPEC-CORE.md` §5.1) | 已定 |
| agent 工具与仓库托管 | 由执行的 agent 按所用工具自行适配,记入本文件 | 默认(不阻塞) |

## 待确认

1. **TTS 服务商(待定,M5 前确定,不阻塞 M0–M4)**:做 spike S5 盲听后由作者选定。候选只作参考(OpenAI、ElevenLabs、Azure、Google 等)。面向中国大陆的 SaaS 阶段需另行核实各家的可用性与合规。
2. **开源许可证(暂缓)**:公开仓库前再决定。

## 变更日志

### 2026-10-11 文档拆分

- 执行 agent 为 Cursor agent。Cursor 原生读取根目录 `AGENTS.md`,因此不另建 `.cursor/rules`。
- CI 按 `docs/ARCHITECTURE.md` 使用 GitHub Actions;仓库托管尚未确定(当前目录还不是 git 仓库),T0.1 时确认。
- 完整方案的文件名为 `docs/plan.md`(plan §0.1 里写的是 `docs/PLAN.md`),各文档统一引用 `docs/plan.md`。
- plan §9.2 写 spike 产出范围为 S1–S4,但 S5 同样写入 `docs/spikes/S5.md`;BACKLOG 中统一为 S1–S5。
- lint 工具(Biome 或 ESLint+Prettier)尚未选定,留到 T0.1 spec 中决定并记入本文件。
