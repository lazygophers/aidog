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
3. 结论落在读出来的值上（「13% 透明近黑」），不落在推理链上（「按顺序应该被
   覆盖」）。见 CLAUDE.md 的 accent token 语义条目：本仓 `--accent` 是文字色，
   shadcn `bg-accent` idiom 在 dark 下系统性错位，静态推理最容易在这里翻车。

## 全库同类 bug 审计流程（2026-09-30 accent 全量修复 3411cbc1a）

发现一个 accent 误用后，同 session 内做全库清查：

1. **先分类再处理**：`grep -r 'bg-accent\|var(--accent)' src/` 找全部候选，然后按两类分开：
   - 有意浓底：配对了 `text-accent-foreground`（Radix 的 `DropdownMenuSubTrigger` 等）→ 保留；
   - 误用淡底：预期是 hover/focus/selected 的轻底，实际落到文字色 → 改 `bg-accent-subtle` / `--accent-subtle`。
   混在一起处理会错改有意浓底，分类后各批处理速度更快。

2. **零消费者也修**：如果一个组件当前没有业务消费者（如 Toggle），仍要修——不修则下次引入时带着白块回来，届时排查成本更高。注明「防未来复用」。

3. **已知验证缺口要明说**：Dialog Close 走 Radix `DialogContent`，但当前应用无真实触发路径（「添加平台」用内联表单）。这种情况不是「已验证」，在 commit 回报和收尾清单里显式列出「未覆盖路径 N 条」，不默默跳过。

## Vite-only 模式下 Tauri IPC 组件的取证回退（2026-09-30）

SearchableProtocolSelect 在 Vite-only 下因 Tauri IPC 调用返回空，组件不挂载，
`getComputedStyle` 无法采集。此时唯一可行方法：

1. 读源码找出该组件用的 className（`bg-accent`、`focus:bg-accent` 等）；
2. 按 CLAUDE.md 的 accent token 解析链（`mono.ts` → `tokens.generated.ts` → `globals.css`）
   静态推导出 dark/light 各模式的最终值；
3. 结论标明「源码推导，Vite-only 无 IPC，未跑真渲染」。

这是此场景下的合法降级，不是「静态推理替代 computed style」的一般许可——
只要组件能挂载，仍优先跑真渲染（规则 1/2）。
