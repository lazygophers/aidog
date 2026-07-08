# Implement: bailian(+bailian_coding) model_list 全谱补全

## 执行层编排

**单文件单协议族改动**，无依赖无并行需求 → 轻量 inline（单 trellis-implement 在 task worktree 内直做），不拆 subtask。

## 改动面

唯一文件：`src-tauri/defaults/platform-presets.json`

| 协议 | 字段路径 | 改动 |
|------|----------|------|
| `bailian` | `protocols.bailian.model_list.default` | 数组从 6 项 → 全谱（~45 项，见 prd.md 清单） |
| `bailian_coding` | `protocols.bailian_coding.model_list.default` | 数组从 3 项 → 5 项（Coder 全系 + qwen3.7-max） |

**不动**：`endpoints` / `models.default.default` / `name` / `desc` / `source_urls` / `homepage` / `logo_url` / `version`。

## 执行步骤（trellis-implement）

1. 读 `src-tauri/defaults/platform-presets.json`，定位 `protocols.bailian.model_list.default` 与 `protocols.bailian_coding.model_list.default`
2. 按 `prd.md` Requirements 段清单，整组替换两数组（保持 JSON 缩进风格 = 2 空格，与文件现有风格一致）
3. 验证：
   - `python3 -m json.tool src-tauri/defaults/platform-presets.json > /dev/null`（JSON 合法）
   - `python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));b=d['protocols']['bailian']['model_list']['default'];c=d['protocols']['bailian_coding']['model_list']['default'];print('bailian',len(b),'dup',len(b)-len(set(b)));print('coding',len(c),'dup',len(c)-len(set(c)))"`（无重复）
4. `cd src-tauri && cargo test`（defaults 相关 test 通过）
5. `cd src-tauri && cargo clippy`（无新 warning）

## 验收标准

见 prd.md Acceptance Criteria。

## 失败处理

- cargo test 失败 → 读 test 报告，若 test 硬编码 model_list 长度则同步更新 test 断言，禁关闭 test
- JSON 格式错 → 重格式化，禁手改结构
- model id 拼写存疑 → 对照 research/bailian-models.md 计费页表格，禁臆造

## 上下文注入

implement.jsonl / check.jsonl 待 Phase 1.3 整理（注入 research/bailian-models.md + platform-presets.json 路径）。
