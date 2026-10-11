# S4 词表导入 spike

## 目标

弄清用户自带词表(TXT / CSV,如高中 3500 词、四级词书)的常见写法,产出把每行归一为词元条目的规则,供 T2.2 实现与测试。

## 范围与非范围

**范围**

- 收集作者实际使用的词表文件(只在本地分析,放在 `data/`,不提交,不在仓库里保存全文),统计行数与各类写法的占比。
- 需要覆盖的写法(至少):
  - 纯词头:`abandon`;
  - 带词性与中文释义:`abandon v. 放弃`、`abandon vt.&n. 放弃;放纵`;
  - 带音标:`abandon /əˈbændən/ v. 放弃`、`[əˈbændən]`;
  - 编号与分隔符:`1. abandon`、`abandon\t放弃`、CSV 列;
  - 变体:`a (an)`、`colour (color)`、`color/colour`;
  - 短语:`be able to`、`according to`、`a lot of`;
  - 缩写与特殊形式:`a.m./A.M.`、`Mr.`、`TV`、`well-known`;
  - 屈折形式词头(`glasses`、`goods`、`clothes`)是否应保留原样;
  - 大小写不同的重复、同词多义多行的重复;
  - 空行、标题行(如 `A`、`Unit 1`)。
- 规则结论:每种写法如何得到 `headword`、`variants`、`pos`、`meaning`、`isPhrase`,以及是否对词头做词形还原(初步建议:不做,词头本身就是原形;复数形式的固定词头保留原样)。

**非范围**:内置任何词表;多词短语在文章中的匹配(MVP 的分析器只按单词计算,短语条目只存储,不参与覆盖率)。

## 接口

```ts
type WordListEntry = {
  headword: string;          // 小写,已去音标、编号、释义
  variants: string[];        // 如 colour (color) -> ["color"]
  pos?: string;
  meaning?: string;
  isPhrase: boolean;
  raw: string;
};
function parseWordListLine(line: string): WordListEntry | null;   // null 表示跳过(空行、标题)
```

## 验收标准

- AC1:`docs/spikes/S4.md` 给出规则表,每条规则带输入 / 输出示例。
- AC2:每条规则至少有一个用例写入 `fixtures/wordlist/cases.json`(合成示例,不含作者文件原文),格式 `{ input, expected }`,T2.2 直接用作测试。
- AC3:`spikes/S4/run.ts` 在作者的词表文件上输出:总行数、解析成功数、跳过数、去重后词条数、短语数,以及无法解析的行清单;解析成功率 ≥98%(默认目标,未达到时把剩余情况交作者决定)。
- AC4:S4.md 写明变体的处理方式(变体是否也作为已知词元加入基线)与重复项的合并规则。

## 测试计划

spike 不要求单元测试;AC2 的用例在 T2.2 中转为正式测试。

## 依赖与风险

**待确认(停下来问人)**

1. 需要作者提供高中 3500 词与四级词书的文件(或说明格式)。
2. 这些文件的来源与许可:MVP 只在本地使用,不提交到仓库;`docs/licenses.md` 记录“用户自带,不随仓库分发”。

**风险**:词表格式千差万别,规则无法覆盖全部;无法解析的行要在导入结果中列出,让用户自己修正。
