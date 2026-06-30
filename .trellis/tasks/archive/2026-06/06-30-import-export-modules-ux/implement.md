# Implement — 执行编排

## Subtask 拆分 (按文件集互斥, 强依赖链 → 主要串行)

### ST1 — 后端: 3 新 scope + 导出逐项 (一个 trellis-implement, 串行内部)
**文件集**: `src-tauri/src/gateway/import_export/{mod.rs,collect.rs,apply/mod.rs}` + `src-tauri/src/commands/backup.rs` + `src-tauri/src/startup.rs` + 相关 `test_*.rs`
**产出**:
- mcp/middleware/model_price 三 scope: Payload 字段 + SCOPE_* 常量 + collect 收集 + build_items 枚举 + apply_db 写入
- export_preview command (collect 全量 → build_items conflicts空 → items)
- export_to_file 加 selection 参数 + collect 后 is_selected 过滤
- 注册新 command (startup.rs)
- 单测: 往返一致 + build_items/apply key 一致性
**验收**: `cd src-tauri && cargo test` import_export 相关绿 + `cargo clippy` 零 warning
**依赖**: 无 (可先跑)

### ST2 — 前端: IA 重组 + 导出逐项 UI (依赖 ST1 契约)
**文件集**: `src/components/settings/ImportExport.tsx` + `src/services/api.ts`
**产出**:
- ImportExportScope 加 mcp/middleware/model_price; exportToFile 加 selection; exportPreview 封装
- ALL_SCOPES 扩展; IA 按侧栏菜单分组 (平台三 scope 合并模块); setting items 按 scope 字段映射菜单组
- 导出前 export_preview 列 items → 逐条目勾选 (复用导入渲染, 默认全选) → exportToFile(selection)
**验收**: `yarn build` (tsc && vite build) 通过; 手测导出/导入往返
**依赖**: ST1 (command 契约 + selection 参数)

### ST3 — i18n 7 语言 (依赖 ST2 key)
**文件集**: `src/locales/*.json` (+ docs 按需)
**产出**: 新 scope label + IA 分组标题 + 导出逐项 UI 文案, 7 语言全补
**验收**: `node scripts/check-i18n.mjs` 零缺口
**依赖**: ST2 (确定 key 名)

## 调度

```
ST1 (后端) ──→ ST2 (前端) ──→ ST3 (i18n)
```
强依赖链, 主要串行。并发上限 2 但本链无并行机会 (契约逐级依赖)。
worktree: 默认 1 task 1 worktree, 三 subtask 共享。

## 失败处理
- ST1 cargo test/clippy 不过 → 读报告定点修, 同 agent 重试; 连续 2 次失败 → STOP 回传
- ST2 tsc 报边界类型错 → 对照 ST1 command 签名修字段名/类型 (Rust↔TS 边界)
- ST3 check-i18n 红 → 补缺失语言 key

## 跨层契约 (Rust↔TS, 重点防错)
- export_to_file: Rust `selection: Option<Vec<(String,String)>>` ↔ TS `[string,string][] | undefined`, invoke key camelCase `selection`
- export_preview 返回结构与 import preview 的 items 对齐 (ImportItem: scope/key/label/...)
- 新 scope 字符串值前后端逐字一致 ("mcp"/"middleware"/"model_price")
