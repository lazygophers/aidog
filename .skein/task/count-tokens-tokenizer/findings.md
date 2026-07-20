# count_tokens 本地估算升级 per-model tokenizer — 调研收敛

深度调研的收敛结论 + 依据/引用 (过程笔记存 research/):

## s2 下载 HF tokenizer.json 落点 (2026-07-20)

bundled 到 `src-tauri/crates/aidog_core/assets/tokenizers/`, s3 `include_bytes!` 即可加载。

| 模型 | 文件 | 字节 | MB | 下载源 | `.model.type` |
|---|---|---|---|---|---|
| glm-4 | `glm-4.json` | 19967863 | 19.0 MB | https://huggingface.co/THUDM/glm-4-9b-chat-hf/resolve/main/tokenizer.json | BPE |
| qwen2 | `qwen2.json` | 7028015 | 6.7 MB | https://huggingface.co/Qwen/Qwen2-7B/resolve/main/tokenizer.json | BPE |

- 两文件 `python3 -m json.tool` 校验合法, jq 解析无报错。
- `model_type` 字段实为 `.model.type` (HF tokenizer.json schema 顶层无 `model_type`, 该 key 在 config.json)。
- 调研报告预估 glm-4 ~1.8MB, 实测 19MB — 原预估偏低。glm-4-9b-chat-hf vocab ~150K tokens (中文 token 化重), 单文件自包含 (vocab+merges+pretokenizer) 必然体积大。
- glm-4 备选源全部 404: `THUDM/glm-4-9b`, `THUDM/glm-4-9b-chat`, `THUDM/glm-4-9b-chat-1m`, `THUDM/GLM-4-9B-Chat`, `THUDM/chatglm-6b/chatglm2-6b/chatglm3-6b/chatglm3-6b-32k`, `zai-org/GLM-4-9B-Chat`。仅 `glm-4-9b-chat-hf` 可用 (非 -hf 后缀仓 repo 已 rename/不存在)。
- glm-4 19MB 超 10MB 软上限: 无降级路径 (无更小 GLM 4 tokenizer 可用), 维持 19MB, 需注意 binary 体积; 若最终 release 体感过大可考虑运行时按需懒加载或换 ChatGLM2/3 老版本 (但与 glm-4 实际 vocab 已分叉, token 估算会偏)。

决策: 保留 glm-4 (19MB) + qwen2 (6.7MB) 两文件 bundled, 总 ~26MB binary 体积增量。
