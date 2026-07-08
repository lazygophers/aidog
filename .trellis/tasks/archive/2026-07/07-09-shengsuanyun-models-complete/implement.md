# Implement: shengsuanyun(盛算云) model_list+endpoints+models 全量补全

## 执行层编排

**单文件单协议改动**，无依赖无并行需求 → 轻量 inline（单 trellis-implement 在 task worktree 内直做），不拆 subtask。

## 改动面

唯一文件：`src-tauri/defaults/platform-presets.json`

| 字段路径 | 改动 |
|----------|------|
| `protocols.shengsuanyun.model_list.default` | 数组从 13 项 → 全量 172 项（见 prd.md + research line 39-247） |
| `protocols.shengsuanyun.endpoints.default` | 数组从 1 端点 → 3 端点（anthropic + openai + gemini） |
| `protocols.shengsuanyun.models.default.default` | 对象从 `{}` → 三档（default/coder/fast） |

**不动**：`name` / `desc` / `source_urls` / `homepage` / `logo_url` / `client_type` / `version`。

## 执行步骤（trellis-implement）

1. 读 `src-tauri/defaults/platform-presets.json`，定位 `protocols.shengsuanyun`
2. **model_list.default**：按 `research/shengsuanyun-models.md` line 39-247 全量清单（172 项，按 provider 分组顺序：Anthropic → OpenAI → Google → DeepSeek → Ali → Bigmodel → Moonshot → MiniMax → Bytedance → x-ai → Xiaomi → Tencent → Baidu → Intern → Streamlake → StepFun → Meta → Xai → Longcat）整组替换数组
   - 核实 preset 原 13 项中 `openai/gpt-5.3-codex` 是否在 research 172 项内：research line 276/297/389 标 API 未找到 → **移除**（preset 真值源 = 官方 API）
3. **endpoints.default**：替换为三端点（见 prd.md JSON 块）
4. **models.default.default**：替换为三档对象（见 prd.md JSON 块）
5. 保持 JSON 缩进风格 = 2 空格（与文件现有风格一致）
6. 验证：
   - `python3 -m json.tool src-tauri/defaults/platform-presets.json > /dev/null`（JSON 合法）
   - `python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));m=d['protocols']['shengsuanyun']['model_list']['default'];print('count',len(m),'dup',len(m)-len(set(m)))"`（172 项 + 无重复）
   - `cd src-tauri && cargo test`（defaults 相关 test 通过）
   - `cd src-tauri && cargo clippy`（无新 warning）

## 验收标准

见 prd.md Acceptance Criteria。

## 失败处理

- cargo test 失败 → 读 test 报告，若 test 硬编码 model_list 长度则同步更新 test 断言，禁关闭 test
- JSON 格式错 → 重格式化，禁手改结构
- model id 拼写存疑 → 对照 research/shengsuanyun-models.md line 39-247 官方 API 清单，禁臆造
- `openai/gpt-5.3-codex` 去留 → 以 research line 276/297/389 实证为准（API 未找到 = 移除）

## 上下文注入

implement.jsonl / check.jsonl 待 Phase 1.3 整理（注入 research/shengsuanyun-models.md + platform-presets.json 路径）。
