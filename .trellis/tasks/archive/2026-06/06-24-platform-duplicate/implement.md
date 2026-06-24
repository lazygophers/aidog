# Implement — 平台复制功能

单一交付, 纯前端, 单 worktree 单 writer + 单 checker。

## 触点 (3 处)
1. **src/components/platforms/PlatformCard.tsx**
   - `PlatformCardActions` interface 加 `onDuplicate: (p: Platform) => void;` (L17-31 区)
   - 卡片操作区 (L263-278, 查看日志/编辑/删除按钮串) 新增「复制」图标按钮: `className="btn btn-ghost btn-icon"`, `title={t("platform.duplicate", "复制")}`, `onClick={(e) => { e.stopPropagation(); actions.onDuplicate(p); }}`。位置建议: 编辑按钮左侧或右侧。图标用「双方块/copy」SVG (14×14, stroke currentColor strokeWidth 1.5), 与现有按钮风格一致。

2. **src/pages/Platforms.tsx**
   - 新增 `handleDuplicate = async (p: Platform) => {...}`: 复制 `handleEdit` (L1922-1973) 的**全字段 state 灌入**逻辑, 唯一差异 **`setEditing(null)`** (替代 `setEditing(p)`)。包括: name/protocol/apiKey/models/availableModels/endpoints/extra/mockConfig/newApiConfig/manualBudgets/breaker/codingPlan/joinGroupIds(反查手动组,同 handleEdit L1950-1960)/lockedGroupId=null/setShowForm(true)。
   - Claude 配置: 同 handleEdit 加载 `platform:p.id` + global 的 claude_code 合并填入 `claudeConfigJson`; 仅当存在非空 override diff 时 `setShowClaudeConfig(true)` (令保存逻辑 L2080-2096 把 override 落到新平台 id), 否则不强开。
   - 把 `handleDuplicate` 注册进 `actionsRef.current` (L2244-2252 两处对象) + `cardActions` useMemo (L2253-2266) 加 `onDuplicate: (p) => actionsRef.current.handleDuplicate(p)`。
   - 复用现有 `handleSave` create 分支, 不改 handleSave。

3. **src/locales/{zh-CN,en-US,ar-SA,de-DE,es-ES,fr-FR,ja-JP,ru-RU}.json** (8 个)
   - 在 `platform` 命名空间加 `"duplicate"` key, 各语言译文 (zh-CN="复制", en-US="Duplicate", 其余按语言)。ar-SA 注意 RTL 不影响 key 值。

## 约束
- 纯前端, **禁改 Rust/db/Tauri command**。
- 不破坏现有 handleEdit / handleSave; handleDuplicate 与 handleEdit 高度同构, 可抽公共灌入函数或直接复制 (优先不过度抽象, 复制+改 editing 即可, 但若抽公共函数须保证 handleEdit 行为不变)。
- worktree 隔离执行, 主工作区零改动。

## 验证 (checker)
- `yarn build` (tsc && vite build) 通过, 零 tsc error。
- `node scripts/check-i18n.mjs` 通过 (8 locale platform.duplicate 齐, 无裸 key)。
- 人工核对验收标准 1-5 (见 prd.md): 按钮存在 / 复制为新建态 / 取消无副作用 / 保存生独立平台 / 分组归属一致。
