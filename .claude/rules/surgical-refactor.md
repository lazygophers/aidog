# 大段文本产出：编辑执行模式、生成流劣化降维与切函数后的遗留体核对

2026-10-07 源码简化轮（commit 8cde235，八项重构；最大单项 forward_attempt 1103 行
切三阶段）沉淀。适用于一切大块结构性改码：巨函数切分、大段替换、跨区域搬移；
2026-10-08 tab-redesign 轮（commit 2054d78）补充：长 HTML / 长代码 / 长 heredoc
的一次成型生成同样适用（§3）。

## 1. 大块多行替换：锚单行、拼模板，不手打多行串

逐字多行替换串在大文件上反复失配：本 session Edit「String to replace not found」
3 次（ProxyLog 40 字段块 / resolve_ 函数 / use 头），随后 shell / python 内联拼装
脚本又因引号嵌套炸 3 次（unmatched backtick、python -c 多行串语法错）——每次失败
烧一轮完整生成。跑通的降维模式：

1. **锚点取单行唯一串**（函数签名行、`let x = ...;` 行），不锚多行块；
2. **大段新代码写 /tmp 模板文件再拼装**（awk / python 按锚点行号拼接）；拼装脚本
   本身落盘成文件再跑，不写 `python -c` 内联——inline 的引号嵌套必炸（本 session
   实证 3 次）；
3. 目标函数 ≤200 行时第三选：Write 整函数重写，一次成型。

判据：替换串超过 ~15 行或含嵌套引号/宏，直接走 2 或 3，不试 Edit 逐字匹配。

## 2. 切函数：搬块之后、编译之前，核对遗留体自由变量

把代码块搬进新 fn 后，留在原函数的尾段继续引用被搬走块的局部变量，编译才炸
（本 session `cannot find value` 八种符号——`target_protocol` / `attempts` /
`same_protocol_passthrough` / `disable_thinking` / `breaker_th` / `requested_model`
/ `needs_model_remap` / `attempt_ts`——几十次编译红，每红一轮等一次 `cargo check`）。
`cargo check` 是兜底门禁，但每轮红 = 一次构建等待；核对是秒级的：

1. **搬完一个块，当场核对遗留体**：遗留体引用的每个标识符，要么仍在原作用域，
   要么已进新 fn 的参数 / 返回值；
2. 核对动作具体化：列出被搬块定义的 `let` 名单，逐个 grep 遗留体行区间；
3. 先核对再 `cargo check`——不把编译器当唯一的 scope 检查器烧构建周期。

## 3. 长输出生成流劣化：见垃圾即降维，不在劣化输出上打补丁

一次成型生成长 HTML / 长代码块 / 长 heredoc，中后段会出占位 token、错误路径、
语法垃圾（2026-10-08 tab-redesign 轮当天 6+ 次，全部发生在长输出里；短 Write /
Edit 小步锚点替换 / 行切片脚本从未出）。这是生成流劣化，不是思路错——重试同
长度整段生成大概率再劣化。

1. **判据**：产出里出现占位 token、不存在的路径、语法垃圾，任一即判生成劣化，
   弃整段重来；
2. **降维顺序**：① 拆多次短 Write（每段独立成块，靠后续 Edit/拼接接上）→
   ② Edit 单行锚点小步替换（§1 模式）→ ③ python 行切片脚本拼装（脚本落盘再跑，
   不内联）；
3. **禁止**在已劣化的长输出上继续打补丁——垃圾密度随长度涨，修补成本超过重写；
4. 长 heredoc 是重灾区：拆成多次短 heredoc append，或落 /tmp 模板文件后 cp。
