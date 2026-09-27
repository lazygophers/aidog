# wayfinder 调研轮：票面预设、agent 派发、ask-ui 解读

2026-09-27 claude-perm-ux 轮（/wayfinder + 后台 research agent）暴露的三个流程缺口。
适用于一切「ask-ui 定向 → 建票 → 派后台 agent 取证 → 结票」的调研型 session。

## 1. 票面预设事实必须标「未证实」

该轮两张 research 票的票面预设全被报告推翻：票 01 写「settings 三层级」（实际五层）、
「modes 四个」（实际六个）；票 02 预设「/hooks 可视化编辑」（实际面板已改只读）。
预设来自主会话记忆，写进 Question 时没带来源，下游若不先结取证票就会把错的前提
当靶子。

规则：

1. **Question 里凡来自记忆、未带出处的外部事实，显式标注「未证实，先核对」**，
   并要求 agent 报告里逐条判定票面预设的真假（推翻也算结票产出，不是失败）。
2. **依赖这些事实的下游票必须 `Blocked by` 取证票**（该轮 03/04 已正确挂边，
   缺的只是第 1 条的显式标注）。

## 2. 派 agent 干票，状态迁移指令要贴进派发 prompt

该轮一个 research agent 结票时把 `Status:` 写成 `done`；tracker 约定值是
`claimed` / `resolved`（`docs/agents/issue-tracker.md` Wayfinding operations 节）。
agent 不会自己去读那份 doc——词表不贴进 prompt 就等于没有。

规则：派发 prompt 里原样贴三行：开工置 `Status: claimed`；结票在文末追加
`## Answer` 段并置 `Status: resolved`；**状态词只有这两个**，禁 `done` / `closed`
等近义词。主会话收报告后仍应 grep 一次 `Status:` 核对，别假设 agent 写对了。

## 3. ask-ui 选项题收到「只写补充说明、未选任何选项」

该轮 5 题里 2 题用户只写自由文本没点选项。此时**语义是主会话解读出来的**，
不是用户拍的板。

规则：

1. 解读结果写进 map 的 Notes / 决策历史，标明「用户未选项、系解读」，
   「我说」字段照实抄用户原话，不誊写解读后的版本。
2. 解读承担了方向性权重时（定对象、定终点这类），下一轮 ask-ui 顺带用一题
   复核「我理解你是 X，对吗」，不为省一题把解读当拍板。
