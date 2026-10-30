//! client-types 内置常量（原 defaults/client-types.json，2026-08-16 内置化）。
//! 手维护：改这里，禁再落回外部 JSON 文件。

pub const BUNDLED: &str = r##"{
  "last_updated": 1793367651,
  "client_types": [
    {
      "value": "default",
      "simulation": {
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ]
        }
      }
    },
    {
      "value": "claude_code",
      "simulation": {
        "user_agent": "claude-cli/2.1.204 (external, cli)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "anthropic-version", "value": "2023-06-01"},
            {"name": "anthropic-beta", "value": "claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11"},
            {"name": "anthropic-dangerous-direct-browser-access", "value": "true"},
            {"name": "x-app", "value": "cli"},
            {"name": "x-stainless-arch", "value": "arm64"},
            {"name": "x-stainless-lang", "value": "js"},
            {"name": "x-stainless-os", "value": "MacOS"},
            {"name": "x-stainless-package-version", "value": "0.94.0"},
            {"name": "x-stainless-retry-count", "value": "0"},
            {"name": "x-stainless-runtime", "value": "node"},
            {"name": "x-stainless-runtime-version", "value": "v26.3.0"},
            {"name": "x-stainless-timeout", "value": "600"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ]
        }
      }
    },
    {
      "value": "claude_code_vscode",
      "simulation": {
        "user_agent": "claude-cli/2.1.204 (external, claude-vscode, agent-sdk/0.1.30)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "anthropic-version", "value": "2023-06-01"},
            {"name": "anthropic-beta", "value": "claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11"},
            {"name": "anthropic-dangerous-direct-browser-access", "value": "true"},
            {"name": "x-app", "value": "cli"},
            {"name": "x-stainless-arch", "value": "arm64"},
            {"name": "x-stainless-lang", "value": "js"},
            {"name": "x-stainless-os", "value": "MacOS"},
            {"name": "x-stainless-package-version", "value": "0.94.0"},
            {"name": "x-stainless-retry-count", "value": "0"},
            {"name": "x-stainless-runtime", "value": "node"},
            {"name": "x-stainless-runtime-version", "value": "v26.3.0"},
            {"name": "x-stainless-timeout", "value": "600"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ]
        }
      }
    },
    {
      "value": "claude_code_sdk_ts",
      "simulation": {
        "user_agent": "claude-cli/2.1.204 (external, sdk-ts)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "anthropic-version", "value": "2023-06-01"},
            {"name": "anthropic-beta", "value": "claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11"},
            {"name": "anthropic-dangerous-direct-browser-access", "value": "true"},
            {"name": "x-app", "value": "cli"},
            {"name": "x-stainless-arch", "value": "arm64"},
            {"name": "x-stainless-lang", "value": "js"},
            {"name": "x-stainless-os", "value": "MacOS"},
            {"name": "x-stainless-package-version", "value": "0.94.0"},
            {"name": "x-stainless-retry-count", "value": "0"},
            {"name": "x-stainless-runtime", "value": "node"},
            {"name": "x-stainless-runtime-version", "value": "v26.3.0"},
            {"name": "x-stainless-timeout", "value": "600"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ]
        }
      }
    },
    {
      "value": "claude_code_sdk_py",
      "simulation": {
        "user_agent": "claude-cli/2.1.204 (external, sdk-py)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "anthropic-version", "value": "2023-06-01"},
            {"name": "anthropic-beta", "value": "claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11"},
            {"name": "anthropic-dangerous-direct-browser-access", "value": "true"},
            {"name": "x-app", "value": "cli"},
            {"name": "x-stainless-arch", "value": "arm64"},
            {"name": "x-stainless-lang", "value": "js"},
            {"name": "x-stainless-os", "value": "MacOS"},
            {"name": "x-stainless-package-version", "value": "0.94.0"},
            {"name": "x-stainless-retry-count", "value": "0"},
            {"name": "x-stainless-runtime", "value": "node"},
            {"name": "x-stainless-runtime-version", "value": "v26.3.0"},
            {"name": "x-stainless-timeout", "value": "600"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ]
        }
      }
    },
    {
      "value": "claude_code_gh_action",
      "simulation": {
        "user_agent": "claude-cli/2.1.204 (external, claude-code-github-action)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "anthropic-version", "value": "2023-06-01"},
            {"name": "anthropic-beta", "value": "claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11"},
            {"name": "anthropic-dangerous-direct-browser-access", "value": "true"},
            {"name": "x-app", "value": "cli"},
            {"name": "x-stainless-arch", "value": "arm64"},
            {"name": "x-stainless-lang", "value": "js"},
            {"name": "x-stainless-os", "value": "MacOS"},
            {"name": "x-stainless-package-version", "value": "0.94.0"},
            {"name": "x-stainless-retry-count", "value": "0"},
            {"name": "x-stainless-runtime", "value": "node"},
            {"name": "x-stainless-runtime-version", "value": "v26.3.0"},
            {"name": "x-stainless-timeout", "value": "600"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"}
          ]
        }
      }
    },
    {
      "value": "codex_cli",
      "simulation": {
        "user_agent": "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"},
            {"name": "OpenAI-Beta", "value": "responses=experimental"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "codex_tui",
      "simulation": {
        "user_agent": "codex-tui/0.38.0",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"},
            {"name": "OpenAI-Beta", "value": "responses=experimental"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "codex_desktop",
      "simulation": {
        "user_agent": "Codex Desktop/0.38.0",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"},
            {"name": "OpenAI-Beta", "value": "responses=experimental"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "codex_vscode",
      "simulation": {
        "user_agent": "codex_vscode/0.38.0",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "api-key", "value": "{api_key}"},
            {"name": "OpenAI-Beta", "value": "responses=experimental"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "pi_cli",
      "simulation": {
        "user_agent": "pi/0.84.2 (darwin; node/v24.0.0; arm64)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "anthropic-version", "value": "2023-06-01"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "kilo_code",
      "simulation": {
        "user_agent": "kilo/latest/7.8.18/cli",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "opencode",
      "simulation": {
        "user_agent": "opencode/1.19.5",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "qwen_code",
      "simulation": {
        "user_agent": "QwenCode/0.26.4 (darwin; arm64)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "roo_code",
      "simulation": {
        "user_agent": "RooCode/3.54.0",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"},
            {"name": "HTTP-Referer", "value": "https://github.com/RooVetGit/Roo-Cline"},
            {"name": "X-Title", "value": "Roo Code"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "HTTP-Referer", "value": "https://github.com/RooVetGit/Roo-Cline"},
            {"name": "X-Title", "value": "Roo Code"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"},
            {"name": "HTTP-Referer", "value": "https://github.com/RooVetGit/Roo-Cline"},
            {"name": "X-Title", "value": "Roo Code"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"},
            {"name": "HTTP-Referer", "value": "https://github.com/RooVetGit/Roo-Cline"},
            {"name": "X-Title", "value": "Roo Code"}
          ]
        }
      }
    },
    {
      "value": "cline",
      "simulation": {
        "user_agent": "Cline/4.1.26",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "factory_droid",
      "simulation": {
        "user_agent": "factory-cli/1.0.0",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "crush",
      "simulation": {
        "user_agent": "Charm-Crush/0.99.0 (https://charm.land/crush)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "continue",
      "simulation": {
        "user_agent": "Continue/IDE",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "zed",
      "simulation": {
        "user_agent": "Zed/1.26.2 (macos; aarch64)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    },
    {
      "value": "gemini_cli",
      "simulation": {
        "user_agent": "GeminiCLI/0.66.0/gemini-pro (darwin; arm64; terminal)",
        "auth": {
          "anthropic": [
            {"name": "x-api-key", "value": "{api_key}"}
          ],
          "openai": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ],
          "gemini": [
            {"name": "x-goog-api-key", "value": "{api_key}"}
          ],
          "default": [
            {"name": "Authorization", "value": "Bearer {api_key}"}
          ]
        }
      }
    }
  ]
}"##;

// ── simulation 配置加载（2026-10-04 自 aidog_core::gateway::proxy::headers 下沉）────
// 下沉动机：aidog_adapter（quota 脚本出站默认 UA）也够得着，避免 core↔adapter 依赖倒挂。
// 消费方：aidog_core apply_client_headers（proxy / model_test / fetch-models 客户端模拟）+
// aidog_adapter quota 脚本默认 UA。真值源不变：app data `~/.aidog/client-types.json` 优先，
// 缺失/空/非 JSON 回落 BUNDLED；OnceLock 启动加载，禁每请求读盘。

use std::collections::HashMap;
use std::sync::OnceLock;

/// 单条 simulation header 定义（name + value 模板，占位符由调用方引擎替换）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SimulationHeader {
    pub name: String,
    pub value: String,
}

/// per-protocol auth 矩阵：serde rename key（`anthropic`/`openai`/`gemini`/...）→ headers 数组。
/// 含保留 key `default` 兜底未知 protocol。
pub type AuthMatrix = HashMap<String, Vec<SimulationHeader>>;

/// 单 entry 的 simulation 配置（全自包含，禁 family 继承）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Simulation {
    /// 缺省 = 不注入 UA（如 `default` entry）。
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub auth: AuthMatrix,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ClientTypeEntry {
    value: String,
    #[serde(default)]
    simulation: Simulation,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ClientTypesDoc {
    client_types: Vec<ClientTypeEntry>,
}

static SIMULATION_CACHE: OnceLock<HashMap<String, Simulation>> = OnceLock::new();

/// app data `~/.aidog/client-types.json` 优先，缺失/空/非 JSON 回落 BUNDLED。
fn load_client_types_body() -> String {
    let path = dirs::home_dir().map(|h| h.join(".aidog").join("client-types.json"));
    if let Some(p) = path
        && let Ok(content) = std::fs::read_to_string(&p)
        && !content.trim().is_empty()
        && serde_json::from_str::<serde_json::Value>(&content).is_ok()
    {
        return content;
    }
    BUNDLED.to_string()
}

/// 首次访问解析 simulation map；解析失败兜底空 map（apply 路径再兜底 Bearer）。
fn simulation_map() -> &'static HashMap<String, Simulation> {
    SIMULATION_CACHE.get_or_init(|| {
        let body = load_client_types_body();
        match serde_json::from_str::<ClientTypesDoc>(&body) {
            Ok(doc) => doc
                .client_types
                .into_iter()
                .map(|e| (e.value, e.simulation))
                .collect(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "client-types.json simulation parse failed (should never happen), using empty map"
                );
                HashMap::new()
            }
        }
    })
}

/// 未知 client_type（JSON 无 entry）→ 回落 `default` entry（PRD R2「等价 default」语义）；
/// `default` 也缺 → None（apply 路径再兜底 Bearer-only）。
pub fn simulation_for(client_type: &str) -> Option<&'static Simulation> {
    simulation_map()
        .get(client_type)
        .or_else(|| simulation_map().get("default"))
}

/// 平台 code（含 newapi / deepseek / glm_coding 等别名，非纯 wire 协议名）→ 模拟客户端
/// UA（平台默认端点 client_type → simulation.user_agent）。quota 脚本出站默认 UA 用：
/// 无对应模拟 UA（default entry 无 UA）→ None 不注入。
pub fn simulation_user_agent_for_platform(code: &str) -> Option<String> {
    let ct = crate::registry::derive_client_type_for_platform(crate::registry::PlatformCode(code));
    simulation_for(&ct).and_then(|s| s.user_agent.clone())
}
