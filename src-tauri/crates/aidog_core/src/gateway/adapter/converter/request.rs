//! 入站请求解析与出站请求转换：内部 ChatRequest ↔ 各 wire 协议 body + API 路径。

use crate::gateway::models::Protocol;
use serde_json::Value;

use super::super::types::*;

/// 将内部 ChatRequest 转为目标格式的 JSON body + API 路径。
///
/// - `wire_protocol`: 请求体格式（由 endpoint 协议决定：anthropic/openai/openai_responses/openai_completions/gemini）
/// - `platform_protocol`: 平台类型（由平台主协议决定，决定 OpenAI-compatible 平台的 API 路径）
pub fn convert_request(req: &ChatRequest, wire_protocol: &Protocol, platform_protocol: &Protocol) -> (Value, String) {
    match wire_protocol {
        Protocol::Anthropic => {
            let anthropic_req = super::super::anthropic::to_anthropic(req);
            let json = serde_json::to_value(&anthropic_req).unwrap();
            (json, "/v1/messages".to_string())
        }
        Protocol::Gemini => {
            let gemini_req = super::super::gemini::to_gemini(req);
            let json = serde_json::to_value(&gemini_req).unwrap();
            let path = format!("/v1beta/models/{}:streamGenerateContent", req.model);
            (json, path)
        }
        Protocol::OpenAIResponses => {
            let responses_req = super::super::openai_responses::to_responses(req);
            let json = serde_json::to_value(&responses_req).unwrap();
            (json, "/v1/responses".to_string())
        }
        Protocol::OpenAICompletions => {
            let completions_req = super::super::openai_completions::to_completions(req);
            let json = serde_json::to_value(&completions_req).unwrap();
            (json, "/v1/completions".to_string())
        }
        // CPA(CLIProxyAPI)导入平台类型：platform_type 作为 wire 回退（无 endpoint 匹配时），
        // 各自映射到同族 adapter（design.md: grok→openai_responses / 其余→gemini）。
        // 正常路径下 endpoint[].protocol 已显式声明同族 wire，不会落到这里。
        Protocol::CpaGrok => {
            // xAI Grok 原生 `/responses`（同 OpenAI Responses 语义）。
            let responses_req = super::super::openai_responses::to_responses(req);
            let json = serde_json::to_value(&responses_req).unwrap();
            (json, "/v1/responses".to_string())
        }
        Protocol::CpaAistudio | Protocol::CpaAntigravity | Protocol::CpaVertex => {
            // cpa-aistudio: 与 gemini-api-key 同 API（generativelanguage.googleapis.com），仅 auth 不同。
            // cpa-antigravity / cpa-vertex: 仅存配置，路由暂不支持——
            //   antigravity 实际路径 `/v1internal:streamGenerateContent` / vertex 含
            //   `projects/{p}/locations/{l}/publishers/google/models/...` 结构，gemini adapter
            //   path 不兼容。这里给 gemini 占位（preset endpoint 显式 protocol=gemini 才真正生效）。
            let gemini_req = super::super::gemini::to_gemini(req);
            let json = serde_json::to_value(&gemini_req).unwrap();
            let path = format!("/v1beta/models/{}:streamGenerateContent", req.model);
            (json, path)
        }
        // OpenAI Chat Completions — 标准 /v1/chat/completions，OpenAI-compatible 平台用各自路径
        _ => {
            let openai_req = super::super::openai::to_openai(req);
            let json = serde_json::to_value(&openai_req).unwrap();
            let path = provider_api_path(platform_protocol);
            (json, path)
        }
    }
}

/// OpenAI Chat Completions 端点路径（统一后缀，base_url 负责版本前缀）
fn provider_api_path(_protocol: &Protocol) -> String {
    "/chat/completions".to_string()
}

/// 同协议透传时的出站 API 路径：与 `convert_request` 对各 wire 协议产出的 path 保持一致，
/// 但**不转换 body**（透传保留原始请求体结构）。
///
/// - `wire_protocol`: 出站 wire 协议（= 入站协议，因为透传仅在精确同协议时触发）
/// - `model`: 用于 Gemini path 中的模型段（其余协议忽略）
/// - `platform_protocol`: 平台类型，决定 OpenAI-compatible 平台的 chat path 后缀
pub fn passthrough_api_path(wire_protocol: &Protocol, model: &str, platform_protocol: &Protocol) -> String {
    match wire_protocol {
        Protocol::Anthropic => "/v1/messages".to_string(),
        Protocol::Gemini => format!("/v1beta/models/{}:streamGenerateContent", model),
        Protocol::OpenAIResponses => "/v1/responses".to_string(),
        Protocol::OpenAICompletions => "/v1/completions".to_string(),
        // CPA 平台类型 wire 回退（见 convert_request 对应 arm 注释）。
        Protocol::CpaGrok => "/v1/responses".to_string(),
        Protocol::CpaAistudio | Protocol::CpaAntigravity | Protocol::CpaVertex => {
            format!("/v1beta/models/{}:streamGenerateContent", model)
        }
        _ => provider_api_path(platform_protocol),
    }
}

/// 将入站请求按源协议解析为内部 ChatRequest（支持所有 AI 请求协议）。
///
/// 返回 `Err(String)` 携带解析失败原因(serde 错误细节等)，供上层记录到日志便于诊断。
pub fn parse_incoming_request(source_protocol: &str, body: &Value) -> Result<ChatRequest, String> {
    match source_protocol {
        "openai" => super::super::openai::from_openai(body).ok_or_else(|| "openai from_openai returned None".to_string()),
        "openai_responses" => super::super::openai_responses::from_responses(body).ok_or_else(|| "openai_responses from_responses returned None".to_string()),
        "openai_completions" => super::super::openai_completions::from_completions(body).ok_or_else(|| "openai_completions from_completions returned None".to_string()),
        "gemini" => super::super::gemini::from_gemini(body).ok_or_else(|| "gemini from_gemini returned None".to_string()),
        // Anthropic / 默认: ChatRequest 结构已兼容 Anthropic 格式，直接反序列化;
        // ContentBlock 已对未知类型(thinking/image/…)降级 Unknown, 失败时返回 serde 错误细节供诊断。
        _ => serde_json::from_value(body.clone()).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
#[path = "test_request.rs"]
mod test_request;
