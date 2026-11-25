# UI 配色 bug：静态推理两轮不自洽就改跑真渲染读 computed style

select.tsx → mono.ts → tokens.generated.ts → globals.css 的变量链 + cascade 静态推理可能连续产出互相矛盾、每版单看都「说得通」的结论；定论靠 vite dev + Playwright（系统 Chrome channel）起真 app 读 getComputedStyle。

## 规则

1. **配色 bug（「某状态看不清 / 颜色不对」）的静态推理产物是假设不是结论。** 同一问题推理到第二版与第一版矛盾，就停手改跑真渲染。「继续推 cascade」永远比「起 dev server 量一下」贵。
2. 取证跑法：`yarn dev` 起 vite，Playwright `chromium.launch({ channel: "chrome" })` 开页面，对目标元素逐状态触发（选中 / 键盘高亮分别触发）后读 `getComputedStyle(el).backgroundColor`（及 color）。**深浅两 mode 各读一遍**——两 mode 走的 token 分支不同，只验一个漏一半。本机 playwright 未装 chromium headless shell，默认 `chromium.launch()` 起不来；一切 Playwright 截图 / 渲染验证（含 huashu-design 的 design_lint、渲染类票截图验收）一律带 `channel: "chrome"`，跑不成就报「做了没验证」，不静默跳过。
3. 结论落在读出来的值上（「13% 透明近黑」），不落在推理链上（「按顺序应该被覆盖」）。见 CLAUDE.md 的 accent token 语义条目：本仓 `--accent` 是文字色，shadcn `bg-accent` idiom 在 dark 下系统性错位，静态推理最容易在这里翻车。
4. **agent 自报的视觉验证结论（无遮挡 / 无重叠 / 动画正常）采信前自己真渲染截图核对几何。** reduced-motion 动画同样靠真组件浏览器几何断言（相交断言先红）验证。几何类断言红 = 真先红，比「目测正常」强一级。
5. **壳层（WKWebView）行为复现用 `channel: "webkit"`**：Tauri macOS 壳是 WKWebView，Chrome 复现到 ≠ 壳层没问题；本机 webkit-2388 已手动装进 `~/Library/Caches/ms-playwright/`。缓存被清或 revision 漂移时的手动安装法：revision 从 `node_modules/playwright-core/browsers.json` 读，`https://registry.npmmirror.com/-/binary/playwright/builds/webkit/<rev>/webkit-mac-15-arm64.zip` 直连可下（官方 CDN 无代理卡死，npmmirror 直连通过），zip 解压进 `~/Library/Caches/ms-playwright/webkit-<rev>/` 后写入空文件 `INSTALLATION_COMPLETE`，headless 起浏览器读 UA 冒烟即算装成。

## 全库同类 bug 审计流程

发现一个 accent 误用后，同 session 内做全库清查：

1. **先分类再处理**：`grep -r 'bg-accent\|var(--accent)' src/` 找全部候选，按两类分开——有意浓底（配对 `text-accent-foreground`，如 Radix `DropdownMenuSubTrigger`）保留；误用淡底（预期 hover/focus/selected 轻底，实际落到文字色）改 `bg-accent-subtle` / `--accent-subtle`。混着处理会错改有意浓底。
2. **零消费者也修**：组件当前无业务消费者（如 Toggle）仍要修——不修则下次引入时带着白块回来，排查成本更高。注明「防未来复用」。
3. **已知验证缺口要明说**：Dialog Close 走 Radix `DialogContent`，但当前应用无真实触发路径（「添加平台」用内联表单）——不是「已验证」，在 commit 回报和收尾清单显式列「未覆盖路径 N 条」，不默默跳过。

## Vite-only 模式下 Tauri IPC 组件的取证

组件因 Tauri IPC 调用返回空不挂载、`getComputedStyle` 采不到时，按序两档：

**① mock 传输层真渲染（首选）**：vite dev（1420）+ Playwright `channel: "chrome"` + `page.route("**/api/rpc/**")` 按命令名返回假数据（transport.ts 的 web 传输层走 `/api/rpc/`，拦这一层等于拦全部 invoke），`**/api/events`（SSE）一并拦住防挂起；未命中命令记入未命中日志并返回 null。组件真挂载，getComputedStyle / 深浅两 mode 截图全部可用。三个坑：

- 假数据形状必须逐字段对齐真实 IPC 返回——缺字段、对象给了 null、数组给了 `[]` 都会让页面空崩；形状取真值用「显示类 bug」节的只读 RPC 读用户实例，或读 Rust 返回 struct 的 serde 定义。
- 假数据命名一眼是演示（名称带「演示」、id 带 demo-），日期相对 now 让图表有形。
- 截图前杀掉残留 vite 进程再起：旧进程的模块缓存会渲染出裸 i18n key。

**② 静态推导（mock 成本过高时的末位降级，非一般许可）**：① 读源码找出该组件用的 className（`bg-accent`、`focus:bg-accent` 等）；② 按 CLAUDE.md 的 accent token 解析链静态推导 dark/light 各模式最终值；③ 结论标明「源码推导，Vite-only 无 IPC，未跑真渲染」。mock 传输层成本是一次脚本的成本，能搭就搭，仍以真渲染为准（规则 1/2）。

## 显示类 bug：先用只读 RPC 读用户实例的真实数据

「日志页展示全是空的」类问题，几分钟定位靠的是直接读真实行：对用户常驻 AiDog 发只读管理接口 `POST 127.0.0.1:9890/api/rpc/proxy_log_list`、`proxy_log_get`（本机回环，见 CLAUDE.md「Local API」），看到真实行根因当场定论，不必先猜渲染层。

- 只调 list / get 类只读命令；写命令、`pkill`、改设置一律不碰（perf-testing.md §1 的边界）。
- context-mode 钩子会拦 `curl`：写一个 node `fetch` 脚本文件（放 `.tmp/<任务名>/`）再跑，收工删。
- 有 `auth_token` 时带 Bearer；9890 不通就报「做了没验证」，不换端口试别的实例。
