# registry 数据维护：编辑脚本预检与验证输出读取

registry 数据轮次（r98-r105）暴露的两类流程缺陷。适用于一切「文本级编辑 JSON + 跑
check-registry 验证」的维护轮。

## 编辑脚本先预检，不盲插

文本级插入（不做 JSON round-trip，保科学计数法字面）在这轮两次撞上既有脏形状断言失败：

- 目标键已存在且**重复**（aihubmix `claude-3-opus-20240229.json` 双 `"capabilities"` 键，
  后值覆盖前值，行为碰巧一致但属数据错误）；
- 同文件同键出现两处（双 `official`），插入锚点匹配到的不是预期位置。

2026-09-24 起就有同族教训（price 块尾无逗号两种插入形状「已在多轮重犯」，见
`workflows/registry-data-alignment.md` 第 8 轮）。规则：

1. **批量插入前先预检每份目标文件**：目标键已存在 → 跳过并列入报告，不盲插不覆盖；
   重复键、锚点不唯一 → 单独处理，禁止放宽断言硬闯。
2. **同一插入形状问题重犯两次以上，修法是把预检写进脚本**（给 check-registry 加重复
   JSON 键检测 / 写公共编辑工具），不是靠下一轮 agent 记性好。
3. 插入后 `git diff` 逐文件确认改动只落在目标键，防止锚点误匹配绞坏邻近字段
   （第 8 轮曾绞坏 14 文件后重建）。

## 验证输出先看格式再 grep

check-registry 的覆盖率行是中文模板 `必补字段 ${name} 覆盖率 ${pct}%`
（`scripts/check-registry.mjs:367`），且仅在低于门禁时作为 failure 打印。用想当然的
pattern（英文 `coverage` 之类）grep 输出零命中，被当成「输出里没有覆盖率信息」→
漏读了本轮验证结论。规则：

1. **grep 零命中只说明 pattern 不匹配，不说明信息不存在。** 下「输出没有 X」的结论前，
   先去脚本源码 grep 打印模板（`console.log`/`console.error` 的字符串）确认实际格式。
2. 验证命令输出不长（百行级）就整读一遍再 grep；长输出先 `wc -l` 分段，确认每段都过眼
   或每段都有对应 grep。

## index.json 单独变更时盖戳脚本静默零盖（2026-09-30）

bump-registry-last-updated.mjs 的循环里 index.json 被 `continue` 跳过
（scripts/bump-registry-last-updated.mjs:100-105），只在**其它数据文件至少
一个被盖**时才推高 index 的全局 last_updated。单独改 index.json（重排平台
清单、改 pricing_only）→ 脚本输出「0 个文件已盖戳」且 exit 0，看似成功实际
没盖——index 自身的顶层 last_updated 没变，远程同步按内容比较跳过。此时手动
把 index.json 顶层 last_updated 改成当前 Unix 秒（等价操作），或顺带改任一
其它数据文件再跑脚本。

判脚本干了什么以输出数字为准：「0 个文件已盖戳」= 本次没盖，不是成功。

## 清单类测试逐条 panic：先收集全量失败再动手（2026-09-30）

修 platform slug 白名单测试，每修一个 panic 就暴露下一个（cline → zdotai），
循环三轮才绿。包含/排除名单、键集对比这类清单断言失败时，先把门禁改成一次
输出全部不匹配项（或临时收集再断言），看全了再一次性修，不逐条试错。
