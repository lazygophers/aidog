//! generate_image 的 provider 适配层（magpie 票 12，spec D5）。
//!
//! generate_image = OpenAI images API 兼容实现。端点与请求形状取自一手 spec：
//! openai/openai-openapi（github）master openapi.yaml v2.3.0，path `/images/generations`，
//! Bearer 鉴权，body `{model, prompt, n, size}`，响应 `data[]` 条目带 `url` 或 `b64_json`。
//! base_url 用户自配、含版本前缀（如 `https://api.openai.com/v1`），最终 URL =
//! base_url + `/images/generations`，禁额外拼接（项目 URL 构造铁律）。
//!
//! 出参 markdown 图片链接，按响应形态：条目带 `url` → `![generated image](url)`；
//! 带 `b64_json` → `data:image/png;base64,…` data URL 内嵌（免落盘，MCP text 可携带；
//! 代价是消息体变大，取舍记录在票 ## Answer）。不发 `response_format`（可选参数，
//! 两种响应形态都按数据判别）。

use aidog_search::{call, http_client};
use crate::mcp_tools::MediaProviderConfig;

/// `generate_image` 工具入口。
pub async fn generate_image(
    cfg: Option<&MediaProviderConfig>,
    prompt: &str,
    size: Option<&str>,
) -> Result<String, String> {
    let cfg = cfg.filter(|c| c.is_configured()).ok_or(
        "未配置图片 provider（在 aidog 设置里填写 base_url / API key / 模型名）",
    )?;
    let client = http_client()?;
    let mut body = serde_json::json!({
        "model": cfg.model.trim(),
        "prompt": prompt,
        "n": 1,
    });
    if let Some(s) = size.map(str::trim).filter(|s| !s.is_empty()) {
        body["size"] = serde_json::json!(s);
    }
    let url = format!(
        "{}/images/generations",
        cfg.base_url.trim().trim_end_matches('/')
    );
    let v = call(client.post(url).bearer_auth(cfg.api_key.trim()).json(&body)).await?;
    let items = v["data"]
        .as_array()
        .ok_or_else(|| format!("响应缺少 data 数组: {}", v.to_string_lossy_truncate()))?;
    let mut links = Vec::new();
    for item in items {
        if let Some(u) = item["url"].as_str().filter(|u| !u.is_empty()) {
            links.push(format!("![generated image]({u})"));
        } else if let Some(b64) = item["b64_json"].as_str().filter(|b| !b.is_empty()) {
            links.push(format!("![generated image](data:image/png;base64,{b64})"));
        }
    }
    if links.is_empty() {
        return Err("响应条目缺少 url / b64_json 字段，无法生成图片链接".into());
    }
    Ok(links.join("\n\n"))
}

/// `Value` 的错误信息截断预览（避免把整张 b64 图贴进错误）。
trait ValuePreview {
    fn to_string_lossy_truncate(&self) -> String;
}

impl ValuePreview for serde_json::Value {
    fn to_string_lossy_truncate(&self) -> String {
        aidog_search::truncate_str(&self.to_string(), 200).to_string()
    }
}

#[cfg(test)]
#[path = "test_media.rs"]
mod test_media;
