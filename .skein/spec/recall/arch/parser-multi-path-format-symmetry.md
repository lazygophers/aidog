---
title: parser 多路径格式识别必须对称
layer: recall
category: arch
keywords: [parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth]
source: cpa-parse-no-provider
authored-by: skein-memory
---

# parser 多路径格式识别必须对称

何时被读: 写 / 改 parser 有多个入口识别同一格式时(如「单文件导入」+「目录扫描」+「压缩包解压」)
谁读: 写导入解析器 / 多入口数据消费逻辑的开发者

## 规则

parser 有多个入口(parse_single_file / scan_dir / scan_auth_dir / import_archive)识别同一格式时, 若只在单入口识别 → **同格式两入口行为分裂**: 一入口正常, 另一入口跳过/报错。bug 修复常只改一处, 另一处仍断。

## Why

多入口是常见模式(用户单文件 vs 批量目录 vs 压缩包)。格式识别逻辑若内联在各入口, 易漏对称:
- 入口 A 加了新格式识别, 入口 B 忘加
- 入口 A 的格式判定改了, 入口 B 旧逻辑残留

aidog CPA parser 实证(cpa-parse-no-provider):
- `parse_single_file`(source 路径)只认 CPA config stub(6 个顶层 key), **不认** OAuth 凭据 JSON
- `scan_auth_dir`(auth_dir 路径)认 OAuth 凭据
- → 用户拖含 OAuth 凭据的 zip(source 路径)全跳过「无 CPA provider 段」, auth_dir 扫描正常

## How to apply

1. grep parser 所有入口(`parse_*` / `scan_*` / `import_*`), 列各入口支持的格式
2. 同一格式需在多入口识别 → **抽公共函数**, 各入口复用(禁内联两份):
   ```rust
   fn parse_oauth_json(content: &str) -> Option<Vec<CpaProvider>> { ... }
   // parse_single_file + scan_auth_dir 都调它
   ```
3. 测试: 同格式输入经各入口, 返回结构对称(或对称错误: 扫描入口静默跳过 / 单文件入口返 Err)

## Cross-ref

- `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser.rs:532-565` 抽取示例(cpa-parse-no-provider s1)
- 关联 [[dedup-empty-field-key]](本 task 同源: 多路径 OAuth 识别修好后, dedup 空字段 bug 暴露)
- 关联 [[cpa-oauth-credential-format]](识别的格式定义)
