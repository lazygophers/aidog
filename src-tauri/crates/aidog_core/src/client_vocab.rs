//! 客户端词表：两级（大类 → 变体）单一真值源，入站 UA 识别与前端只读词表共用。
//!
//! - 代码（`code`）落库（`proxy_log.client` / `stats_agg_hourly.client`），发布后禁改名。
//! - 识别只看 User-Agent（小写后比较）；规则表有序、首个命中即返回：变体先于大类，AI 客户端先于
//!   SDK，SDK 先于通用 HTTP 库。顺序即本表顺序，由 `client_detect` 的表驱动测试固定。
//! - 旧入站码（claude_code / codex / pi / cursor / windsurf / gemini_cli）保持为大类码，历史行零迁移。
//! - 每条 `src` 必填：一手来源写 `owner/repo:path` 或包名；取不到一手的以 `推测:` 开头
//!   （取证报告 `.scratch/平台-客户端白名单/research/01-ua-vocab.md`）。出站模拟表本模块不管（票 07）。
//! - 显示名在 8 locale 文件的 `flow.client.<code>`（`name_key`），不在此处。
//! - 汇总统计表 client 维度记到变体码：每 (小时, model, group, platform, req_model) 下 client 值
//!   上限 = 本表可识别码数（约 55）+ `other` + 历史 `''`，行数按实际出现的 client 增长而非全集乘积。

use serde::Serialize;

#[derive(Clone, Copy)]
enum M {
    /// UA 包含子串。
    C(&'static str),
    /// UA 以前缀开头。
    P(&'static str),
    /// UA 整串相等。
    E(&'static str),
    /// 两个子串同时包含。
    And(&'static str, &'static str),
}

impl M {
    fn hit(self, ua: &str) -> bool {
        match self {
            M::C(s) => ua.contains(s),
            M::P(s) => ua.starts_with(s),
            M::E(s) => ua == s,
            M::And(a, b) => ua.contains(a) && ua.contains(b),
        }
    }
}

pub struct ClientEntry {
    pub code: &'static str,
    /// 所属大类码；`None` = 本条即大类。
    pub parent: Option<&'static str>,
    rules: &'static [M],
    pub src: &'static str,
}

impl ClientEntry {
    /// 所属大类码（大类返回自身）。
    pub fn category(&self) -> &'static str {
        self.parent.unwrap_or(self.code)
    }
}

const fn cat(code: &'static str, rules: &'static [M], src: &'static str) -> ClientEntry {
    ClientEntry { code, parent: None, rules, src }
}
const fn var(code: &'static str, parent: &'static str, rules: &'static [M], src: &'static str) -> ClientEntry {
    ClientEntry { code, parent: Some(parent), rules, src }
}

const CC_BIN: &str = "一手-二进制 @anthropic-ai/claude-code-darwin-arm64@2.1.314 内联 UA 模板 claude-cli/${VERSION} (external, ${entrypoint}…)";

/// 有序表：先出现者先匹配；无规则的大类（`rules` 为空）只作分组。
pub static CLIENTS: &[ClientEntry] = &[
    // ── Claude Code：变体（entrypoint 精确区分）先于大类 ──
    var("claude_code_vscode", "claude_code", &[M::And("claude-cli/", ", claude-vscode")], CC_BIN),
    var("claude_code_sdk_ts", "claude_code", &[M::And("claude-cli/", ", sdk-ts")], "一手-源码 @anthropic-ai/claude-agent-sdk@0.3.314:core.mjs CLAUDE_CODE_ENTRYPOINT=sdk-ts"),
    var("claude_code_sdk_py", "claude_code", &[M::And("claude-cli/", ", sdk-py")], "一手-源码 anthropics/claude-agent-sdk-python:src/claude_agent_sdk/_internal/transport/subprocess_cli.py@1ea8cb74dd"),
    var("claude_code_sdk_cli", "claude_code", &[M::And("claude-cli/", ", sdk-cli")], CC_BIN),
    var("claude_code_gh_action", "claude_code", &[M::And("claude-cli/", ", claude-code-github-action")], CC_BIN),
    var("claude_code_desktop", "claude_code", &[M::And("claude-cli/", ", claude-desktop")], CC_BIN),
    var("claude_code_mcp", "claude_code", &[M::And("claude-cli/", ", mcp")], CC_BIN),
    var("claude_code_local_agent", "claude_code", &[M::And("claude-cli/", ", local-agent")], CC_BIN),
    cat("claude_code", &[M::C("claude-cli")], CC_BIN),
    // ── Codex：originator 前缀变体；大类 `codex` 兜底在其它 AI 前缀之后（见下） ──
    var("codex_cli", "codex", &[M::P("codex_cli_rs/")], "一手-源码 openai/codex:codex-rs/login/src/auth/default_client.rs:42,152-166@f2ac998"),
    var("codex_tui", "codex", &[M::P("codex-tui/")], "一手-源码 openai/codex:codex-rs/tui/src/app_server_connection.rs:36"),
    var("codex_vscode", "codex", &[M::P("codex_vscode/")], "一手-源码 openai/codex:default_client.rs:141-146; default_client_tests.rs:114"),
    var("codex_desktop", "codex", &[M::P("codex desktop/")], "一手-源码 openai/codex:default_client.rs:141-146; app-server/README.md:230"),
    var("codex_exec", "codex", &[M::P("codex_exec/")], "一手-源码 openai/codex:codex-rs/exec/src/lib.rs:272"),
    // ── pi：short 模式 UA 恰为 `pi`；detailed 含 `node/v…`，故先于一切 HTTP 库 ──
    cat("pi", &[], "占位分组"),
    var("pi_cli", "pi", &[M::P("pi/"), M::P("pi "), M::E("pi")], "一手-源码 earendil-works/pi:packages/ai/src/utils/identity.ts@0441483; aidog gateway/pi.rs:179-196"),
    // ── 其余 AI 客户端（各自独立大类） ──
    cat("kilo_code", &[M::P("kilo/"), M::P("opencode-kilo-provider")], "一手-源码 Kilo-Org/kilocode:packages/opencode/src/installation/index.ts:51-55@0a9faef（旧 VS Code 扩展 UA 未取到：推测）"),
    cat("opencode", &[M::P("opencode/")], "一手-源码 anomalyco/opencode v1.19.5 packages/opencode/src/session/llm/request.ts:17,195,200"),
    cat("qwen_code", &[M::P("qwencode/")], "一手-源码 QwenLM/qwen-code:packages/core/src/core/openaiContentGenerator/provider/default.ts:105-108@38337f7"),
    cat("roo_code", &[M::P("roocode/"), M::P("roo-code/")], "一手-源码 RooCodeInc/Roo-Code:src/api/providers/constants.ts:3-7@b867ec9"),
    cat("cline", &[M::P("cline/")], "一手-源码 cline/cline:sdk/packages/llms/src/providers/request-headers.ts:93,143,155@a47448718a"),
    cat("factory_droid", &[M::P("factory-cli/")], "一手-二进制 downloads.factory.ai/factory-cli/releases/1.0.0 droid 内联 JS（仅托管路径；BYOK 路径推测）"),
    cat("amp", &[M::P("amp-cli/")], "推测: @ampcode/cli-darwin-arm64 二进制 strings 含 Amp-CLI/ 字面量，未证实作 UA 发送"),
    cat("crush", &[M::P("charm-crush/")], "一手-源码 charmbracelet/crush:internal/agent/agent.go:63,812,1448@16b7ee1"),
    cat("continue", &[M::P("continue/")], "一手-源码 continuedev/continue:core/llm/llms/ClawRouter.ts:47@5522c6f（仅 ClawRouter 路径有效）"),
    cat("zed", &[M::P("zed/")], "一手-源码 zed-industries/zed:crates/zed/src/main.rs:502-512@422b53cf"),
    cat("copilot_cli", &[M::P("copilot/")], "推测: 二手 cheaper-inference/OmniRoute:open-sse/config/providerHeaderProfiles.ts:133-137@689b1eb573"),
    cat("gemini_cli", &[M::P("geminicli"), M::C("proxy_client=geminicli")], "一手-源码 google-gemini/gemini-cli:packages/core/src/core/contentGenerator.ts:262-272@b633c07"),
    cat("cursor", &[M::C("cursor")], "推测: 产品名子串；官方文档称自带 Key 请求经 Cursor 云端发出，一手 UA 未取到"),
    cat("windsurf", &[M::C("windsurf")], "推测: 产品名子串；一手 UA 未取到"),
    // ── Codex 大类兜底（originator 全含 codex）：排在 AI 前缀之后、SDK/HTTP 之前 ──
    cat("codex", &[M::C("codex")], "一手-源码 openai/codex:default_client.rs:141-146 originator 全部含 codex；兜住老版本 / 自定义 originator"),
    // ── 框架 / SDK / HTTP 工具：宽泛规则最后 ──
    cat("litellm", &[M::P("litellm/")], "一手-源码 BerriAI/litellm:litellm/llms/custom_httpx/http_handler.py:180-194@a44e44b9"),
    cat("langchain", &[M::P("langchain")], "一手-源码 langchain-ai/langchain:libs/partners/anthropic/langchain_anthropic/chat_models.py:106,1745@864754c5e2"),
    cat("anthropic_sdk", &[], "分组"),
    var("anthropic_sdk_py", "anthropic_sdk", &[M::C("anthropic/python "), M::P("anthropicbedrock/"), M::P("anthropicvertex/")], "一手-源码 anthropics/anthropic-sdk-python:src/anthropic/_base_client.py:822-823@202db72c87"),
    var("anthropic_sdk_ts", "anthropic_sdk", &[M::C("anthropic/js ")], "一手-源码 anthropics/anthropic-sdk-typescript:src/client.ts:1325-1326@cb5fba4198"),
    cat("openai_sdk", &[], "分组"),
    var("openai_sdk_py", "openai_sdk", &[M::C("openai/python ")], "一手-源码 openai/openai-python:src/openai/_base_client.py:832-833@05992039b9"),
    var("openai_sdk_node", "openai_sdk", &[M::C("openai/js ")], "一手-源码 openai/openai-node:src/client.ts:906-907@1c716c325a"),
    cat("python_http", &[], "分组"),
    var("python_requests", "python_http", &[M::P("python-requests/")], "一手-源码 psf/requests:src/requests/utils.py:942-948@3c7baaa05a"),
    var("python_httpx", "python_http", &[M::P("python-httpx/")], "一手-源码 encode/httpx:httpx/_client.py:119@b5addb64f0"),
    var("python_aiohttp", "python_http", &[M::C("aiohttp/")], "一手-源码 aio-libs/aiohttp:aiohttp/http.py:62-64@3df45d9844"),
    cat("node_http", &[], "分组"),
    // 仅整串相等：pi detailed UA 含 `node/v…`，Contains 会误命中。
    var("node_fetch", "node_http", &[M::E("node"), M::E("undici")], "一手-源码 nodejs/undici:lib/web/fetch/index.js:72-74,1538@1b7b18e59f"),
    var("node_axios", "node_http", &[M::P("axios/")], "一手-源码 axios/axios:lib/adapters/http.js:915@9d11464a75"),
    var("node_node_fetch", "node_http", &[M::P("node-fetch")], "一手-源码 node-fetch/node-fetch:src/request.js:277-278@8b3320d2a7"),
    cat("go_http", &[M::P("go-http-client/")], "一手-源码 golang/go:src/net/http/request.go:550@5be815fece"),
    cat("java_http", &[], "分组"),
    var("java_okhttp", "java_http", &[M::P("okhttp/")], "一手-源码 square/okhttp:okhttp/src/commonJvmAndroid/kotlin/okhttp3/internal/-UtilCommon.kt:370@16fb6aef3a"),
    var("java_httpclient", "java_http", &[M::P("java-http-client/")], "一手-源码 openjdk/jdk:src/java.net.http/share/classes/jdk/internal/net/http/HttpRequestImpl.java:80@9b2f38106b"),
    cat("cli_http", &[], "分组"),
    var("curl", "cli_http", &[M::P("curl/")], "一手-源码 curl/curl:src/config2setopts.c:975-976@a133b27d07"),
    var("wget", "cli_http", &[M::P("wget/")], "一手-源码 mirror/wget:src/http.c:1789@8775506f63"),
    var("httpie", "cli_http", &[M::P("httpie/")], "一手-源码 httpie/cli:httpie/client.py:38@5b604c37c6"),
];

/// 按 UA 识别：`Some(条目)` = 命中变体或大类；`None` = 认不出（无 UA / 非 UTF-8 由调用方传空串）。
pub fn identify(ua: &str) -> Option<&'static ClientEntry> {
    let ua = ua.to_lowercase();
    CLIENTS.iter().find(|e| e.rules.iter().any(|m| m.hit(&ua)))
}

/// 平台客户端白名单判定：空 = 全放行；`hit` 为 `None`（认不出）= 不命中；
/// 否则白名单含该码或其所属大类即命中（勾大类 = 全部变体）。
pub fn client_allowed(whitelist: &[String], hit: Option<&ClientEntry>) -> bool {
    if whitelist.is_empty() {
        return true;
    }
    hit.is_some_and(|e| whitelist.iter().any(|w| w == e.code || w == e.category()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabVariant {
    pub code: &'static str,
    pub name_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabCategory {
    pub code: &'static str,
    pub name_key: String,
    /// 大类自身可被识别（勾大类 = 含其全部变体与自身兜底）；false = 纯分组。
    pub matchable: bool,
    pub variants: Vec<VocabVariant>,
}

fn name_key(code: &str) -> String {
    format!("flow.client.{code}")
}

/// 只读词表（大类 → 变体），前端经 command 取。
pub fn vocab() -> Vec<VocabCategory> {
    CLIENTS
        .iter()
        .filter(|e| e.parent.is_none())
        .map(|c| VocabCategory {
            code: c.code,
            name_key: name_key(c.code),
            matchable: !c.rules.is_empty(),
            variants: CLIENTS
                .iter()
                .filter(|v| v.parent == Some(c.code))
                .map(|v| VocabVariant { code: v.code, name_key: name_key(v.code) })
                .collect(),
        })
        .collect()
}

crate::tauri_command! {
    /// 两级客户端词表（只读）。
    pub async fn get_client_vocab() -> Result<Vec<VocabCategory>, String> {
        Ok(vocab())
    }
}

#[cfg(test)]
#[path = "test_client_vocab.rs"]
mod test_client_vocab;
