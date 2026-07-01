# PRD — 导出默认勾选 skills（方案 C：导出+导入两侧删 filter）

> research: `.trellis/tasks/07-01-export-default-check/research/export-default-check.md`
> 用户决策：方案 C（导出+导入两侧删 skills filter），research 标不推荐但用户接受风险

## 目标
导出/导入 preview 默认勾选 skills scope（当前 `ImportExport.tsx:202` 导出 + `:304` 导入刻意 filter 排除 skills，引用 F2 导入误删修复防 npx 误触）。

## 决策（用户定）
- **方案 C**：删 `ImportExport.tsx:202`（导出 filter）+ `:304`（导入 filter）两侧 skills 排除
- mcp 实测已默认全选（无 filter），不改

## ⚠️ 风险（用户已接受）
- 导入侧 :304 删 = **失去 F2 npx 误删防御**：导入含 skills 时默认勾选 → apply 触发 npx 安装/删除，若用户误触确认可能误删现有 skill（[[skills-removal-cfg-test-hard-guard]] / [[skills-fs-fallback-delete-bypass-guard]] 历史 bug 同源）
- 缓解：apply 侧 npx 守卫（cfg(test) 硬拦 + fs 兜底物理删 guard）仍在，本 task 只动前端默认勾选，不动后端守卫

## 交付项
### D1 — 删导出 skills filter
- `ImportExport.tsx:199-205` 删 `.filter((it) => it.scope !== "skills")`（或改条件移除 skills 排除）
- 导出 preview 展开后 skills 条目默认勾选

### D2 — 删导入 skills filter
- `ImportExport.tsx:301-307` 删导入侧对称 skills filter
- 导入 preview 展开后 skills 条目默认勾选

### D3 — 验证 scope 级初始勾选（research 提及 :159-161）
- 确认 scope 卡片级初始勾选是否也排除 skills/mcp；若是，按方案 C 语义同步（但用户选 C 仅针对条目级 filter，scope 级 :159-161 默认 platform/group/group_platform/setting —— check 时确认是否需同步，无需则不动）

## 验收
1. 导出 preview：skills 条目默认勾选（无需手动勾）
2. 导入 preview：skills 条目默认勾选
3. mcp 行为不变（一直默认全选）
4. `yarn build` 绿 + tsc 0 error
5. F2 守卫（后端 npx cfg(test) + fs guard）未动，仍生效

## 非目标
- 不改后端 export_preview（无 defaultChecked 字段，100% 前端）
- 不改后端 apply / skills_sync 守卫
- 不改 mcp 逻辑（已默认全选）

## 风险
- 导入侧默认勾 skills + apply npx → 用户误触风险升（用户接受，后端守卫兜底）
- check 时验证 D3 scope 级是否需同步
