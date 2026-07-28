# release 手动重发支持 — PRD (主入口)

## 目标
- [x] release.yml 支持 `workflow_dispatch` 手动**重发**同版本: 现状手动 dispatch 时 `v<版本>` tag/release 已存在 → tauri-action 重建失败(撞 tag)。目标: 手动触发时先删旧 tag+release, tauri-action 重建成功; 版本号仍唯一读 `.version`(不加输入框, 不改版本源)。

## 边界
- [x] 范围内: 仅改 `.github/workflows/release.yml`。加 `cleanup` 前置 job (`if: workflow_dispatch`), `gh release delete "v$V" --cleanup-tag || true` 删旧 release+tag; `release` job `needs: cleanup` + `if: always() && needs.cleanup.result != 'failure'` 保 push 路径(cleanup 被 skip)照跑。
- [x] 范围外: 不加版本输入框(版本恒读 .version); 不动 matrix/构建/签名/发布文本; 不动 push 触发条件。
- [x] 约束: cleanup 仅 workflow_dispatch 生效, push(.version 变更)本就是新版本无碰撞跳过; `|| true` 幂等(删不存在 tag 不报错)。

## 验收标准
- [x] release.yml 加 cleanup job; release job needs+if 正确(push skip 不连带); `gh release delete` 带 `--cleanup-tag --yes`; YAML 语法有效; 版本仍读 .version 无输入框。

## 索引
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein subtask list release-manual-cleanup`)
