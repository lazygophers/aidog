//! OpenAI 格式 → GLM 平台

use crate::gateway::adapter::types::*;
use serde_json::Value;

/// OpenAI 请求 → GLM 平台格式
pub fn to_glm(req: &ChatRequest) -> Result<Value, String> {
    // GLM 使用 OpenAI 兼容格式，基本可以直接转换
    // TODO: 处理 GLM 特定字段（如果有）
    serde_json::to_value(req)
        .map_err(|e| format!("OpenAI to GLM error: {}", e))
}
