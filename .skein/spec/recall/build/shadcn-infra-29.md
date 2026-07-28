---
title: vite @ alias 手动配置
layer: recall
category: build
keywords: [vite,alias,resolve,shadcn,tsconfig]
source: shadcn-infra
authored-by: skein-spec
created: 1784706688
status: active
related: []
updated: 1784706688
---

# vite @ alias 手动配置

## 触发场景
使用 shadcn/ui 或其他假设存在 `@` 别名的库时，项目原无 `@` → `src` 的路径别名配置，导致 `import @/components/xxx` 解析失败。

## 陷阱-正解
- **陷阱**: shadcn 假设 vite 已有 `@` 别名（标准 scaffolding 如 Vite 默认模板已配），但本项目从零开始或迁移时可能缺失
- **正解**: 手动在 `vite.config.ts` 添加 alias resolve：
  ```ts
  export default defineConfig({
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src')
      }
    }
  })
  ```
- 同步 tsconfig `paths`: `"@/*": ["./src/*"]`（TS 类型解析）

## 反例
❌ 只配 vite alias 不配 tsconfig → 类型检查报错
❌ 用相对路径 `../../components` → 不符合 shadcn 假设，后续组件难以维护

## 案例
- shadcn-infra task: shadcn 生成的组件含 `import @/components/xxx`，本项目无 `@` 别名导致 TS 错误

## 适用
shadcn/ui 迁移、Vite 从零配置、路径别名标准化

## 关联
[[shadcn-infra-28]] (同任务 cva 依赖)
