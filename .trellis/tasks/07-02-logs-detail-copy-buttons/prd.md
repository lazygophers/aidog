# PRD — 请求日志详情每个元素一键复制

## 背景
Logs 详情页展示请求全貌(MetaItem grid + attempts 时序 + RequestTabs 用户/上游各 5 块),现有仅「复制全部」(`copyDetail`)和列表行复制。用户要**每个元素**都能一键复制该元素的**具体内容**(如点上游 request body 的复制 → 剪贴板得该 body 内容)。

## 决策(用户已锁)
| 维度 | 决策 |
|---|---|
| 范围 | 全含:RequestTabs 5 块 × 2 tab + MetaItem 每个 value + attempts 每个尝试卡片 |
| UI | 每个 `<pre className="code-block">` 右上角浮动复制图标(GitHub 风)+ 复制成功 ✓ 闪烁反馈 |
| 复制内容 | 元素展示的具体内容(pre 内文本 / MetaItem value / attempt 摘要) |

## 交付
1. **CopyButton 组件**(Logs.tsx 内联或抽 shared):右上角浮动图标 svg + `writeText(text)` + copied ✓ 反馈(复用现有 `copied`/`copiedId` state 模式 + `@tauri-apps/plugin-clipboard-manager` writeText)。props:`{ text: string; title?: string }`,空 text 时不渲染。
2. **RequestSectionContent**(`Logs.tsx:873`)5 块 —— URL / 请求头 / 请求体 / 响应头 / 响应体的 `<pre>` 改为 `position: relative` 容器,右上角浮 `<CopyButton text={展示文本}>`。复制内容 = pre 当前展示文本(格式化 JSON string,即 `bodyStr(reqBody)` / `JSON.stringify(headers,null,2)` / url)。空/未捕获块不显示按钮(或 disabled)。
3. **MetaItem grid** —— 每个 MetaItem 加 CopyButton,复制 value 文本(展示值,如 platform name / "123 ms" / 时间串)。MetaItem 组件加可选 `copyText?` prop,有则渲染按钮。
4. **attempts 每个尝试卡片**(`Logs.tsx:379`)—— 右上角 CopyButton,复制该 attempt 摘要:`平台名 | 状态码 | 耗时ms | 错误(若有)` 组合串。
5. **i18n** —— 新增 key:`logs.copy`(复制,button title)+ `logs.copied`(已复制,反馈)。8 locale 全补(zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-PG),走 `scripts/check-i18n.mjs` 校验。

## 验收
- 详情页每个元素(5块×2tab + MetaItem + attempts)右上角有复制图标
- 点击 → 剪贴板得该元素具体内容(上游 reqBody 点复制 = upstream_request_body 内容)
- 复制成功图标变 ✓ 短暂反馈(2s 复位)
- 空 body / "(未捕获)" / "(streaming)" 块不渲染复制按钮
- attempts 每卡复制得摘要串
- `yarn build` + `scripts/check-i18n.mjs` 全绿;8 locale 新 key 全覆盖

## 非目标(YAGNI)
- 列表行复制(已有 `copyRow`)
- 全量复制(已有 `copyDetail`,保留)
- 单个 header key 独立复制(headers 整块一个复制按钮,非逐 key)
- 复制格式切换(原始 vs 格式化)—— 默认复制展示的格式化内容

## 调度(串行依赖)
与 `07-02-stats-logs-filter-unify`(P0,改 Logs.tsx 筛选区)+ proxy `P1`(改 Logs.tsx 筛选加无平台选项)**同文件 Logs.tsx** → write-files 相交,必须串行。
```
P0(stats-logs-filter-unify) → 本 task(logs-detail-copy-buttons) → proxy P1
```
active 集满 2(deeplink + P0),本 task **排队**:planning 完成,P0 finish 后 start。

## 风险
- CopyButton 浮动定位与 `<pre>` 滚动(overflow:auto)交互 —— 按钮须固定在 pre 容器(非滚动内容),`position:absolute` 贴右上,滚动时不飘
- attempts 卡片 grid 布局加按钮须不破坏现有列对齐
- i18n 8 locale 新 key 全补(ar-SA RTL 下图标位置确认)
