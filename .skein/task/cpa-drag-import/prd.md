# CPA 配置导入拖拽支持 — PRD (主入口)

## 目标
- CpaImportModal 加原生文件拖拽导入: 拖入文件/压缩包/文件夹到 modal → 自动解析 → 预览累加
- 替代/补充现有「选择源」dialog 按钮(保留按钮, 拖拽为快捷路径)
- 用户价值: CPA 配置常多文件分散(多个 provider yaml/json), 一次性拖入全部比逐个 dialog 选源高效

## 边界
**范围内**:
- 整个 modal 区域接收原生拖拽(Tauri onDragDropEvent, 非 HTML5 DnD — macOS WKWebView drop 不触发)
- 拖入多文件 → 全部作为独立源, 逐个调 cpaImportApi.parse 累加预览行(多源叠加)
- 拖入即解析(无需再点「解析」按钮)
- auth-dir 凭据目录也支持拖入(区分目标: 拖到 auth-dir 按钮区 → setAuthDir; 其余 → 源 parse)
- dragActive 视觉反馈(enter/over 高亮 modal 边框 + 提示文案)

**范围外(非目标)**:
- 不改后端 cpa_import_parse 签名(单 path, 前端循环调用)
- 不改 parse 返回结构(MappedPlatform[] + skipped + source_files)
- 不加压缩包新格式支持(rar/7z 仍提示先解压)
- 不改 apply 链

**已知约束**:
- Tauri onDragDropEvent 是 webview 级事件, payload 只给 paths[], 无 DOM target 坐标
- HTML5 onDrop 在 macOS WKWebView drop 不触发(ImportExportTab:271 注释实证); onDragEnter/onDragOver 触发性待验(用于 auth-dir target 识别)
- auth-dir target 识别方案: HTML5 onDragEnter/onDragOver/onDragLeave 判 e.target 最近祖先是否 auth-dir 按钮 → dragTargetRef = "source"|"authdir"; Tauri drop 时读 ref。若 WKWebView HTML5 enter 也不触发 → auth-dir 拖拽退化为仅 dialog(源拖拽不受影响, 是主路径)

## 验收标准
- [ ] 拖入单个 yaml/json 文件到 modal → 自动解析, 预览表出现条目
- [ ] 拖入压缩包(zip/tgz/tar) → 自动解析(后端解压)
- [ ] 拖入文件夹 → 自动解析(后端递归)
- [ ] 拖入多文件 → 全部解析, 预览行累加(不覆盖前次)
- [ ] 重复拖入(第二次拖) → 增量累加, 不清空已预览行
- [ ] dragActive 视觉: 拖入悬浮时 modal 边框高亮 + 提示「松开以导入」
- [ ] auth-dir 按钮区拖入目录 → setAuthDir(若 WKWebView HTML5 enter 可触发; 否则退化 dialog, 文档标注)
- [ ] 拖入非配置文件(如 .txt) → 走后端 skipped, 不崩
- [ ] 8 locale i18n key 补全(check:i18n 全绿)
- [ ] yarn build 过(tsc + vite)
- [ ] modal 关闭重开状态清(现有逻辑, 拖拽新增状态也清)

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list cpa-drag-import`)
