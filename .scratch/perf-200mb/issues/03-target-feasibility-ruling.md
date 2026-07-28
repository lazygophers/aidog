# 03 200MB 目标可达性裁定

Type: grilling
Status: open
Blocked by: 01, 02
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

拿 [01] 的实测分解与 [02] 的 WebView 下限，裁定「全进程总和 ≤200MB @ 50 路并发峰值」这个目标是否成立。

三种可能的结局，本票必须落其中之一：
- **成立** —— 锁死 200MB，后续票按这个预算分配额度（WebView X MB / tokenizer Y MB / SQLite Z MB / 其余）
- **口径需改** —— 例如改成「Rust 主进程 ≤200MB，全进程另定」，或「空闲 ≤200MB，峰值另定上限」
- **数字需改** —— 200MB 物理不可达，依实测重设一个有依据的目标

如果结论是「当前架构不可达」，则 map 中 **Not yet specified** 的「是否需要架构级手段」graduate 成新票；否则那条 fog 直接划入 out of scope。

这是 HITL 票——预算怎么分、口径怎么让步，是用户的取舍，agent 不得代答。

## 验收

- 一个明确的数字与口径，写进 map 的 Decisions so far
- 若成立：一张内存预算分配表，每项有上限
- 若不成立：新目标 + 为什么原目标不可达的一句话依据（引 [01]/[02] 的具体数字）
