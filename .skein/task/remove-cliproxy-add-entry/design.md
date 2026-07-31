# 移除添加平台表单里的「从 cli-proxy 添加」入口 — 详细设计

## 现状

cli-proxy 平台目前有两个创建入口：

1. `src/pages/CliProxy/index.tsx:429` — provider 行的「建平台行」按钮 → `handleCreatePlatform`（`:176`）
2. `src/pages/platforms/PlatformEditForm.tsx:157` — 新建平台表单页头的「从 cli-proxy 添加」按钮 →
   打开 provider picker Dialog（`:182-219`）→ `createCliProxyPlatform`（`usePlatformForm.ts:789`）

两条路做的是同一件事。入口 2 语义上是从「添加平台」流程里旁路跳到另一个数据源，
用户在添加平台时要先判断「这个平台算不算 cli-proxy」才知道该点哪个按钮。删入口 2，
创建路径收敛到 CliProxy 页自身。

## 删除面

`PlatformEditForm.tsx` 四块，都是入口 2 独占：

- `:155-159` 按钮（`!editing` 分支内，与「智能识别」并列）
- `:182-219` picker `<Dialog>`
- `:107` `showCliProxyPicker` state、`:105` `cliProxyProviders` state
- `:110-119` picker 打开时拉 provider 列表的 effect

`usePlatformForm.ts:789` 的 `createCliProxyPlatform`：删前先 grep 全部调用点。
若只有 picker 一处 → 连同 prop 链路删干净；若另有引用 → 保留并在改动说明里写明依据。
不猜，以 grep 结果为准。

## 明确保留

- **编辑态只读展示**：`isCliProxyEditing`（`:108`）及其 provider 反查 effect（`:121-133`）、
  继承字段区（`:269-300`）。已有 cli-proxy 平台仍要能看能编，这块与新建入口无关。
- **CliProxy 页「建平台行」**：`index.tsx:176/429` 与后端 `cli_proxy_cmd` 的 create_platform
  command 全部不动 —— 删掉入口 2 后它是唯一创建路径，动它等于把功能删没。

## i18n

删仅入口 2 用的 key：`platform.cliProxy.addFromProvider` / `pickerTitle` / `pickerHint` /
`pickerEmpty`，8 个 locale 各删一遍。

`platform.cliProxy.inherited*` / `provider` / `wireProtocol` / `baseUrl` / `models` 是编辑态在用的，
**不能删**。逐 key grep 确认引用点归零再删 —— 误删仍在用的 key 会让编辑态直接露出裸 key 文本。

## 测试接缝 (seam)

不新建接缝。这是纯删除，没有新增逻辑分支值得单测：

- `scripts/check-i18n.mjs` 覆盖「key 删干净且 8 语言对齐」与「没有引用点指向已删 key」两个风险，
  是本 task 唯一实质风险（误删仍在用的 key）的直接检测器
- `yarn build`（tsc）覆盖「state / prop 删干净无悬空引用」
- `yarn test` 跑现有 18 个测试文件，确认没碰坏别的

人工确认一条：编辑一个已有 cli-proxy 平台，继承字段区正常显示、无裸 key。
