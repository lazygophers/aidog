# CPA 多 provider 导入改确认+多平台编辑页 — PRD (主入口)

## 目标
CPA 配置导入多条 provider 时, 从「直接批量创建(无编辑)」改成「手风琴编辑页逐项确认后底部批量保存」。用户价值: 多 provider 导入后能逐项核对/改模型/改名称等再落库, 避免脏数据批量灌入需逐条删改。

成功长什么样:
- N provider 导入 → 进手风琴编辑页 → 逐项展开改配置 → 底部「全部保存」一次性创建 N platform
- 每项复用现有添加平台编辑表单(formSections/usePlatformForm), 仅 token 区按 provider 类型(OAuth vs api-key)差异化渲染

- [x] 目标已定

## 边界

**范围内**:
- CPA 多 provider(≥2)导入路径: CpaImportModal onApplied 多条分支从 `runBatchCreateFromCpa` 改为打开手风琴编辑页
- 手风琴容器: 同页纵向展开, 同时只展开 1 项(折叠其余), 顶部 provider 切换条
- 每项复用 PlatformEditForm/formSections/usePlatformForm 编辑能力(名称/模型/endpoints/extra 等全字段可改)
- token 区差异化: OAuth provider(cpa-* 协议)token 只读掩码展示(来自 auth_dir access_token); 非 OAuth provider api_key 可编辑输入框
- 底部统一批量保存: 遍历 N 项触发各自 handleSave(create), 汇总成功/失败报告
- 部分失败: 已成功的不回滚, 失败项标红留页内继续改+重试保存

**范围外(非目标)**:
- 单 provider 路径不变(仍 applyCpaToForm 灌单条创建表单)
- 后端 cpa_import_parse/apply 不改(纯前端改 onApplied 分派 + 新编辑容器)
- runBatchCreateFromCpa 保留(底部批量保存复用其 create 循环逻辑, 或直接复用 handleSave)
- 不改 SmartPasteModal / MultiKeyPreview(多 key 预览是另一路径)

**已知约束**:
- MappedPlatform 无 oauth_type 字段, 靠 protocol 区分(cpa-* 协议 = OAuth; 注释 part4.ts:19 "OAuth = access_token")
- usePlatformForm 是大 hook(43.9K, 内聚表单 state + handlers), 多实例化需每项独立子组件挂载
- handleSave 内部调 platformApi.create, 多实例并发保存无外层 state 冲突(创建后 refreshPlatforms 各实例触发, 批量场景可接受)
- 手风琴容器组件需协调 N 子组件的保存触发(子组件 forwardRef 暴露 save() 或父级遍历调)

- [x] 边界已定

## 验收标准
- [ ] CPA 导入 ≥2 provider → 进手风琴编辑页(非直接批量创建)
- [ ] 手风琴同时只展开 1 项, 折叠其余; 顶部切换条点 provider 名跳该项
- [ ] 每项展开 = 完整添加平台编辑表单(名称/协议/endpoints/models/extra 全字段可改), 复用 formSections
- [ ] OAuth provider(cpa-*) token 区只读掩码(非可编辑输入框); 非 OAuth api_key 可编辑
- [ ] 底部「全部保存」→ N platform 创建; 成功项关闭, 失败项标红留改
- [ ] 部分失败: 成功不回滚, 失败可改后重试(只重存失败项)
- [ ] 取消/关闭编辑页 → 确认弹窗(防误丢 N 项改动)
- [ ] yarn build 过(tsc + vite); check:i18n 全绿(新 key 8 locale 补全)
- [ ] 单 provider 路径回归(applyCpaToForm 灌表单不变)

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list cpa-import-multi-edit`)
