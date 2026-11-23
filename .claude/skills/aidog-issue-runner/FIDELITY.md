# FIDELITY · aidog-issue-runner v2（2026-11-22）

女娲主题 Skill 变体升级（快速档，语料 = 仓库自身规则文件，无网络调研）。

## 增量来源

先验快照与语料采集：`references/research/00-prior.md`、`01-corpus-capture.md`。「和先验不一样」合计 10 条（≥8 过关）。

## 结构变化（v1 → v2）

| 项 | v1 | v2 |
|---|---|---|
| SKILL.md | 单文件 40 行 | 核心循环 + 2 个 references（按需读，保住 5K token 截断线） |
| push 预检 | 只写 rev-list 计数 | 补禁 is-ancestor、失败正解、sha 界定 |
| 合并纪律 | 未提 | READY 绑 SHA、--match-head-commit、合并态复测、评论自带证据 |
| 中断续做 | 未提 | 五查 checklist、半成品采信三前提、挂死接管判据 |
| 假红归因 | 未提 | 六步排查序 + 假绿两盲区 |
| commit 纪律 | 一句带过 | patch 过滤全流程、$ 展开、索引核对 |
| 派发要素 | 未提 | 8 项固定要素（含 idle 唤醒预期形态） |
| 误读表 | 未提 | 5 条速查 |

## 验证（2026-11-22，本会话机械核对）

- 规则节名存在性：shared-worktree.md 9 个节、pr-merge-discipline.md §2~§5、parallel-implement.md §7/§12/§15/§17/§18/§20/§21/§32/§37 —— 全部命中（grep 实测，§ 计数假阴已用 `^## N.` 复核）。
- 命令/脚本存在性：`yarn check:defaults`（package.json）、`make lint`（Makefile）—— 命中。
- description 254 字符（<300 过关）；正文无本机绝对路径（grep /Users/ 零命中）。
- 封存题增量测试：**未做**（语料为规则文件，无可封存的人物式岔路决策；以引用存在性机械核对替代）。引用内容与规则原文的一致性为人工誊写，未做独立子代理比对 —— **未独立验证**。
