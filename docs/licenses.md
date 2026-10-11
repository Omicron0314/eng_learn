# 词表与词形数据来源

- MVP 不内置或分发任何词书。用户自带 TXT/CSV/apkg 仅在本地导入；未将用户牌组或词表内容公开成 fixtures。
- `fixtures/wordlist/cases.json`、`fixtures/lemma/probes.json` 为合成/原创测试输入，不代表任何第三方词书。500 行负载用例在测试内生成标识符。
- 现有 wink-lemmatizer 安装包版本 3.0.4 的 package metadata 标为 MIT；本阶段没有新引入词形表或数据文件。未完成其所有上游数据来源的独立审计，不宣称许可问题全部解决。
- 项目自身 LICENSE 与 package.json license 字段继续暂缓，本阶段未新增。

## Web 阶段合成样例

`fixtures/wordlist/baseline-sample.txt`（189 条常用词）和 `fixtures/eval/targets.json`（30 组）为本次手工编排的原创测试输入，不摘录用户词书、不声称是高中 / 四级词表；仅用于离线工具与合成导入验证。

## 审查修复样例与私有文件

新增 `fixtures/analyzer/samples/import-baseline.txt` 为原创短文，仅用于合成基线覆盖率链路，不声称为真实四级语料。追加变体/音标规则也是原创合成输入。父审查对用户自带 blank.apkg 只提供大小/笔记数/字段数的私有读取证据；本次未再读取、自动导入或复制其内容，不随仓库分发，不宣称用户文件可再分发。
