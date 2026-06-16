# updater 新签名密钥对

## 背景
原 tauri.conf.json 的 updater pubkey 对应的私钥已丢失，CI 报 `Missing comment in secret key`。已本地新生成密钥对（`~/.tauri/aidog.key` + `.pub`，仓库外，空口令）。

## 范围（仅 2 处仓库文件）
1. `src-tauri/tauri.conf.json` plugins.updater.pubkey → 替换为新公钥（`~/.tauri/aidog.key.pub` 内容）。
2. `.gitignore` 加 `*.key` / `*.key.pub` 防线（防任何密钥误提交；私钥本就在仓库外）。

## 非范围
- GitHub secrets（`TAURI_SIGNING_PRIVATE_KEY` = base64 私钥文件 / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = 空）由用户本机手动设，私钥不进对话/不外传。

## 验收
- tauri.conf.json pubkey = 新公钥。
- .gitignore 含 *.key 规则。
- JSON 合法。
