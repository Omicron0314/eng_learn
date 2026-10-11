# S3 提示词评测 spike

## 目标

在真实 LLM 上评测 `article.v1` 提示词:JSON 合法率、coverage、目标词出现次数,确认 M3 的质量门槛可达。同时建立以后每次改提示词都要跑的评测工具 `pnpm eval:generation`。

## 范围与非范围

**范围**

- 提示词文件 `prompts/article.v1.md`:内容取自 `docs/SPEC-CORE.md` §5 的骨架,占位符 `{genre}`、`{minWords}`、`{maxWords}`、`{level}`、`{targets}`、`{avoidWords}`。
- 渲染函数 `renderArticlePrompt(template, vars)`(`packages/core/src/generation/prompt.ts`,纯函数,T3.2 复用)。提示词文本由调用方读取后传入,core 不读文件。
- 评测样例 `fixtures/eval/targets.json`:30 组,每组 8 个四级常见词(agent 选词),并指定体裁(三种体裁各 10 组)。
- 已知词集合:优先读取 `EVAL_BASELINE_PATH` 指向的本地基线词表(作者的高中 3500 词,不提交);未配置时用 `fixtures/wordlist/baseline-sample.txt`,结果中注明来源。
- 评测工具 `tools/eval/generation.ts`(脚本 `pnpm eval:generation`):
  - 每组:渲染提示词 → `completeJson`(T0.3)→ `analyzeText`(T2.1)→ 记录指标;
  - 指标:首次 JSON 合法率、修复后合法率、coverage(中位数、P10)、`unknownRatio` 中位数、目标词出现 ≥2 次的比例、字数落在区间内的比例、平均延迟、token 数、费用;
  - 费用上限:按 `EVAL_MAX_COST_USD`(默认 1)累计,达到上限即停止并报告已完成组数;未配置单价时拒绝运行真实评测(无法保证上限);
  - 输出:终端表格 + `tools/eval/results/<时间戳>.json`(含当前配置的模型名与提示词版本,模型名只出现在结果数据里);
  - `--fake` 参数:使用 FakeLLM 的固定回复,供 CI 验证工具本身。
- 结论 `docs/spikes/S3.md`:指标表(引用结果文件)、典型失败样例、对提示词的改进建议(只提建议,修改提示词需另行评审)。

**非范围**:重试逻辑(T3.2);提示词多版本对比(以后每次改提示词时做)。

说明:评测工具与 `article.v1` 渲染函数属于长期保留的工具代码,不受“spike 代码不进入产品”的限制。

## 接口

```ts
type ArticlePromptVars = {
  genre: "story" | "dialogue" | "monologue";
  minWords: number; maxWords: number; level: string;
  targets: string[]; avoidWords?: string[];
};
function renderArticlePrompt(template: string, vars: ArticlePromptVars): LLMMessage[];
```

## 验收标准

- AC1:`renderArticlePrompt` 单元测试:占位符全部替换;`avoidWords` 为空时整行不出现;渲染结果中不出现任何模型名。
- AC2:`pnpm eval:generation --fake` 在 CI 中运行成功,输出包含全部指标列,并生成结果 JSON。
- AC3:未配置单价时运行真实评测,脚本拒绝并给出提示;配置后,累计费用达到上限时停止。
- AC4(手动,真实调用,需作者确认后运行):30 组结果写入 `docs/spikes/S3.md`,并给出与门槛的对比:JSON 合法率 ≥95%,“适中”档 `unknownRatio` 中位数 ≤5%。
- AC5:未达门槛时,S3.md 列出失败原因与改进建议,交作者决定是否修改提示词(提示词变更需附评测前后对比)。

## 测试计划

- 单元:`renderArticlePrompt`(AC1)。
- 集成:`--fake` 模式跑完整流程(AC2、AC3 的拒绝分支)。

## 依赖与风险

- 依赖 T0.3、T2.1。
- **待确认(停下来问人)**:真实评测会产生费用,每次运行前须作者确认;`EVAL_BASELINE_PATH` 指向的词表由作者提供。
- 风险:FakeLLM 回复无法反映真实质量,AC4 必须真实运行一次才算 S3 完成。
