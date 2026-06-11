# ST2: 验证 \n 两行 + 降级

- **目标**: 确认 \n 两行可行，不行降级单行
- **产出**:
  - 查 Tauri 2.0 TrayIcon::set_title 对 `\n` 的 macOS 行为（文档/源码/已知）
  - 渲染两行 → 保留 `\n`；仅单行/异常 → 降级 `{name} {second}`（单行），注释说明
  - cargo build 0
- **验证**: cargo build；逻辑两行优先+降级；GUI 实际渲染用户验
- **资源**: design.md、Tauri tray docs
- **依赖**: ST1
- **失败处理**: \n 不确定 → 代码两行优先 + 注释降级路径，标注待用户 macOS 验证
