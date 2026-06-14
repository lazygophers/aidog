# Design: 导入导出子系统

## 文件格式 `.aidogx`

```
偏移    长度    字段                说明
0       4       magic              b"ADGX"
4       1       format_version     0x01
5       1       flags              reserved
6       1       nonce_len          12 (GCM 标准)
7       1       obf_key_len        32
8       32      obfuscated_key     K XOR pad  （pad = SHA256(magic||version||salt)[:32]，salt = 固定常量）
40      12      nonce              GCM nonce
52      8       payload_len        u64 LE
60      N       ciphertext+tag     AES-256-GCM(payload) (tag 在末尾 16B)
60+N    32      hmac               HMAC-SHA256(key=pad, msg=header[0:60+N])
```

- 重组密钥：`K = obfuscated_key XOR SHA256(b"ADGX"||0x01||salt)[:32]`
- 人眼看：magic 之后全是看似随机的字节流（obf_key / nonce / cipher / hmac 视觉无差异）
- 校验链：HMAC 防篡改 → GCM tag 防篡改+解密失败 → manifest.checksum 防语义损坏

## payload 结构（明文 JSON，加密前）

```jsonc
{
  "manifest": {
    "format_version": 1,
    "aidog_version": "x.y.z",
    "created_at": "2026-06-14T...",
    "source_machine": "<hostname/hash>",
    "scopes": ["platform","group","codex","claude_code","proxy_setting","model_price","skills"],
    "checksum": "<sha256 of plaintext payload excluding checksum field>"
  },
  "platform": [{ ... 全字段 ... }],
  "group": [{ ... }],
  "group_platform": [{ group_name, platform_id }],
  "setting": [{ key, value }],          // proxy 全局设置
  "codex_global": "<~/.codex/config.toml 文本>",
  "codex_profiles": [{ group, toml }],
  "claude_code_group_settings": [{ group, json }],
  "model_price": [{ ... }],
  "skills": [{ name, source, scope, agents: [{ slug, enabled }] }]
}
```

## 模块划分（src-tauri/src/gateway/）

| 文件 | 职责 |
| --- | --- |
| `import_export/mod.rs` | 子系统入口 + 公共类型 |
| `import_export/container.rs` | `.aidogx` 读写：`encrypt(payload_bytes) -> Vec<u8>` / `decrypt(file_bytes) -> Vec<u8>` |
| `import_export/collect.rs` | 导出：从 db + 文件系统收集各 scope 数据 → payload |
| `import_export/apply.rs` | 导入：payload → db 写入 + 文件回写 + 冲突检测 |
| `import_export/skills_sync.rs` | skills 自动化：add/enable/disable via npx |

## Tauri commands（lib.rs）

```rust
export_collect(scopes: Vec<String>) -> Result<Vec<u8>, String>      // 返回加密字节
export_to_file(scopes: Vec<String>, path: String) -> Result<(), String>
import_read_file(path: String) -> Result<ImportPreview, String>     // 解密 + 返回 manifest + 冲突清单
import_apply(path: String, decisions: Vec<ConflictDecision>) -> Result<ImportReport, String>
```

前端流程：
1. export：勾选 scope → `tauri-plugin-dialog` save → `export_to_file`
2. import：open → `import_read_file` 拿 preview → 冲突弹窗收集 decisions → `import_apply`

## 冲突模型

```rust
enum ConflictKind { DuplicateName, DuplicateKey }
struct ConflictItem { scope: String, key: String, existing_summary: String, incoming_summary: String }
enum Decision { Overwrite, Skip, Rename(String) }
struct ConflictDecision { scope, key, decision }
```

后端 `import_apply` 接受 decisions，逐项应用：
- Overwrite → INSERT OR REPLACE
- Skip → 跳过
- Rename → 改名后 INSERT

## Skills 自动化（导入子流程）

导出时从锁文件 + `npx skills list --json` 收集：
```jsonc
[{ "name": "...", "source": "owner/repo", "scope": "user"|"project", "agents": [{ "slug": "claude-code", "enabled": true }] }]
```

导入时对每条：
1. `npx skills add <source> -s <name> -a <slug> -g -y`（每个 enabled agent）
2. 对 enabled=false 的 agent → `npx skills remove`（保持与原一致）
3. scope 必须与原一致（user → `-g`，project → 无 `-g`）

## 安全考量

- 密钥 K 永不出现在日志（tracing 时 mask）
- salt 是编译期常量（非密钥，只是让 pad 非显然）
- pad 与 obfuscated_key 都在文件内，但 XOR 结果（真 K）只在内存瞬时存在
- 注意：此方案不是强加密（密钥可被程序重组 = 任何拿到文件+程序的人可解密），满足的是"人眼无法判断密钥"而非"防逆向"。PRD 已明确此约束。

## 事务性

导入 apply 用单个 SQLite 事务包裹所有 db 写入；任一失败回滚。文件类（codex/claude-code）先备份原文件到 `.bak`，写失败回滚。

## 新增依赖（Cargo.toml）

```toml
aes-gcm = "0.10"
sha2 = "0.10"
hmac = "0.12"
rand = "0.8"
base64 = "0.22"
```

