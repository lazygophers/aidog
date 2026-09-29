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
