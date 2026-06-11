# ST4: 迁移生效 + 删卡片开关 + 测试

- **目标**: 旧配置迁移 + 删平台卡片 tray 开关 + 测试
- **产出**:
  - 删 Platforms.tsx tray 开关 UI（show_in_tray/tray_display 编辑，功能移设置页）；旧列保留（迁移读兼容）
  - 迁移验证：升级后无 tray config → 自动从旧 show_in_tray 生成默认（ST1 逻辑）端到端通
  - 测试：TrayConfig 序列化往返 / 渲染多 item 文字组装 / today_token_total / 颜色三态映射 / 迁移默认生成
- **验证**: cargo test + tsc 0；端到端配置→渲染
- **资源**: research/06-migration.md、design.md
- **依赖**: ST2, ST3
- **失败处理**: 删开关注意别窗口 Platforms.tsx 冲突；测试失败修代码
