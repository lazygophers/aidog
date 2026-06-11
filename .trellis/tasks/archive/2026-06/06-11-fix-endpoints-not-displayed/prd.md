# PRD: 修复 endpoints 前端不展示

## Bug
DB platform.endpoints 有值（platform 1/2），前端 Protocol Endpoints 不展示。

## 根因（运行时实证）
`parse_endpoints`（db.rs:29 `serde_json::from_str(json).unwrap_or_default()`）反序列化 endpoints 数组时，因单个元素含未知 `client_type:"anthropic"`（`ClientType` enum 无此变体，serde externally-tagged enum 遇未知变体报错）→ **整个数组**解析失败 → `unwrap_or_default()` 静默返空 `[]` → 所有 endpoints 在 Rust 侧丢失。后端其余环节（PLATFORM_COLUMNS 列序 / row_to_platform index / list_platforms / api 类型 / 前端渲染）全正常。

## 修复
- models.rs：新增 `deserialize_client_type_lenient`（未知 client_type 字符串回退 `ClientType::Default`，不再让整个数组失败）；`PlatformEndpoint.client_type` 加 `#[serde(default, deserialize_with="...")]`
- db.rs：回归测试 `endpoints_with_unknown_client_type_still_parse`（含 "anthropic" 的两元素数组，修复前 len=0，修复后 len=2）
- （`#[serde(other)]` 尝试失败：要求最后变体且不能与 rename 共存 → 改 field-level deserialize_with）

## 验证
- cargo build 0、cargo test 40 passed（含回归测试）、tsc 0

## Commit 状态
修复入 commit `540b912`（多窗口并行：别窗口 pricing 工作改同 models.rs/db.rs，commit 时卷入了本修复。fix 已生效，但未独立 commit）。

## 后续（数据层，可选）
DB platform 1/2 残留无效 `"client_type":"anthropic"`（现回退 Default 可正常显示）；如需语义准确可数据迁移改为 `claude_code`。
