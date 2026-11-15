//! 平台 code → 适配器注册表（issue 68）。
//!
//! 显式登记：门禁测试 `every_registry_platform_has_adapter_registration` 保证
//! registry 全量平台（含无内置默认端点的 devin / newapi / passthrough）都有注册项，
//! 收集全量缺失后一次报错。运行时 `adapter_for` 对未登记 code 回落通用适配
//! （用户平台 `platform_type` 可取 registry 清单外的协议变体）。

use aidog_db::models::Protocol;
use serde_json::Value;

use super::hooks;
use super::PlatformAdapter;

/// 家族基座（通用适配）：全部接口与钩子 = trait 默认实现，
/// 与历史「按 wire 协议分派 + 通用钩子」链路逐字节等价。
struct GenericAdapter;

impl PlatformAdapter for GenericAdapter {}

/// 家族示例：KimiAdapter 只登记在 `kimi` 上（platform_type = Kimi）。
/// 基座默认的 coding_plan 注入按平台协议分派（仅 Kimi 协议注入 prompt_cache_key）；
/// 家族成员的无条件注入在 platform=kimi 时与基座 Kimi 臂产出一致——演示「成员只覆盖差异」。
/// `kimi_en` / `kimi_coding` 是独立协议（KimiEn / KimiCoding），走默认基座，不注入
/// （迁移前 headers.rs 同为 no-op，review 必改 1：行为零变化）。
pub struct KimiAdapter;

impl PlatformAdapter for KimiAdapter {
    fn inject_coding_plan(&self, body: &mut Value, _platform: &Protocol) {
        hooks::inject_kimi_prompt_cache_key(body);
    }
}

static GENERIC: GenericAdapter = GenericAdapter;
static KIMI: KimiAdapter = KimiAdapter;

/// 显式注册表：registry 平台 code → 适配器单例（与 src-tauri/defaults/registry/platforms/ 一一对应；
/// 初始映射全部为默认家族基座，Kimi 家族覆盖 coding_plan 注入差异，行为与基座协议臂等价）。
static REGISTRY: &[(&str, &dyn PlatformAdapter)] = &[
    ("aihubmix", &GENERIC),
    ("anthropic", &GENERIC),
    ("atlascloud", &GENERIC),
    ("bailian", &GENERIC),
    ("bailian_coding", &GENERIC),
    ("bailian_coding_en", &GENERIC),
    ("bailian_en", &GENERIC),
    ("byteplus", &GENERIC),
    ("claude_code", &GENERIC),
    ("cline", &GENERIC),
    ("cline_pass", &GENERIC),
    ("codex", &GENERIC),
    ("commandcode", &GENERIC),
    ("compshare", &GENERIC),
    ("compshare_coding", &GENERIC),
    ("crazyrouter", &GENERIC),
    ("deepseek", &GENERIC),
    ("devin", &GENERIC),
    ("doubao", &GENERIC),
    ("gemini", &GENERIC),
    ("glm", &GENERIC),
    ("glm_coding", &GENERIC),
    ("glm_coding_en", &GENERIC),
    ("glm_en", &GENERIC),
    ("intern_inkstone", &GENERIC),
    ("kimi", &KIMI),
    ("kimi_coding", &GENERIC),
    ("kimi_en", &GENERIC),
    ("litellm", &GENERIC),
    ("longcat", &GENERIC),
    ("magpie", &GENERIC),
    ("meta", &GENERIC),
    ("minimax", &GENERIC),
    ("minimax_coding", &GENERIC),
    ("minimax_en", &GENERIC),
    ("mistral", &GENERIC),
    ("modelscope", &GENERIC),
    ("newapi", &GENERIC),
    ("novita", &GENERIC),
    ("nvidia", &GENERIC),
    ("openai", &GENERIC),
    ("opencode", &GENERIC),
    ("opencode_zen", &GENERIC),
    ("openrouter", &GENERIC),
    ("passthrough", &GENERIC),
    ("pipellm", &GENERIC),
    ("qianfan", &GENERIC),
    ("qianfan_coding", &GENERIC),
    ("sensenova", &GENERIC),
    ("sensenova_en", &GENERIC),
    ("shengsuanyun", &GENERIC),
    ("siliconflow", &GENERIC),
    ("siliconflow_en", &GENERIC),
    ("stepfun", &GENERIC),
    ("stepfun_en", &GENERIC),
    ("tencent_coding", &GENERIC),
    ("tencent_tokenhub", &GENERIC),
    ("tencent_tokenhub_intl", &GENERIC),
    ("therouter", &GENERIC),
    ("tokenrhythm", &GENERIC),
    ("typesafe", &GENERIC),
    ("xai", &GENERIC),
    ("xiaomi_mimo", &GENERIC),
    ("xiaomi_mimo_coding", &GENERIC),
    ("xiaomi_mimo_coding_en", &GENERIC),
];

fn table() -> &'static std::collections::HashMap<&'static str, &'static dyn PlatformAdapter> {
    static T: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static dyn PlatformAdapter>> =
        std::sync::OnceLock::new();
    T.get_or_init(|| REGISTRY.iter().copied().collect())
}

/// 门禁用：registry 平台 code 是否有显式注册项。
pub fn lookup(code: &str) -> Option<&'static dyn PlatformAdapter> {
    table().get(code).copied()
}

/// 平台 code → 出站适配器；未登记 code（registry 清单外的用户协议变体）回落通用适配。
pub fn adapter_for(code: &str) -> &'static dyn PlatformAdapter {
    table().get(code).copied().unwrap_or(&GENERIC)
}
