# 前端中文字面量门禁：翻译范围、默认值不落盘、白名单与脚本坑

2026-11-22 ~ 11-23 门禁上线轮沉淀（eb212248f 等 8 提交，账本 `.scratch/memory/2026-11-23.md`「前端中文字面量门禁上线并清完存量」）。
门禁 = `yarn check:no-cjk-literal`（`scripts/check-no-cjk-literal.mjs`，已进 `make lint`）：扫 `src/` 下 ts/tsx 字面量里的汉字、假名、韩文与全角标点（U+3000–U+303F、U+FF01–U+FF5E），`t()` 第二参的兜底值同样拦。

## 翻译范围（用户拍板）

- 只翻译 APP 自身文案。用户输入的内容，以及代表用户输入的演示数据（如状态行预览平台名 `glm/GLM-main`），不翻译、不加 key。
- `t()` 兜底值一律英文，不写中文。
- 前端文案有后端镜像（通知模板、状态行前缀、托盘文字）时，前后端一起加多语言；只改前端会让预览与实际输出不一致。

## 默认值不落盘，存量不迁移

可随语言变化的默认值（状态行段前缀、通知类型模板）不写进设置或 DB：未改过时存空，显示时按当前语言取（2cbb83f63 改 `src/components/settings/editors/StatusLineSection/` 的 `useStatusLinePanel.ts` 与 `SegmentEditModal.tsx`；bfd773fb8 删掉 `aidog_core/src/hooks.rs` 启动时写入中文模板的 `seed_default_templates`）。已存的旧中文值不迁移、不清空：存量分不清是用户自写还是旧默认。新增此类默认值照此写，review 时查「默认值有没有被抄进存储」。

## 白名单（ALLOWLIST）

- 条目 `{file, match, reason}`，`match` 与命中字面量的 span 重叠即豁免（`allowedBy`，脚本 :49）。因此 `match` 写成完整字面量、连同引号（`'"简体（通用）"'`）；只写开头（`'"简体（'`）会放行同文件所有同前缀的新文案（审查实锤后改完整匹配）。
- 只收三类：语言自称名、须与 Rust 逐字一致的内部名或预设值（写明对应 Rust 位置）、非界面文案的匹配规则。
- 门禁不扫 locale JSON 的值；locale 里的全角标点（如 ja-JP 的全角括号）不在门禁内。

## 写 TS AST 脚本：借 docs 子包的 TypeScript 6

根包 `typescript` 是 7.x（Go 版，无 `createSourceFile` 等 JS API）。需要编译器 API 的脚本（门禁、codemod）照门禁脚本 :12-13 写 `createRequire(join(ROOT, "docs/package.json"))("typescript")`；改写源码按字面量 `start/end` 做文本替换，不 AST 重打印整文件。

## 扩大门禁字符集：先盘点命中数，超阈值停手问

给门禁加新字符类（本轮全角标点）前，先跑一次扫描数出存量命中，派发 prompt 预设阈值（本轮 40）：超出即停手回报清单，由用户拍板一起改还是另开票。本轮 53 处超阈值，表单拍板后再改，零返工。处置时半角替换会改变中文界面的标点；要保留全角就改成带插值的整句 key，回报里写明这项可见变化。
