# 代理卡片合并 URL 输入 — PRD

## 目标
- [ ] Coding 设置代理卡片 UI 精简：HTTP_PROXY/HTTPS_PROXY/ALL_PROXY 三键合并为单一「代理 URL」输入（同值写三键），NO_PROXY 单独输入。消除用户面对 3 个几乎总同值的输入框的冗余。

## 边界
- [ ] 范围内：CodingToolsSettings.tsx 代理卡片 state/load/handler/JSX 改 2 输入；i18n 8 locale 的 codingTools.proxy.desc 文案更新（反映合并语义）。
- [ ] 范围外：不改 proxy-env.ts（codex 命令注入读 4 键分别 export，三键同值 export 无害，YAGNI）；不改 i18n key 结构（沿用 .title/.desc）。
- [ ] 约束：读时三键不一致取首个非空（HTTP_PROXY→HTTPS_PROXY→ALL_PROXY）作 URL 显示；写时三键同值覆盖。

## 验收标准
- [ ] yarn build 通过
- [ ] scripts/check-i18n.mjs 通过
- [ ] yarn test 全过
- [ ] 代理卡片 2 输入：URL（onChange 更新 draft，onBlur 写 HTTP/HTTPS/ALL_PROXY 三键同值）+ NO_PROXY（单独）
- [ ] 失败回滚 + 常驻错误态沿用 runCommit

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json
