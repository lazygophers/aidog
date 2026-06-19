# Implement — platform-card-last-test-badge

## Checklist（拓扑序）

- [ ] **S1** 后端查询 + command + api 封装（sub-agent, worktree）
  - db.rs `get_last_test_result` / models.rs `LastTestResult` / lib.rs command + 注册 / api.ts 类型+方法
  - 验证: `cd src-tauri && cargo build && cargo clippy -- -D warnings && cargo test`
  - Review gate: invoke 参数名 camelCase 与 TS 对齐（跨 Rust↔TS 契约首现）
  - rollback: `git checkout -- <files>`
- [ ] **G1-S1** S1 验收：clippy/test 全绿 + api.ts 方法可被 S2 引用
- [ ] **S2** 前端徽章 + 事件 + 跨页刷新（sub-agent, worktree，依赖 S1）
  - usePlatformCards lastTestMap/refreshLastTest/事件监听 + handleQuickTest 派发
  - Platforms.tsx load 拉取 + 透传
  - PlatformCard 徽章 UI
  - Groups.tsx testOne 派发 / ModelTestPanel 派发
  - i18n key（若新增）8 locale 补全
  - 验证: `yarn build && yarn check:i18n`
  - rollback: `git checkout -- <files>`
- [ ] **G1-D1** D1 验收：手验三项（快速测试即时徽章 / 批量测试跨页 / 重启持久）
- [ ] bump .version（用户可见功能变更）
- [ ] 非平凡发现落 memory（测试结果徽章跨页刷新机制 / 事件名约定）
- [ ] worktree 合并 + 移除
