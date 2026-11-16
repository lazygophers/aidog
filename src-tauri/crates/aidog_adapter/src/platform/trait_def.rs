//! [`PlatformAdapter`] trait 定义与默认实现（家族基座 = trait 默认实现 + 协议构件组合）。

use aidog_db::models::Protocol;
use serde_json::Value;

use crate::converter;
use crate::platform::error;
use crate::platform::hooks;
use crate::platform::usage;
use crate::types::{ChatRequest, ChatStreamEvent};

/// usage 抽取结果：`(input, output, cache_read, cache_write, cache_write_1h, 按次工具计数)`。
pub type UsageTuple = (i32, i32, i32, i32, i32, Vec<(String, i64)>);

/// 平台出站适配器接口：一个适配器覆盖该平台全部端点（wire 协议作入参传入）。
///
/// 五件核心接口（请求构造 / 非流式响应解析 / 流式事件解析 / 错误体处理 / usage 抽取）
/// 与差异钩子的默认实现 = 协议构件组合，即家族基座；成员覆盖差异
/// （示例 [`crate::platform::registry::KimiFamilyAdapter`]）。
pub trait PlatformAdapter: Send + Sync {
    /// ① 请求构造：内部归一请求 → 出站 JSON body + API path。
    /// `wire` = 端点协议（用户平台配置 `platform.endpoints[].protocol`），
    /// `platform` = 平台协议（`platform_type`）。
    fn build_request(
        &self,
        chat_req: &ChatRequest,
        wire: &Protocol,
        platform: &Protocol,
    ) -> (Value, String) {
        converter::convert_request(chat_req, wire, platform)
    }

    /// 同协议透传分支的出站 API 路径（不转换 body，仅按 wire 协议派生路径）。
    fn passthrough_api_path(
        &self,
        wire: &Protocol,
        model: &str,
        platform: &Protocol,
        is_stream: bool,
    ) -> String {
        converter::passthrough_api_path(wire, model, platform, is_stream)
    }
    /// ② 非流式响应解析：上游 JSON → 客户端协议 body；None = 同族透传。
    /// `wire` = 端点协议（上游响应格式），`client` = 客户端协议（入站 source）。
    fn convert_response(
        &self,
        body: &Value,
        wire: &Protocol,
        client: &Protocol,
        model: &str,
    ) -> Option<Value> {
        converter::convert_response(body, wire, client, model)
    }

    /// ③ 流式事件解析：上游单帧 `data:` JSON → 中立事件流（帧内带 usage 时多产出一个
    /// `ChatStreamEvent::Usage`，见 converter::parse_sse，issue 71）。
    fn parse_sse_frame(&self, data: &Value, wire: &Protocol) -> Vec<ChatStreamEvent> {
        converter::parse_sse(data, wire)
    }

    /// ④ 错误体处理：上游非 2xx 错误体分类（模型类 / 服务类 / 未知）。
    fn classify_error(&self, status: u16, body: &str) -> error::UpstreamErrorClass {
        error::classify_upstream_error(status, body)
    }

    /// ⑤ usage 抽取（非流式响应文本）。
    fn extract_usage(&self, body: &str) -> UsageTuple {
        usage::extract_usage(body)
    }

    // ══ 平台差异钩子（家族成员覆盖差异；默认 = 现状通用行为）══

    /// Coding Plan 端点注入（端点 `coding_plan` flag 命中时调用）。默认按平台协议分派
    /// （仅 Kimi 协议注入 prompt_cache_key），与历史 headers.rs 同名函数等价。
    fn inject_coding_plan(&self, body: &mut Value, platform: &Protocol) {
        hooks::inject_coding_plan_fields(body, platform);
    }

    /// 同协议透传分支：无签名 thinking 块清洗（仅 Anthropic wire 上游），
    /// 返回剥除块数（与 `converter::strip_unsigned_thinking_blocks` 等价）。
    fn strip_unsigned_thinking_blocks(&self, body: &mut Value) -> usize {
        crate::protocols::strip_unsigned_thinking_blocks(body)
    }

    /// max_tokens 出站裁剪（body 层，键名按 wire 协议）。返回 Some((原值, 裁后值)) = 发生了裁剪。
    fn cap_max_tokens(
        &self,
        body: &mut Value,
        model_max: Option<i64>,
        wire: &Protocol,
    ) -> Option<(u32, u32)> {
        hooks::cap_body_max_tokens(body, model_max, wire)
    }

    /// disable_thinking 出站写入（含强制思考剥除，语义见 hooks 实现注释）。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }

    /// 未建模顶层字段兜底透传：从客户端原体按目标 wire 协议允许集合补齐。
    /// 返回被挡字段名（只有名没有值），调用方拼进 `proxy_log.field_trace`。
    fn passthrough_unmodeled_fields(
        &self,
        body: &mut Value,
        src: &Value,
        wire: &Protocol,
        upstream_url: &str,
    ) -> Vec<String> {
        hooks::apply_field_passthrough(body, src, wire, upstream_url)
    }
}
