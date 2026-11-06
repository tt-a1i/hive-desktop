# Domain Docs

本项目使用 single-context：根目录 GLOSSARY.md 与 docs/adr/。

## 读取规则

探索代码前，读取已有 GLOSSARY.md，以及与当前工作相关的 ADR。
文件不存在时直接继续；由 /domain-modeling 在术语或决策得到确认后按需创建。

## 术语

需求、规格、代码和测试中的领域概念遵循 GLOSSARY.md。
发现术语缺口时，先核对项目已有用法，再记录需要领域建模的问题。

## 决策冲突

方案与已有 ADR 冲突时，明确指出对应 ADR、冲突内容及重新讨论的理由。
