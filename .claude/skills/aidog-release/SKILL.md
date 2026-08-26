---
name: aidog-release
description: |
  aidog 发版全流程（版本号 → push 触发 CI → 后台监控构建 → 更新 Release 说明）。固化 `.version` 单一可信源（禁手改 package.json / tauri.conf.json / Cargo.toml / docs/package.json，必须跑 sync-version.mjs）、CI 的 `--check` 漂移门禁、tag 撞车与 workflow_dispatch 重发路径、gh run 后台监控（禁主 agent sleep 轮询）、gh release edit 覆盖 releaseBody 模板。触发词：发版、发布新版本、release、出包、bump 版本、更新版本号、打 tag、CI 挂了、构建失败、release 说明、changelog、latest.json、自动更新。
when_to_use: 要发一个新版本（改版本号 + 触发 CI + 出安装包）；release CI 失败要定位修复；已发布的 Release 说明要补 changelog；版本漂移（sync-version --check 红）要修
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
4. **Release 正文由 workflow 的 `releaseBody` 模板生成**（下载表 / macOS quarantine / 自动更新 / 文档链接）。`gh release edit` 写进去的 changelog **会被同版本重发（workflow_dispatch）整体重建冲掉** —— 所以正文的可信副本永远是仓库里的 `.github/release-notes/v<版本>.md`，**每次 dispatch 重发完成后立刻重跑 §4 的 `gh release edit`**，这条闭环是发版流程的一部分，不是可选收尾。
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
git add .version package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml docs/package.json
git commit -m "chore(release): bump version to $NEW"
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
| tauri-action `tag already exists` | 同版本重发 | 走 `gh workflow run release.yml`（cleanup job 删旧 tag+release），**跑完重跑 §4 的 `gh release edit`**（§0-4） |
| 仅 windows 或仅 macos 挂 | 平台特有编译/签名问题 | `gh run view <id> --log-failed` 看真实报错。修完代码后版本号不用动 —— 但 push 只在 diff 含 `.version`/`release.yml` 时才触发，所以同版本重跑一律走 `gh workflow run release.yml`，同样**跑完重跑 §4** |
| 签名相关报错 | `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` secret 缺失/过期 | 仓库 secrets 修，非代码问题 |

## 4. 更新 Release 说明

CI 产出的正文只有通用模板，**没有本次改了什么**。

**完整性铁律：changelog 的范围必须是「上一个 release tag → 本次 tag」的全部非 merge commit，一条不漏。**
注意 tag 不连续（`v0.1.9` 之后直接是 `v0.1.11`，没有 `v0.1.10`），所以上一版**必须按 tag 排序取**，不能按版本号 -1 推算。

```bash
V=$(tr -d '[:space:]' < .version)
PREV=$(git tag --sort=-v:refname | grep -v "^v$V$" | head -1)   # 上一个真实存在的 tag
git rev-list --count --no-merges "$PREV..HEAD"                  # 总数，写完要对得上
git log --reverse --pretty="%s" --no-merges "$PREV..HEAD" | grep -E "^(feat|fix|style|perf|i18n)"   # 用户可见
git log --reverse --pretty="%s" --no-merges "$PREV..HEAD" | grep -Ev "^(feat|fix|style|perf|i18n)"  # 内部（refactor/chore/docs/test）
```

正文草稿放 `.github/release-notes/v<版本>.md`（随代码入库，发版前就能写好、可 review、可 diff；注意 `.scratch/` 被全局 gitignore，放那里不进版本库），CI 跑完直接：

```bash
gh release edit "v$V" --notes-file .github/release-notes/v$V.md
```

写法：

- 分四段 —— `新增` / `修复` / `内部（不影响使用）` / 保留模板四节（下载表 / macOS quarantine / 自动更新 / 文档），**只加不删**。
- 用户可见的 commit 按**功能域**合并成一条人话（同一 feature 的多个 tracer-bullet 票合并，别一票一行），内部 commit 允许整段概括，但 **refactor/chore/docs 不能整类丢弃** —— 大规模拆 crate、覆盖率、文档重做这些用户会问「这版到底动了什么」。
- 不贴 commit hash，不留 `feat(scope):` 前缀。
- 收尾核对：每一条 commit 都能在正文里找到归属（合并进某条也算），数量对得上 `git rev-list --count`。

## 5. 验证门禁

```bash
node scripts/sync-version.mjs --check        # 版本一致
gh run list --workflow=release.yml -L 1      # 状态 completed / success
gh release view "v$(tr -d '[:space:]' < .version)" --json assets -q '.assets[].name'
```

v0.1.11 实际资产清单（8 个，作为基线比对）：

```
AiDog_<版本>_universal.dmg           # macOS 安装包（无 .sig，dmg 不参与 updater）
AiDog_universal.app.tar.gz(+.sig)    # macOS updater 包 —— 自动更新真正下载的是它
AiDog_<版本>_x64-setup.exe(+.sig)    # Windows NSIS（推荐）
AiDog_<版本>_x64_en-US.msi(+.sig)    # Windows MSI
latest.json                          # updater 清单（includeUpdaterJson: true），缺它客户端「关于」页自动更新会瞎
```

收尾自检：
- [ ] `.version` 是唯一手改的版本文件，4 个 manifest 由脚本写。
- [ ] push 前 `--check` 绿。
- [ ] CI 两个平台 job 都 success。
- [ ] Release 资产 8 个齐（dmg / app.tar.gz+sig / exe+sig / msi+sig / latest.json）。
- [ ] Release 正文补了本次 changelog，模板四节没被删；若期间 dispatch 重发过，`gh release edit` 已重跑一次。

---

## 反例黑名单（§0 铁律未覆盖的独有坑）

1. ❌ 未经用户授权 `git push` —— 项目 CLAUDE.md 明令禁止，发版这步永远由用户拍板。
2. ❌ 按版本号 -1 推算上一个版本 —— tag 不连续（`v0.1.9` 后直接 `v0.1.11`），只能 `git tag --sort=-v:refname` 取。
3. ❌ 把正文草稿放 `.scratch/` —— 全局 gitignore，不进版本库，dispatch 重发后就没得恢复。
4. ❌ changelog 只写 feat/fix，整类丢掉 refactor/chore/docs —— 拆 crate、覆盖率、文档重做这些用户会问「这版到底动了什么」。

## 相关

- workflow：`.github/workflows/release.yml`
- 同步器：`scripts/sync-version.mjs`（`--check` = CI 门禁）
- 监控脚本：`.claude/skills/aidog-release/scripts/watch-release.sh`
- memory `no-sleep-polling`
