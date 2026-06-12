# PRD: 迁移 proxy_settings.json 到 DB

## 目标

`~/.aidog/proxy_settings.json`（port + autostart）迁移到 SQLite `settings` 表，与项目所有其他设置一致。迁移后删除文件 I/O。

## 改动

### lib.rs

1. `load_proxy_settings` → 从 DB `get_setting("proxy", "settings")` 读取
2. `save_proxy_settings` → 写入 DB `set_setting("proxy", "settings", value)`
3. 删除 `settings_path` 函数
4. 首次加载时检测文件存在则迁移（读文件→写DB→删文件）
5. `proxy_get_settings` / `proxy_set_port` / `proxy_set_autostart` 等 command 改为接收 `Db` state

### 清理

- `balance.yaml` / `config.yaml` 无代码引用（已确认），无需改动
- 删除 `ProxySettings` struct 的文件 I/O 相关代码

## 约束

- 已有 settings 表（scope+key+value JSON），复用 `db::get_setting` / `db::set_setting`
- 迁移一次性：文件存在且 DB 无记录时迁移，之后忽略文件

## 验收

- [ ] `proxy_settings.json` 不再被读写
- [ ] 端口/autostart 存在 DB `settings` 表中
- [ ] 现有文件自动迁移到 DB（一次性）
- [ ] `cargo check` 零 error 零 warning
