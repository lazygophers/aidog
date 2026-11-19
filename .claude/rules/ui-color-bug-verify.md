# UI 配色 bug：静态推理两轮不自洽就改跑真渲染读 computed style

2026-09-29 深色模式 Select 选中项不可见（7a9b860fe）暴露的排查模式缺陷。
select.tsx → mono.ts → tokens.generated.ts → globals.css 的变量链 + cascade
顺序静态推理，连续产出三版互相矛盾的结论，每版单看都「说得通」；最终 vite
dev + Playwright（系统 Chrome channel）起真 app 读 getComputedStyle，30 分钟
内定论两处真因：① 选中项底色是 `--accent-subtle` 缺省（近黑 `--primary` 13%
混出），dark 下近黑不可见；② 键盘高亮项 `focus:bg-accent` 落到 `--accent` =
`accent-text`（dark `#E6E8EC` 近白），出近白块。

## 规则

1. **配色 bug（「某状态看不清 / 颜色不对」）的静态推理产物是假设不是结论。**
   同一问题推理到第二版与第一版矛盾，就停手，改跑真渲染读 computed style。
   「继续推 cascade」永远比「起 dev server 量一下」贵。
2. 取证跑法：`yarn dev` 起 vite，Playwright `chromium.launch({ channel: "chrome" })`
   开页面，对目标元素逐状态触发（选中 / 键盘高亮分别触发）后读
   `getComputedStyle(el).backgroundColor`（及 color）。**深浅两 mode 各读一遍**——
   两 mode 走的 token 分支不同，只验一个会漏另一半。
   本机 playwright 未装 chromium headless shell，默认 `chromium.launch()` 起不来；
   一切 Playwright 截图 / 渲染验证（含 huashu-design 的 design_lint、渲染类票截图验收）
   一律带 `channel: "chrome"`，跑不成就报「做了没验证」，不静默跳过（2026-11-06
   模型配置重设计轮：design_lint 因缺 headless shell 未跑成，huashu-design 收工
   三件套缺 lint）。
3. 结论落在读出来的值上（「13% 透明近黑」），不落在推理链上（「按顺序应该被
   覆盖」）。见 CLAUDE.md 的 accent token 语义条目：本仓 `--accent` 是文字色，
   shadcn `bg-accent` idiom 在 dark 下系统性错位，静态推理最容易在这里翻车。
4. **agent 自报的视觉验证结论（无遮挡 / 无重叠 / 动画正常）采信前自己真渲染
   截图核对几何。** 2026-11-14 节假日日历轮：验收 agent 报「无重叠」，截图
   反证关闭按钮 X 压住 UTC 按钮；reduced-motion 动画同样靠真组件浏览器几何
   断言（相交断言先红）才修干净。几何类断言红 = 真先红，比「目测正常」强一级。

## 全库同类 bug 审计流程（2026-09-30 accent 全量修复 3411cbc1a）

发现一个 accent 误用后，同 session 内做全库清查：

1. **先分类再处理**：`grep -r 'bg-accent\|var(--accent)' src/` 找全部候选，然后按两类分开：
   - 有意浓底：配对了 `text-accent-foreground`（Radix 的 `DropdownMenuSubTrigger` 等）→ 保留；
   - 误用淡底：预期是 hover/focus/selected 的轻底，实际落到文字色 → 改 `bg-accent-subtle` / `--accent-subtle`。
   混在一起处理会错改有意浓底，分类后各批处理速度更快。

2. **零消费者也修**：如果一个组件当前没有业务消费者（如 Toggle），仍要修——不修则下次引入时带着白块回来，届时排查成本更高。注明「防未来复用」。

3. **已知验证缺口要明说**：Dialog Close 走 Radix `DialogContent`，但当前应用无真实触发路径（「添加平台」用内联表单）。这种情况不是「已验证」，在 commit 回报和收尾清单里显式列出「未覆盖路径 N 条」，不默默跳过。

## Vite-only 模式下 Tauri IPC 组件的取证（2026-09-30 立，2026-11-18 扩）

组件因 Tauri IPC 调用返回空不挂载、`getComputedStyle` 采不到时，按序两档：

**① mock 传输层真渲染（首选；2026-11-18 截图轮 20 页 × 深浅 40 图实锤跑通）**：
vite dev（1420）+ Playwright `channel: "chrome"` + `page.route("**/api/rpc/**")`
按命令名返回假数据（transport.ts 的 web 传输层走 `/api/rpc/`，拦这一层等于拦全部
invoke），`**/api/events`（SSE）一并拦住防挂起；未命中命令记入未命中日志并返回
null。组件真挂载，getComputedStyle / 深浅两 mode 截图全部可用。三个坑：

- 假数据形状必须逐字段对齐真实 IPC 返回——缺字段、该是对象的给了 null、该是数组
  的给了 [] 都会让页面空崩（本轮目检修掉 6 处形状缺陷）；形状取真值用
  「显示类 bug」节的只读 RPC 读用户实例，或读 Rust 返回 struct 的 serde 定义。
- 假数据命名一眼是演示（名称带「演示」、id 带 demo-），日期相对 now 让图表有形。
- 截图前杀掉残留 vite 进程再起：旧进程的模块缓存会渲染出裸 i18n key。

**② 静态推导（mock 成本过高时的末位降级，非一般许可）**：

1. 读源码找出该组件用的 className（`bg-accent`、`focus:bg-accent` 等）；
2. 按 CLAUDE.md 的 accent token 解析链（`mono.ts` → `tokens.generated.ts` → `globals.css`）
   静态推导出 dark/light 各模式的最终值；
3. 结论标明「源码推导，Vite-only 无 IPC，未跑真渲染」。

这不是「静态推理替代 computed style」的一般许可——mock 传输层成本是一次脚本的
成本，能搭就搭，仍以真渲染为准（规则 1/2）。

## 显示类 bug：先用只读 RPC 读用户实例的真实数据（2026-11-09 代理日志空行）

「日志页展示全是空的」类问题，几分钟定位靠的是直接读真实行：对用户常驻 AiDog 发只读管理接口
`POST 127.0.0.1:9890/api/rpc/proxy_log_list`、`proxy_log_get`（本机回环，见 CLAUDE.md「Local API」），
看到 115 行 http-connect 非 AI 隧道空行，根因当场定论，不必先猜渲染层。

- 只调 list / get 类只读命令；写命令、`pkill`、改设置一律不碰（perf-testing.md §1 的边界）。
- context-mode 钩子会拦 `curl`：写一个 node `fetch` 脚本文件（放 `.tmp/<任务名>/`）再跑，收工删。
- 有 `auth_token` 时带 Bearer；9890 不通就报「做了没验证」，不换端口试别的实例。
