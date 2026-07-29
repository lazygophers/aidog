---
title: measure-footprint-pid-matching
layer: recall
category: optimization
keywords: [measure,footprint,pid,glob,data-corruption,baseline]
status: active
---

## measure.sh 同 label 跨 run 文件混淆

## 触发场景

性能量测脚本 `measure.sh` 按 label 重复运行（如多轮对比测试）时，旧 run 的 footprint 临时文件（命名 `footprint-<label>-<pid>-<proc>.txt`）残留在磁盘。新 run 汇总时 glob 把新旧文件混匹配，累加出荒谬值（实例：某档 graphics 算出 276.6MB，但该档 TOTAL 仅 308.7MB，超出物理约束）。

## 陷阱 & 正解

❌ **陷阱**：glob 匹配所有同 label 的 footprint 文件，不区分 run

```bash
# measure.sh line ~80
cat footprint-${label}-*-*.txt | awk '...'  # 同 label 多 run 的文件全被吃进
```

同 label 不同 pid 文件堆叠，导致数据倍增或跳跃。

✅ **正解**：从 `size-curve-raw.txt` 的自证行提取本 run 真实 pid，精确取文件

```bash
# 从 size-curve-raw.txt 第 1 行提取本 run pid（格式："--- pid: <pid> ..."）
local run_pid=$(head -1 size-curve-raw.txt | grep -oP 'pid: \K\d+')

# 精确匹配本 run 的 footprint 文件
cat footprint-${label}-${run_pid}-*.txt | awk '
  /^ *---/ { n++; next }
  n == 1 { ... }
'
```

这样仅当前 run 的 footprint 被读取，历史文件自动被过滤。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| `glob footprint-${label}-*-*.txt` 无 pid 精选 | `glob footprint-${label}-${run_pid}-*.txt` |
| 汇总前未清旧 run 文件 | 从 size-curve-raw.txt 自证行提 pid + 精确 glob |
| footprint 多段输出全读（如 GPU + Networking + WebContent 堆） | awk `/^ *---/{n++; next} n==1` 仅取第一段分类明细 |

## 案例

实测显示某指标（graphics 等）跳到物理上限 2 倍，对比 size-curve-raw.txt 确认该档 TOTAL 只有 308.7MB。问题源于 glob 把前 3 run 的 footprint 全吃进（3 个 pid × 若干 proc），数据叠加 3 倍。修复后（精确 pid 过滤）单 run 读数回归正常。

## 适用

- `measure.sh` 同 label 重复运行（对比 baseline 常见）
- 任何大块临时数据依赖文件名去重的场景
