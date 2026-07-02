# PRD — D4 skills 分享 + URL 导入

> parent: 07-01-aidog-deeplink-share。依赖 D1(协议框架)+ D3(ShareModal 泛化)。

## 目标
1. **skills 分享按钮**(catalog id 列表语义，用户已锁) — Skills.tsx skill 卡片/详情加分享按钮 → 弹泛化 ShareModal(D3 已泛化) → skill id(owner/repo@skill)序列化 base64 + 生成 `aidog://skill/import?data=<base64>` URL
2. **skills URL 导入** — App.tsx 订阅 `aidog:skill` → 解码 base64 → id 列表 → 批量 `skills_install`(弹确认对话框列出将装的 skill)
3. **skills 粘贴导入** — Skills.tsx 粘贴入口(粘贴 base64 → 解码 → install)

## 决策(已锁)
| 维度 | 决策 |
|---|---|
| 分享语义 | **catalog id 列表**(用户确认，非文件打包) — 分享 owner/repo@skill，接收方 npx install |
| URL 编码 | 明文 base64(id 列表 JSON) |
| 导入路径 | 复用 skills_install{id,agents,scope}(api.ts:1725 现成) |
| ShareModal | 复用 D3 泛化版(本 task 不改 ShareModal) |

## 已知(codebase 实证)
- skills_install(api.ts:1725)接 `{id,agents,scope}` → SkillsOpResult；id = `owner/repo@skill`
- Skills.tsx 无多选 selected(只 :192 open projectPath)；skill list 由 skillsApi.list 提供(installed + catalog)
- SkillDetailView.tsx(10.8K)/SkillInstallView.tsx(12.4K) 详情/安装视图
- App.tsx D1 已 dispatch `aidog:skill` CustomEvent
- D3 ShareModal 泛化后接任意对象 + title

## 交付
1. **Skills.tsx 分享按钮** — skill 卡片/详情(SkillDetailView)加分享图标 → 序列化 skill id(单条;或选中列表批量→id 数组)→ 弹泛化 ShareModal(title=skill 名)+ 「复制为 aidog:// URL」(拼 `aidog://skill/import?data=<base64>`)
2. **App.tsx 订阅 `aidog:skill`** — detail.data base64 → atob → JSON(id 或 id[])→ 弹确认对话框(列出将装 skill)→ 批量 skills_install(默认 agents/scope，或弹让用户选 scope)→ 汇总结果 toast
3. **Skills.tsx 粘贴导入入口** — 工具栏加「从分享导入」按钮 → 粘贴 base64 → 解码 → 同 #2 install 流程
4. **i18n** — `skills.share` + `skills.share.copyUrl` + `skills.importFromShare` + `skills.importConfirm`(列出将装 skill)，8 locale 全补

## 验收
- skill 卡片分享按钮 → ShareModal → base64 + URL
- 点 `aidog://skill/import?data=<base64>` → 唤起 → 确认对话框 → 批量 npx install → 结果 toast
- Skills.tsx 粘贴 base64 → 导入流程
- 接收方未装该 skill 时 install 成功；已装 → 跳过或提示
- 4 门禁全绿

## 非目标(YAGNI)
- skill 文件打包(用户定 id 列表)
- 分享已装 skill 的本地私改(仅 catalog id 引用)
- 加密配对码
- skill scope 复杂选择(默认 global 或弹简单选择)

## 调度(串行)
- write-files: `src/pages/Skills.tsx` + `src/pages/SkillDetailView.tsx` + `src/App.tsx` + `src/locales/*.json`
- 与 D2/D3 撞 App.tsx；依赖 D3 ShareModal 泛化 → **D2 → D3 → D4 串行**

## 风险
- 接收方 npx 环境未就绪(skills_check_env 失败)→ toast 引导装 npx
- catalog id 失效(owner/repo 删库)→ install 失败 toast 透明报错
- 批量 install 部分失败 → 汇总成功/失败清单
