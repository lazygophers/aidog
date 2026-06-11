# ST1: 渲染微调（删剩 + 第二行右对齐）

- **目标**: coding 纯数字 + 第二行值右对齐
- **产出** (lib.rs):
  - coding value lib.rs:1311 `"剩 {:.0}%"` → `"{:.0}%"`（删"剩"）
  - 两行模式第二行(值)右对齐：NSTextTab 值列用 RightTabStopType（location=列右边界），标签列保持 left；标签左对齐起、值右对齐到列末
- **验证**: cargo build + tsc 0
- **资源**: design.md、lib.rs:1311(coding)/两行渲染 tabStops 区
- **依赖**: 无
- **失败处理**: RightTab API 查 objc2-app-kit NSTextTabType::RightTabStopType
