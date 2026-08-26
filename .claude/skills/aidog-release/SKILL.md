---
name: aidog-release
description: |
  aidog 发版全流程（版本号 → push 触发 CI → 后台监控构建 → 更新 Release 说明）。固化 `.version` 单一可信源（禁手改 package.json / tauri.conf.json / Cargo.toml / docs/package.json，必须跑 sync-version.mjs）、CI 的 `--check` 漂移门禁、tag 撞车与 workflow_dispatch 重发路径、gh run 后台监控（禁主 agent sleep 轮询）、gh release edit 覆盖 releaseBody 模板。触发词：发版、发布新版本、release、出包、bump 版本、更新版本号、打 tag、CI 挂了、构建失败、release 说明、changelog、latest.json、自动更新。
when_to_use: 要发一个新版本（改版本号 + 触发 CI + 出安装包）；release CI 失败要定位修复；已发布的 Release 说明要补 changelog；版本漂移（sync-version --check 红）要修
disable-model-invocation: true
paths:
  - .version
  - scripts/sync-version.mjs
  - .github/workflows/release.yml
---

# aidog 发版

一次发版 = **改 `.version` → 同步 manifest → commit+push master → CI 出包 → 补 Release 说明**。

> 行号漂移，定位以**文件名 / 符号名**为准。

---

## 0. 架构铁律（动手前必读）

1. **版本唯一可信源 = 根目录 `.version`（单行 semver）。** `package.json` / `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` / `docs/package.json` 全是派生物，只能由 `node scripts/sync-version.mjs` 写。手改 manifest = 漂移，CI 的 `Verify version sync` 步骤（`sync-version.mjs --check`）直接 exit 1。
2. **CI 触发条件 = push master 且 diff 含 `.version` 或 `release.yml`**（`.github/workflows/release.yml:5-9`）。只改 manifest 不改 `.version` → 不触发；改了 `.version` 但没跑 sync → 触发后在门禁步骤挂。
3. **tag `v<版本>` 已存在时 tauri-action 失败。** 正常 push 路径假定新版本无碰撞；重发同版本必须走 `workflow_dispatch`（cleanup job 会 `gh release delete --cleanup-tag`）。
4. **Release 正文由 workflow 的 `releaseBody` 模板生成**（下载表 / macOS quarantine / 自动更新 / 文档链接）。事后 `gh release edit` 覆盖的内容，**重新 dispatch 同版本会被模板重建冲掉** —— 先确认不再重发，再补说明。
5. **禁主 agent `sleep` 轮询 CI。** 监控脚本以 `run_in_background` 起，结果由 harness 通知回注（memory `no-sleep-polling`）。

---

## 1. 更新版本号

默认 **patch 位 +1**（用户没特别说明时）。

```bash
CUR=$(tr -d '[:space:]' < .version)
NEW=$(echo "$CUR" | awk -F. '{printf "%d.%d.%d", $1, $2, $3+1}')
echo "$NEW" > .version
node scripts/sync-version.mjs        # 写入 4 个 manifest
node scripts/sync-version.mjs --check # 必须 ✓
git add -A && git commit -m "chore(release): bump version to $NEW"
```

> 漂移修复同理：先决定目标版本写进 `.version`，再 sync，**不要反向把 `.version` 改成 manifest 的值**（除非确认那个版本还没发过 tag）。

## 2. push 触发 CI

`git push origin master`（本仓 push 需用户明确授权 —— 项目 CLAUDE.md 禁自动 push）。

远端有两个 remote：`origin` = `https://github.com/lazygophers/aidog.git`（真发版目标），`no-mistakes` 是本地镜像，**push 它不触发任何 CI**。

## 3. 后台监控 CI

```bash
bash .claude/skills/aidog-release/scripts/watch-release.sh
```

用 `Bash(run_in_background: true)` 起，日志落 `/tmp/aidog-release-<版本>.log`。脚本按**本地 HEAD sha** 匹配 run，不会盯错上一次的 run；失败时自动追加 `gh run view --log-failed` 尾部日志到同一文件。

matrix 两个 job：`macos-latest`（`--target universal-apple-darwin`，Apple Silicon + Intel 合一 dmg）、`windows-latest`（x64 exe / msi）。`fail-fast: false`，一个平台挂另一个照跑。

常见失败与处置：

| 症状 | 根因 | 处置 |
| --- | --- | --- |
| `Verify version sync` exit 1 | manifest 被手改 / 漏跑 sync | 跑 `node scripts/sync-version.mjs`，commit 后重推 |
| tauri-action `tag already exists` | 同版本重发 | 走 `gh workflow run release.yml`（cleanup job 删旧 tag+release） |
| 仅 windows 或仅 macos 挂 | 平台特有编译/签名问题 | `gh run view <id> --log-failed` 看真实报错，修完重推新 commit（不必 bump 版本，但需改到 `.version`/`release.yml` 才会重触发 → 用 dispatch 更省事） |
| 签名相关报错 | `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` secret 缺失/过期 | 仓库 secrets 修，非代码问题 |

## 4. 更新 Release 说明

CI 产出的正文只有通用模板，**没有本次改了什么**。补 changelog：

```bash
V=$(tr -d '[:space:]' < .version)
PREV=$(git tag --sort=-v:refname | grep -v "^v$V$" | head -1)
git log --oneline --no-merges "$PREV..v$V"   # 素材
gh release view "v$V" --json body -q .body > /tmp/rel-$V.md
# 在 /tmp/rel-$V.md 顶部（"## 🐕 AiDog" 之后）插入「### ✨ 本次更新」小节，再：
gh release edit "v$V" --notes-file /tmp/rel-$V.md
```

写法：按 conventional commit 的 type 归类（feat / fix / 其余合并成「其他」），每条一行人话，**不要贴 commit hash 和原始 scope 前缀**。保留模板里的下载表 / quarantine / 自动更新 / 文档四节，只加不删。

## 5. 验证门禁

```bash
node scripts/sync-version.mjs --check        # 版本一致
gh run list --workflow=release.yml -L 1      # 状态 completed / success
gh release view "v$(tr -d '[:space:]' < .version)" --json assets -q '.assets[].name'
```

资产必须含：`*_universal.dmg`、`*_x64-setup.exe`、`*_x64_en-US.msi`、对应 `.sig`、`latest.json`（`includeUpdaterJson: true`，缺它客户端「关于」页自动更新会瞎）。

收尾自检：
- [ ] `.version` 是唯一手改的版本文件，4 个 manifest 由脚本写。
- [ ] push 前 `--check` 绿。
- [ ] CI 两个平台 job 都 success。
- [ ] Release 资产含 `latest.json` + 两平台安装包 + `.sig`。
- [ ] Release 正文补了本次 changelog，模板四节没被删。

---

## 反例黑名单（不要做）

1. ❌ 手改 `package.json` / `tauri.conf.json` / `Cargo.toml` / `docs/package.json` 的 version —— 只改 `.version` 后跑 sync。
2. ❌ 主 agent 用 `sleep` + 轮询等 CI —— 后台脚本 + 通知回注。
3. ❌ 未经用户授权 `git push` —— 项目 CLAUDE.md 明令禁止。
4. ❌ 同版本删 tag 手工重发 —— 用 `workflow_dispatch`，cleanup job 已幂等处理。
5. ❌ `gh release edit` 时整段覆盖成纯 changelog —— 下载表和 macOS quarantine 说明是用户唯一入口，必须保留。

## 相关

- workflow：`.github/workflows/release.yml`
- 同步器：`scripts/sync-version.mjs`（`--check` = CI 门禁）
- 监控脚本：`.claude/skills/aidog-release/scripts/watch-release.sh`
- memory `no-sleep-polling`
