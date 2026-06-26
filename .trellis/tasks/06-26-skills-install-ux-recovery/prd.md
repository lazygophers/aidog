# SkillInstallView 改动恢复

## 背景
前序 task 06-26-skills-install-ux-redesign agent 完成实现但**未 commit worktree**, task.py finish → archive hook 销毁 worktree, 改动物理丢失 (git dangling 无残留)。本 task 重做同样改动。

## 需重做的改动 (前序 agent 已验证 diff)

### 1. `src/pages/SkillInstallView.tsx`
- **Bug 1 (loading 显旧)**: 结果列表渲染条件加 `!loading` 前置
  - `{effectiveResults.length > 0 && (` → `{!loading && effectiveResults.length > 0 && (`
- **Bug 3 (并发 busy)**: 加并发锁防 setBusyId 覆盖
  - 在 `installing = busyId === entry.id` 附近加 `const otherBusy = busyId !== null && !installing;`
  - disabled 改: `installing || otherBusy || !writeReady || already || noAgent`
  - 安装按钮加 `title={otherBusy ? t("skills.install.busyOther") : undefined}`

### 2. 8 locale 加 `skills.install.busyOther`
- zh-CN: "等待当前安装完成"
- en-US: "Waiting for current installation to finish"
- ar-SA: "في انتظار اكتمال التثبيت الحالي"
- de-DE: "Auf Abschluss der aktuellen Installation warten"
- es-ES: "Esperando a que termine la instalación actual"
- fr-FR: "En attente de la fin de l'installation en cours"
- ja-JP: "現在のインストール完了を待っています"
- ru-RU: "Ожидание завершения текущей установки"

## 关键
- **必须 commit worktree** (前序漏 commit 致丢, 本次务必 git add + commit)
- Bug 2 (已装仍显安装按钮) 链路完整跳过 (Skills.tsx:777 传 installedNames 正确)

## 验收
- SkillInstallView.tsx Bug 1 + Bug 3 改动落盘
- 8 locale busyOther key 全覆盖
- yarn build + check-i18n 全绿
- **worktree 必须 git commit** (finish 前确认 status 干净)
