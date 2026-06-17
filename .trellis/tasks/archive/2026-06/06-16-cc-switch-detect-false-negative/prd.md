# PRD: cc-switch 探测误报未检测到

## 现象
设置页「从 cc-switch 导入」点「检测 cc-switch」→ 报 `cc-switch 配置未检测到（探测路径：/Users/luoxin/.cc-switch/cc-switch.db）`，但该文件**实际存在**（59MB，可读写）。

## 根因
`read()` (ccswitch.rs:192) 把 `detect()` 返回的 **db 文件路径** `.../cc-switch.db` 当 `override_path` 传回 `detect()`。`detect()` (ccswitch.rs:125-128) 把文件路径当**目录**处理：

```
dir = .../cc-switch.db            ← 文件被当目录
db_path = dir.join("cc-switch.db") = .../cc-switch.db/cc-switch.db   ← 错配
db_path.exists() = false → found=false, path = .../cc-switch.db
```

完整链路：
1. `handleDetect()` → `detect(None)` → found=true，**path=`.../cc-switch.db`**（文件路径）
2. found=true → `handleRead(d.path=.../cc-switch.db)`
3. `read(path=.../cc-switch.db)` → 内部 `detect(path)` 把文件路径当目录 → found=false
4. read 抛「未检测到（探测路径：.../cc-switch.db）」← 用户看到的

实证：python `exists('/Users/luoxin/.cc-switch/cc-switch.db')=True`；settings.json 无 configDir；ccswitch.rs 仅 1 commit（代码即当前）。

## 方案 B（推荐）：read() 不重跑 detect
`read()` 的 path 语义 = 文件路径（前端只传 detect 返回的文件路径）。改为：
- path 是文件（存在）→ 按扩展名判 source_type 直读（`config.json`→json，否则 sqlite）
- path 是目录 / 缺省 → `detect()` 探测后读
- 去掉 read() 无条件 `detect(path)` 重跑（仅缺省时探测）

根除「文件路径当目录」错配，read 少跑一次 detect。

## 备选 A（最小，治标）
`detect()` override_path 若指向 `cc-switch.db`/`config.json` 文件，取父目录。1 处改动，但 read 仍多跑一次 detect。

## 验收
1. dev (`yarn tauri dev`) → 设置页 cc-switch 导入 →「检测」→ found=true → provider 列表显示（不再「未检测到」）
2. `cargo test`（ccswitch 现有 8 测试过）
3. 新增 read() 文件路径直读单测（mock db 文件路径传入 read，断言不重跑 detect 错配）
4. `cargo clippy` 无 warning

## 文件 / 范围
- `src-tauri/src/gateway/import_export/ccswitch.rs`（read() + 可选 detect override 文件取父目录）
- 单测补 read 文件路径场景
- 不动前端、不动 import/apply、不动 import_export 其他子模块

## subtask
单一交付（ccswitch.rs read/detect 路径处理 + 测试），不拆 child。
