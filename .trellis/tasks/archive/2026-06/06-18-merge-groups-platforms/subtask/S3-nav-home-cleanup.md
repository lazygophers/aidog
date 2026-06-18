# S3: 侧栏 + Home 清理（移除 groups 入口）

## 五要素

- **目标**：侧栏移除 `groups` 项，只留 `platforms`。App.tsx render 删 groups 分支。Home.tsx 删「分组」按钮（L448），保留「平台」按钮（L445）。
- **产出**：`src/App.tsx`（BASE_NAV 删 groups 项 L27；render 删 groups 分支 L147）；`src/pages/Home.tsx`（删分组按钮 L448 + 卡片布局调整）。
- **验证**：dev 侧栏只见「AI 平台」；Home 只剩平台入口；点平台进平台页（含内嵌分组段）；无残留 groups 路由白屏。
- **资源**：`src/App.tsx`；`src/pages/Home.tsx`；grep 确认无其他 `onNavigate('groups')` 残留。
- **依赖**：无（文件独立，与 S1/S2 并行）。

## 现状线索

- BASE_NAV groups 项：`App.tsx:27`。
- render groups 分支：`App.tsx:147` `{effectiveNav === "groups" && <Groups .../>}`。
- effectiveNav 无 groups 特殊 fallback（仅 logs/notifications → platforms，L120-121），无需改。
- Home 按钮：`Home.tsx:445`(platforms) / `:448`(groups)。
- `nav.groups` i18n 键**不删**（防他处引用；侧栏不再露出即可）。

## dispatch prompt

```
目标：侧栏移除 groups 项只留 platforms；Home 删分组按钮；App render 删 groups 分支。
已知：BASE_NAV groups 项 App.tsx:27；render 分支 L147；Home 分组按钮 L448（平台按钮 L445 保留）。effectiveNav 无 groups fallback 不动。nav.groups i18n 键保留不删。Groups import 若 App.tsx 不再用则删 import 行。
工作目录与范围：src/App.tsx + src/pages/Home.tsx。禁改 Groups/Platforms 页面/后端/i18n 文件。
输出格式：diff + dev 侧栏/Home 行为描述。
验收标准：侧栏单项；Home 单按钮；无 groups 路由残留白屏；tsc 0 error（删 Groups import 后无 unused）。
失败处理：Home 删按钮后卡片布局空 → 调整网格列数；App 删 import 致 unused → 一并删。
```
