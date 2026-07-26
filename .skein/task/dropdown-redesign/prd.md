# 下拉框 UI/UX 重设计 — PRD

## 目标
- 全 app 下拉框 (shadcn Select 33 处 + FilterDropdown + SearchableProtocolSelect 3 套) 按 example 玻璃+流光规范重设计
- 方案: Radix 换皮 (保留无障碍/键盘) + 扩展 EnhancedSelect (搜索/分组/多选)
- 交互增强: 搜索过滤 + 分组 + 多选模式 (用户确认全要)

## 边界
- 范围内: src/components/ui/select.tsx (Radix 换皮) + 新增 EnhancedSelect 组件 + FilterDropdown/SearchableProtocolSelect 视觉对齐
- 范围外: 33 调用点强制全改 (视觉自动跟随 select.tsx, 交互按需升级 EnhancedSelect)
- 约束: 保留 Radix 无障碍语义 (aria/键盘/焦点管理), 不退化
- 约束: example dropdown 是 vanilla JS, 不可直接搬, 需 React 层重做 CSS+交互

## 验收标准
- [ ] tsc 0 err / test 281 pass / build OK / check-i18n 0 缺译
- [ ] select.tsx Radix 换皮: trigger/content/option 对齐 example (玻璃底/流光描边/萤火虫选中态/slide 动画)
- [ ] EnhancedSelect 组件: 搜索过滤 + 分组 + 多选模式三能力
- [ ] FilterDropdown + SearchableProtocolSelect 视觉与 select.tsx 一致
- [ ] 无障碍不退化 (aria/键盘导航/焦点)
- [ ] 无 console error/warning
## 索引
- 详细设计: design.md (research 后生)
