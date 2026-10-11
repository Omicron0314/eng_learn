# 核心功能规格(SPEC-CORE)

来源:`docs/plan.md` §6。正文里未标记的规格视为“默认”。

## 1. 复习

默认每日新词 20、复习上限 200、目标保持率 0.9(设置可调),使用 ts-fsrs 默认参数,4 档评分(Again / Hard / Good / Easy)。每次评分写 1 条 `review_logs` 和 1 条 `events`。

## 2. 选词 `selectTargets(userId, n=8)`

优先级:① 今天评了 Again/Hard 的词;② 今天新学的词;③ 已到期且可提取率最低的词。排除近 3 天已在文章中出现 ≥3 次的词。候选中随机 20% 标为 `control`:不进入文章,但记录到 `article_targets`,用于验证 H3。

## 3. 分析器 `analyzeText(text, known)`

```ts
type AnalysisResult = {
  tokenCount: number;
  coverage: number;                       // 已知词 token 占比(含目标词)
  unknownLemmas: { lemma: string; count: number }[];
  targetOccurrences: Record<string, number>;  // lemma -> 出现次数
  names: string[];                        // 被判为专有名词而跳过的词
};
```

规则:小写化;分词时保留缩写(don't);词形还原;跳过数字与专有名词(句中大写词,或 LLM 返回的 `names`);功能词默认已知。覆盖率按 token 计,同时报告按词种(type)的值。

## 4. 已知词模型 v0(确定性集合)

已知词 = 用户导入并标为“基线”的词表 ∪ 状态为 Review 且可提取率 ≥0.8 的卡片词 ∪ 用户标记“认识”的词 ∪ 本次目标词。排除:近期被标记“不认识”且之后未复习过的词。概率化模型留到后期。

### 4.1 词表来源

**默认**:MVP 不内置任何词表,导入时由用户把词表标为“基线(已知)”或“学习词书”。作者本人的设置:基线 = 高中 3500 词,学习词书 = 四级词书。词表文件格式沿用 §9。公开发布前才需要内置词表,届时必须逐个核实许可(NGSL、CEFR-J 等各有条款)并记录到 `docs/licenses.md`,这不属于 MVP。

## 5. 文章生成工作流 `generateArticle`

步骤:`selectTargets` → 构建提示词 → LLM(JSON 输出,zod 校验)→ `analyzeText` → 若 `coverage` 低于所选档位下限,带“需避免的词”重试 1 次(重试次数为配置项)→ 落库(articles、article\_targets、analysis\_json、llm\_calls)→ 返回。

- 难度档位:轻松 ≥98% / 适中 95–98% / 挑战 90–95%,默认适中。
- 长度默认 300–400 词(约 2–3 分钟音频);体裁:短故事 / 对话 / 独白。

提示词骨架(`prompts/article.v1.md`,版本号写入 `articles.prompt_version`):

```
System: You write short reading/listening texts for English learners.
Task: Write a {genre} of {minWords}-{maxWords} words for a learner at CEFR {level}.
Target words: {targets}
Rules:
- Use each target word at least twice, in contexts that make its meaning inferable.
- Apart from the target words, use only common, high-frequency vocabulary.
- {avoidWords ? "Do NOT use these words: " + avoidWords : ""}
- Natural spoken-style sentences; no lists of words; no explanations.
Output JSON only: {"title": string, "body": string, "names": string[]}
```

### 5.1 LLM 调用约定(已定)

- 走 OpenAI 兼容接口。`LLM_BASE_URL`、`LLM_API_KEY`、`LLM_MODEL` 全部来自环境变量;代码、提示词、测试和文档里不出现具体模型名,`.env.example` 只放占位符。当前由作者在 `.env` 里填 DeepSeek 的取值。
- 启动时检查这三个变量,缺失则给出明确报错,不使用任何默认模型。
- 更换模型只改 `.env`,不改代码。每次调用的模型名记入 `llm_calls.model` 与 `articles.model`;评测脚本按当前配置运行,并把模型名写进结果。
- TTS 同理:服务商、模型、音色全部来自配置,不写进代码。
- 结构化输出:先尝试 JSON 模式,不支持或失败时退回提示词约束。无论哪种,输出都必须过 zod 校验;校验失败用“修复提示”重试 1 次,仍失败则返回明确错误并记入 `llm_calls`。各家对 JSON 严格度不同,不能假设输出总是合规。
- 文章生成优先选便宜快速的非推理型号,推理型号只留给评测或复杂校验;具体型号由作者在 `.env` 里选。
- 提示词骨架里的 `{level}` 取自设置项,作者的默认值为“B1(约中国大学英语四级水平)”。

## 6. 点词交互

点击正文任意词,弹层显示:词形还原后的词、发音、“在本句中的释义”(LLM 生成,缓存到 `gloss_cache`),以及三个按钮:**认识 / 标记不认识 / 加入复习**。

设置项 `lookupMode`:`markOnly`(默认:只记录并进入 inbox,不建卡)、`autoAdd`(直接建卡)。加入复习的卡片计入每日新词上限;`/inbox` 页可批量加入或忽略。

## 7. 听力

文章页一键生成音频(整篇一条,按文本哈希缓存,相同文本不重复合成)。播放器:变速(0.75–1.25)、先听后看(正文默认折叠)、听完提示难度反馈。逐句重复与听写留到后续版本。成本护栏:每日 TTS 字符数上限(设置项)。

TTS 服务商(待定):不做盲听选型,到 M5 时由作者直接指定一家接入;服务商、模型、音色全部走配置。

## 8. 度量

- 难度反馈(太难 / 刚好 / 太简单)和“不自然”标记写入 `events`。
- `pnpm report:experiment` 输出 exposed 与 control 两组的 Again 率和样本量。
- `/stats` 展示:今日与本周新学、复习数量,已学词列表,文章覆盖率趋势。

## 9. 导入

CSV(列:word,meaning,example,后两列可选)。.apkg:解压并读取笔记字段,前两个字段映射为正面/背面,去 HTML,忽略媒体与调度数据;若检测到新版压缩格式,提示用户导出时选择兼容旧版(是否可行由 spike S2 确认)。缺释义的词在首次展示时由 LLM 生成简明释义并缓存,用户可编辑,来源标记为 AI 生成。
