# R08 · 词元分析、已知词与选词

任务状态以 [BACKLOG](../BACKLOG.md) 为准。阶段：G2。依赖：R02、R06。

## 目标与范围

tokenize_with_ranges、analyze_text(body,known,targets)、build_known_set(now)、select_targets(seed)；输出UTF-8区间、分析版本与诊断。

设计追踪：文章难度、覆盖率与目标词来源。通用约定见 [specs/README](README.md)。

## 验收标准

- **AC1**：原始正文可无损重建，缩写／标点／Unicode与UTF-16转换有样例。
- **AC2**：基线、Review预测、明确认识与近期不认识覆盖规则正确；control不自动并入known。
- **AC3**：token/type覆盖率、未知词、目标出现数与names稳定，空分母不除零。
- **AC4**：优先级、近三日重复排除、floor比例和至少一个exposed固定种子可复现；空候选明确报错。

## 验证与交付

复用原创lemma/wordlist/analyzer fixtures，增加Unicode／时间边界向量；记录词形准确性缺口。

## 边界与风险

词形启发式与专名识别有误差；覆盖率是辅助指标，不冒充完整语言能力测量。
