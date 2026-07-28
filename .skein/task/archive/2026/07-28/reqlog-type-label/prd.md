# 请求记录类型/协议显示直观 — PRD

## 目标
- Logs/RequestLog 表格 + 详情显示的 protocol 裸值 (anthropic/openai/glm/codex...) 改直观显示名
- 复用 getProtocolLabel (src/domains/platforms/defaults.ts:200, async + locale fallback)
- label 已是中文 (requestLog.typeTest=测试/typeQuota=余额) 不动; 只改 protocol 裸值显示

## 边界
- 范围内: src/pages/Logs/DetailPanel.tsx (source_protocol/target_protocol MetaItem) + src/pages/RequestLog.tsx (表格/详情 protocol 列 + markdown 导出)
- 范围外: 其他页 protocol 显示 (PlatformCard/ModelTestPanel 已用 protocolLabel state, 非 scope)
- 约束: getProtocolLabel async, 调用方需 useEffect+state 或顶层数据预解析; markdown 导出保留原始值 (审计可读)

## 验收标准
- [ ] tsc 0 err / test 281 pass / build OK
- [ ] Logs DetailPanel 「用户格式」「请求格式」显示直观名 (非裸 anthropic/openai)
- [ ] RequestLog 表格/详情 protocol 列显示直观名
- [ ] 无 console error/warning
- [ ] check-i18n 0 缺译 (复用现有 protocol name, 无新 key)
## 索引
- 详细设计: 复用 getProtocolLabel, 单文件改
