# PRD — 平台搜索命中只展示命中项

> 用户请求（/trellisx-flow）：搜索的时候，如果命中的是平台，应该只展示命中的平台，而不是这个分组下所有的
> 排队：撞 07-01-07-01-sensenova-platform（同 Platforms.tsx），等其 finish 后 start。

## 目标
搜索命中平台时只展示命中的平台，不连带展示其所在分组的全部平台。

## 现状（main 调研）
- Platforms.tsx 主列表 searchQuery（L1506）只滤 `standalonePlatforms`（未归属分组的），已分组平台在 `GroupsEmbedded` 内展示
- L975-995 搜索框是"添加/选平台下拉"（filtered 选项），非主列表搜索
- **待 exec 定位**：用户报"搜索命中连带整组"的具体搜索入口 — 可能是 Groups 页内嵌平台搜索、或某处 filter 命中平台后整组展开

## 交付项

### D1 — 定位搜索连带整组行为
- grep 全部搜索入口（Groups.tsx / Platforms.tsx / GroupsEmbedded 组件）找命中平台后展示整组的逻辑
- 确认具体 filter 函数 + 展开逻辑

### D2 — 改为只展示命中项
- 命中平台时：只渲染命中的平台卡/行，同组其他平台不连带展示（或折叠）
- 保持未搜索时原行为（整组展示不变）
- 注意：若命中维度含分组名（搜分组名 → 整组展开合理），需区分"命中平台"vs"命中分组名"两种语义

## 验收
1. 搜索某平台名 → 只该平台显，同组其他折叠
2. 搜索某分组名 → 整组展开（合理语义保留）
3. 清空搜索 → 恢复原展示
4. `yarn build` + tsc 0 error

## 非目标
- 不改添加/选平台下拉搜索（L975，那是选项过滤，正常）
- 不改主列表 standalonePlatforms filter（已正确）

## 风险
- 命中维度区分（平台 vs 分组名）需 exec 确认现有 filter 是否已区分；若混 → 拆开
- 搜索状态机可能耦 Groups 展开状态 → 改时注意不破坏展开/折叠持久

## 排队
撞 07-01-07-01-sensenova-platform（同 Platforms.tsx）。等其 finish 后 start。
