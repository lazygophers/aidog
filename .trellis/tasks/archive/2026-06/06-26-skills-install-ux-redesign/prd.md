# SkillInstallView 交互重设计

## 背景
合并三个用户反馈 (全属 `src/pages/SkillInstallView.tsx`):

### Bug 1: 搜索 loading 期间显旧数据
「在搜索等待的过程中，直接展示了数据，而不是加载中的样式，会引起用户误会」
- line 42 `loading` state, line 158 `{loading && (...)}` loading 样式块
- **核实现状**: loading=true 期间旧 results 是否仍渲染? (line 203/215 的 `!loading` 条件可能已隔离, 但 results 列表渲染区需查)
- 修复方向: loading 时隐藏 results (`!loading` 条件) 或清旧 results 或叠 skeleton

### Bug 2: 已安装 skill 仍显安装按钮
「已安装的没有显示已安装还是提供了安装的按钮」
- line 25 `installedNames: Set<string>` prop, line 229 `const already = installedNames.has(entry.name)`, line 233 disabled 含 already, line 249 `{already && (...)}` 显已装
- **现状已有 already 判断** → bug 可能: installedNames 未正确传入 (Skills.tsx 父组件没传最新) / name 不匹配 (catalog entry.name vs installed skill name)
- agent 查 Skills.tsx 传 installedNames 处 + name 匹配逻辑

### Bug 3: 并发点击丢安装中状态
「连续点击两个安装按钮，第一个安装还没有完成就取消了安装中的状态展示」
- line 47 `busyId` 单值, line 150 `disabled={busyId !== null}` — **现状已是"busyId!=null 全禁"防并发方案**
- **可能已修** (line 150 全禁 = 点第二个时所有按钮禁用, 不会覆盖 id1)。agent 核实若已修则跳过; 若仍单值覆盖, 改 busyIds Set 或确认全禁方案生效

## 需求
agent **先逐 bug 核实现状** (pending 定位 9 天前, 代码可能已部分修), 确认是真 bug 再修; 已修的跳过说明。

## 工作目录与范围
**cd 进 task worktree**: `/Users/luoxin/persons/lyxamour/aidog/.worktrees/06-26-skills-install-ux-redesign` (主仓零改动)
范围: `src/pages/SkillInstallView.tsx` 为主 + `src/pages/Skills.tsx` (installedNames 传入) + 可能 `src/services/api.ts` (name 类型对齐)。前端纯 UX, 无后端改。

## 输出格式
回报: (1) **三 bug 现状核实** (每 bug: 真bug/已修/误报 + 证据 file:line) (2) 修复方案 + 改动 diff (3) 改动文件清单 (4) yarn build + check-i18n 结果 (5) 自检行

## 验收
- 三 bug 逐个核实 (真 bug 必修, 已修说明)
- yarn build 通过, check-i18n 零缺失
- 新增/改 i18n key 8 locale 全覆盖 (若有文案改)
- 主仓零改动

## 失败处理
- 某 bug 无法复现 (代码已修) → 标 "已修, 跳过" + 证据, 不强改
- installedNames 传入链路查明父组件确实没传 → 修父组件; 若传了但 name 不匹配 → 修匹配逻辑; 不臆造
