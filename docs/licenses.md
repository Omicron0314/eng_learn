# 数据、字体、图标与依赖来源

本文件记录来源与待验证范围，不代替项目自身的开源许可证。当前不添加LICENSE或包license声明；公开发布前单独决定项目许可。规划采用某依赖不等于本次已安装／分发该依赖。

## 保留资产

- 不内置或分发用户词书。私人TXT/CSV/APKG仅在用户明确选择时本地导入，不复制进fixtures。
- `fixtures/wordlist/cases.json`、`fixtures/lemma/probes.json`、`fixtures/wordlist/baseline-sample.txt`、`fixtures/eval/targets.json`及`fixtures/analyzer/samples/import-baseline.txt`是旧实现留下的原创／合成验证素材，不宣称是正式四级或高中词书。
- 旧`wink-lemmatizer`包metadata标MIT；未独立审计其所有上游数据。Rust迁移若更换词形数据，需重新核对来源，不自动继承结论。
- 历史许可记录见 git 历史（提交 46f499a）。

## 设计与新UI依赖

| 对象 | 本次可见来源／条款 | 实施要求 |
| --- | --- | --- |
| [cijian-m3](cijian-m3.html) | 用户提供设计；内嵌Roboto Flex／Roboto Mono及SIL OFL 1.1声明 | 保留HTML原注释；`crates/cijian-ui/assets/fonts` 为由该HTML字体子集静态实例化的界面字体，许可全文见同目录 `OFL.txt` |
| [neko](neko.html) | 用户提供辅助参考 | 保留原文件；不推定其中所有第三方资源都可直接打包 |
| [gpui-kit-material](https://github.com/mizorewww/gpui-kit-material) | README称MIT，移植自Apache-2.0的文件保留原声明 | 锁定revision后逐项形成第三方notice清单 |
| [GPUI Kit](https://github.com/longbridge/gpui-kit) / [GPUI](https://gpui.rs/) | 上游称软件源码Apache-2.0；Kit部分文档文字另有CC BY 4.0说明 | 核对所锁定源码及vendor补丁许可；引用事实与复制文字分开 |
| Material Symbols | Material README称Apache-2.0，独立图标repo锁revision | 保留原notice，不从网页随意提取未确认资源 |
| 中文字体fallback | 使用系统 Noto Sans CJK SC／Noto Sans SC fallback | 不复制系统字体；未随二进制分发中文字体 |
| 桌面图标 | `apps/desktop/assets/cijian.svg` 的“词”字形取自本机 Noto Sans CJK SC Bold；`apps/desktop/assets/Noto-OFL.txt` 为对应 OFL 全文 | 仅嵌入单字轮廓；不宣称 Noto 其它字形或字体文件已打包 |

上述为公开文档查阅结果，不是全依赖许可证审计。R00/R01锁定后生成依赖与资源清单，发布打包时核对实际包含的字体／图标／音频素材。设计稿只有部分字体字符子集，不应直接当作全语言字体包。

生成文章记录来源、prompt版本与model；用户提供内容保留来源字段，不把“AI生成”当作任何第三方内容许可豁免。备份包可能含用户学习材料，默认留本地。
