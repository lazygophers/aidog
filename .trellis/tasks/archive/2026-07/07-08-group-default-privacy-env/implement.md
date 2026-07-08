# Implement — group 预填隐私 env

## 工作目录

worktree: `.worktrees/07-08-group-default-privacy-env`（task.py start 后建）

## 步骤

### ST1: 定位 + 改新建 group form 初始值

grep 定位新建 group 代码：
```bash
grep -rn "setEditingGroup\|useState.*GroupConfig\|name:.*''" src/pages/Groups.tsx src/components/settings/editors.tsx
```

找到新建 group 的初始 GroupConfig/state，把 `env_vars: []` 改为：
```ts
env_vars: [
  { key: "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", value: "1" },
  { key: "CLAUDE_CODE_ENABLE_TELEMETRY", value: "0" },
  { key: "CLAUDE_CODE_ENHANCED_TELEMETRY_BETA", value: "0" },
  { key: "CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY", value: "1" },
  { key: "CLAUDE_CODE_BYOC_ENABLE_DATADOG", value: "0" },
  { key: "CLAUDE_CODE_PROPAGATE_TRACEPARENT", value: "0" },
  { key: "DISABLE_GROWTHBOOK", value: "1" },
  { key: "CLAUDE_CODE_ATTRIBUTION_HEADER", value: "0" },
  { key: "DISABLE_INSTALLATION_CHECKS", value: "1" },
]
```

**仅「新建」分支预填**，编辑分支保留 `group.env_vars`（不覆盖用户配置）。

### ST2: 补 i18n（5 env × 8 locale × 2 字段）

8 个 `src/locales/*.json` 各加 10 条（5 env × label+desc）：

| key | label_zh | desc_zh |
|---|---|---|
| DISABLE_GROWTHBOOK | 禁用 GrowthBook | 禁用功能开关/A-B 实验 |
| DISABLE_INSTALLATION_CHECKS | 禁用安装检查 | 跳过启动时安装完整性检查 |
| CLAUDE_CODE_BYOC_ENABLE_DATADOG | Datadog 上报 | 启用 Datadog 遥测上报（bring-your-own） |
| CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY | 禁用反馈调查 | 关闭定期反馈问卷 |
| CLAUDE_CODE_PROPAGATE_TRACEPARENT | 传播 traceParent | 向下游传播 W3C traceparent |

其余 7 语言照翻（en/ar/fr/de/ru/ja/es）。插在现有 env.* 条目中（按字母序或文件末尾，跟现有组织）。

### ST3: 验证

```bash
yarn build
cd src-tauri && cargo check  # 若改了 Rust（预期不改）
node scripts/check-i18n.mjs  # 若有
```

## 验收

见 prd.md。自检行 + 改动文件清单 + 验证输出。

## 失败处理

定位不到新建 form 入口 → 报告，标 `需要: main`。
i18n 翻译不确定 → 用英文兜底 + 标注。
build 报错 → 报告错行。
禁 git commit。
