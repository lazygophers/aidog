# Implement: 分组内嵌进平台页

## 有序 checklist

1. **S1 Groups 内嵌重构**（先行，解锁 S2）
   - [ ] Groups.tsx 导出 `GroupsEmbedded({onNavigate, onGroupsChanged})`
   - [ ] 去掉整页 section-header 外框，改子区块标题（复用 page.groups）+ 计数
   - [ ] proxyBaseUrl 条 + 添加分组按钮移子区块标题行右侧
   - [ ] 分组平台成员变更处（saveEdit/handleAddMapping/handleDeleteMapping/setPlatforms 等）调 `onGroupsChanged?.()`
   - [ ] dev 自测 GroupsEmbedded 全交互

2. **S3 侧栏/Home 清理**（并行 S1）
   - [ ] App.tsx BASE_NAV 删 groups 项（L27）
   - [ ] render 删 groups 分支（L147）+ 删 Groups import（L6）
   - [ ] Home.tsx 删分组按钮（L448）+ 卡片布局调整
   - [ ] dev 侧栏单项 / Home 单按钮

3. **S2 Platforms 植入**（S1 完成后）
   - [ ] grep Platforms.tsx 现有 group 加载（auto-group），复用或新增 groupDetailApi.list()
   - [ ] 构建 membership: platformId → groupNames[]
   - [ ] 列表视图（L2963+）顶部插 `<GroupsEmbedded onNavigate onGroupsChanged={refreshMembership}/>`
   - [ ] 平台列表项加所属分组 badge（badge-muted，N:N 多 badge）
   - [ ] dev：分组段全功能 + badge 正确 + 分组改平台后 badge 同步

## 验证命令

```bash
npx tsc --noEmit
node scripts/check-i18n.mjs
yarn tauri dev    # 手测：侧栏单项/分组段全交互/平台badge/N:N同步/7语言zh,en,ar抽测
```

## review gate

- Groups 全功能零回归：拖拽排序/统计聚合/模型映射/中间件规则/编辑保存/添加/删除。
- Platforms 编辑表单零回归（770 行不动）。
- N:N：一平台属多分组，各分组卡片 + 平台 badge 双向可见。

## rollback

单 commit；`git revert <hash>`。无后端/迁移，零副作用回滚。
