# CPA 导入改为填表单模式 — PRD

## 目标
CPA 导入解析后不再直接批量创建。改为: 单 provider → 灌入创建平台表单(同 SmartPaste fullShare), 用户改配置 + 设分组 + 手动保存; 多 provider → 前端批量创建(同 runBatchCreateFromPaste 多 key 模式, 各 provider 独立平台, 用表单当前分组)。

## 用户价值
用户导入 CPA 配置后期望进创建表单页改其他配置(模型/endpoints/熔断/extra 等), 当前直接创建无法修改。group 问题(原 cpa-import-group-missing 触发点)在填表单模式自然解决(表单 GroupAssignSection)。

## 边界
**范围内**:
- CpaImportModal onApplied 改签名传选中 providers(MappedPlatform[]), 移除 cpaImportApi.apply 调用 + group badge
- PlatformEditForm onApplied 收 providers: 1 条 → applyCpaToForm 灌表单; >1 条 → runBatchCreateFromCpa 批量创建
- 新增 applyCpaToForm + runBatchCreateFromCpa(platformPasteApply.ts 或新 platformCpaApply.ts)
- modal 按钮文案动态(1 条「填入表单」/ N 条「批量创建 N 个」)+ 8 locale
- services/api cpaImportApi.apply 标注废弃(不再调用, 保留后端命令无害)

**范围外**:
- 不删后端 cpa_import_apply 命令(保留无害, 删破坏性)
- 不改 cpa_import_parse / MappedPlatform 结构
- 不改 SmartPaste / runBatchCreateFromPaste(仅复用模式)

**group 逻辑(填表单模式天然)**:
- 单条: 表单 GroupAssignSection 用户设(lockedGroupId/joinGroupIds/autoGroup), 走 handleSave 原路径
- 多条: runBatchCreateFromCpa 用表单当前 group(同 runBatchCreateFromPaste:300-301: joinIds=lockedGroupId!=null?[lockedGroupId]:joinGroupIds; auto=lockedGroupId!=null?false:autoGroup)

## 验收标准
- [ ] 导入 1 个 provider 选中应用 → modal 关, 创建表单开(预填 name/protocol/base_url/api_key/models), 用户可改配置 + 设分组 + 保存
- [ ] 导入 N(>1) provider 全选应用 → 批量创建 N 个独立平台(各 provider protocol/base_url/api_key), 用表单当前分组
- [ ] 单条填表单: group 走表单 GroupAssignSection(不再丢失)
- [ ] 多条批量: group 用表单当前设置(lockedGroupId/joinGroupIds/autoGroup)
- [ ] modal 按钮文案动态(1/N)
- [ ] 8 locale 按钮文案补全
- [ ] yarn build + check:i18n 全绿
- [ ] cpa_import_parse 行为不变(回归)
- [ ] SmartPaste 行为不变(回归)

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
