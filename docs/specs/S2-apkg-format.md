# S2 .apkg 格式 spike

## 目标

弄清 Anki `.apkg` 各版本的结构,确定 T1.1 的检测规则与字段提取方法,并确认“新版压缩格式时提示用户用兼容模式重新导出”是否可行。

## 范围与非范围

**范围:需要回答的问题**

1. `.apkg` 里可能出现哪些集合文件:`collection.anki2`、`collection.anki21`、`collection.anki21b`(zstd 压缩)?各自对应哪个 Anki 版本与导出选项?
2. 新版 Anki 导出 `anki21b` 时,是否同时附带一个只含“请升级 Anki”提示笔记的假 `collection.anki2`?如何识别并避免误导入这条提示?
3. 勾选“支持旧版 Anki”(Support older Anki versions)导出时,得到的是哪种文件,能否用 better-sqlite3 直接读取?
4. `notes.flds` 用 `\x1f` 分隔字段,在各版本中是否一致?笔记类型(字段名)在旧版的 `col.models` JSON 与新版的 `notetypes` 表中如何读取?MVP 只取前两个字段,是否需要读字段名?
5. 字段中常见的 HTML、`[sound:...]`、`<img>`、cloze 语法 `{{c1::...}}` 如何清洗为纯文本?
6. 解压与读取需要什么依赖,文件大小与内存占用如何?

**非范围**:媒体文件、复习记录与调度数据(MVP 忽略);写入产品代码。

## 接口

spike 结论要给出 T1.1 采用的接口草案:

```ts
type ApkgFormat = "legacy-anki2" | "anki21" | "anki21b";
type ApkgNote = { fields: string[]; tags: string[] };
detectApkgFormat(entries: string[]): ApkgFormat | "unknown";
readApkgNotes(bytes: Uint8Array): Promise<ApkgNote[]>;   // anki21b 抛 ApkgUnsupportedFormatError
stripAnkiHtml(field: string): string;
```

## 验收标准

- AC1:`docs/spikes/S2.md` 逐条回答上面 6 个问题,并附格式对照表(文件名、压缩方式、schema 版本、能否直接读取、检测规则)。
- AC2:准备合成样例牌组(由作者在 Anki 中新建约 10 张卡,分别用“支持旧版”和默认方式导出),放在 `fixtures/apkg/`,不提交任何来源不明的第三方牌组。
- AC3:`spikes/S2/read.ts` 能读取兼容格式样例并打印前 5 条笔记的清洗后字段;对 `anki21b` 样例给出明确的“不支持,请用兼容模式导出”提示。
- AC4:`stripAnkiHtml` 的规则清单与示例(`<br>`→换行、实体解码、去 `[sound:]`、去 `<img>`、cloze 取答案文本)写进 S2.md,作为 T1.1 的测试用例来源。

## 测试计划

spike 不要求单元测试;AC3 的脚本输出贴进 S2.md。清洗规则在 T1.1 中转成正式测试。

## 依赖与风险

**待确认(停下来问人)**

1. 需要作者提供 2 份样例 `.apkg`(兼容格式与默认格式各一份)。
2. 新增依赖:解压库(推荐 `fflate`,纯 JS、体积小),spike 阶段装在 `spikes/S2/` 下;进入产品需在 T1.1 再次确认。
3. 若结论是“兼容模式导出不可行”,T1.1 需要改为引入 zstd 解压(新增依赖),或只支持 CSV,由作者决定。

**风险**:Anki 格式随版本变化,结论需注明测试所用的 Anki 版本。
