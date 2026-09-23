# Flutter↔Tauri UI 持续对齐

Loop：用户实机发现 Flutter 与 React(Tauri) 界面不一致 → 报差异 → 一轮「定位→修复→验证→关票」。直到逐元素清单零未修项。

## 裁决（2026-09-23 ask-ui `flutter-tauri-ui-20260923155434-fd52`）

| 事项 | 裁决 |
|---|---|
| 验收判据 | **元素级 + 行为级**：每个可见元素两侧同名同位置、每个交互同行为；间距字号跟各自体系（token 同源）。不做像素级截图比对 |
| 参照物 | **React 现状 = 唯一真值**。React 有的 Flutter 必须有；React 没有的 Flutter 不能有。两侧都有的元素不是差异 |
| 已裁决偏离 | **全部抹平**（2026-09-23 推翻此前保留决定），见「偏离清零」 |
| 触发 | **事件触发**：用户报「XX 没对齐」即跑一轮。不巡检、不定时 |
| 验收 | **机器验收**：flutter analyze 0 issues + 相关 widget 测试全绿 + check-ui-parity 缺口 0 + 逐元素表更新，即关票。不等用户实机确认 |

## 触发

用户消息包含「没对齐 / 还是存在 / 还是有 / 差异」等对 UI 一致的否定描述。

## 每轮流程

1. **定位（禁空想）**：用户说的元素在两侧源码里各找到 `file:line`。React 侧先确认该元素真的存在/不存在——两侧都有的不是差异，回用户一句出处即关。
2. **修**：按 React 真值改 Flutter。React 侧只在 React 本身有缺陷时才动（须在回报里写明依据）。
3. **验证（真跑）**：`(cd flutter && flutter analyze)` 0 issues；改动到的 test 文件全绿；`node scripts/check-ui-parity.mjs --report --keys` 缺口 0。
4. **记账**：对应清单（`.scratch/flutter-ui-parity/inventory/*-v2.md`）状态列写 `已修 <commit>`；决策记 `.scratch/memory/YYYY-MM-DD.md`。
5. **提交**：`git commit -- <显式路径>`，绝不 `-a`（多会话共用工作区）。

## 防误判铁律（`.claude/rules/ui-parity-audit.md` 全文有效）

- 按能力搜不按 API 名搜；树位置 ≠ 渲染位置。
- 判定三档：缺口 / 共同空白 / 有意偏离。共同空白要改得连 React 一起改。
- 判「缺失」前必须读两侧现行源码，不许拿旧清单当现状。

## 偏离清零（待办，2026-09-23 拍板全部抹平）

1. 组内平台排序：上下移按钮+移组下拉 → 真拖拽（React `usePlatformDrag`）。方案：分组列表 `SliverReorderableList`（AppShell 已有 SliverPage 插槽路径，见 `.scratch/memory/2026-09-23.md`），组内嵌套手势冲突用 sliver 化解决。做完后「组内拖放指示线」（第三梯队）一并落地。
2. ~~导入导出「应用导入」前的二次确认卡~~ ✅ 已修 `7b5f012a`（2026-09-24）：删确认卡直接执行，按钮文案对齐 `applying`/`applyN`。
3. ~~拖入文件阶段不判扩展名~~ ✅ 已核框架不可达（2026-09-24）：desktop_drop 0.8.4 进入事件无文件列表（`drop_target.dart:50-58`），保持落下判，源码注释已标「框架不可达偏离」。终态。
4. 关窗拦截：两侧已是同行为（都隐藏不弹），无事可做，从偏离清单删除。

## 边界

- 不做视觉重设计、不做像素级 golden test（裁决明确排除）。
- React 侧正被其他会话改时：只读 React HEAD，不基于未提交改动对齐。
- 绝不 kill/restart `/Applications/AiDog.app`（实时流量）。
