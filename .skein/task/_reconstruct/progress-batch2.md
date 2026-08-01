# Spec Batch 2 进度

## A 步骤 ✅ 完成
- [x] http-client-no-env-proxy (core/proxy)
- [x] zh-hans-literal-sync (core/i18n)

## C 步骤 C1/C3 ✅ 完成
改名链修复（8 文件）：
- trellis-04 → protocol-variant-extension
- auto-fix-downgrade-34 → db-split-access-point-audit
- auto-fix-downgrade-38 → enum-variant-delete-needs-migration
- rule-57 → protocol-wire-str
- rule-58 → adapter-deadcode-whitelist-authority
- mock-platform-short-circuit → core/arch/mock-platform-bypasses-forward-pipeline
- trellis-03 删链（已过期）

## B 步骤 🔄 进行中
Fork agent 写 recall 规则（12 categories + ~60 rules）

## C 步骤 C2 ⏳ 待处理
B 步完成后改链接（对应表已备）：
- shadcn-infra-28 → shadcn-add-verify-deps
- shadcn-infra-30 → css-var-alias-layer
- shadcn-infra-31 → theme-token-runtime-switch
- shadcn-infra-32 → locale-deadkey-cleanup-ownership
- trellis-18 → frontend-conventions
- rule-45 → planning-scope-pregrep
- auto-fix-downgrade-36 → grep-before-write
- dirty-float-hour-normalization (keep-slug)
- form-level-tz-state-sharing (keep-slug)

## 断链现状
- 初始：25 条
- 现在：14 条（4 条已修，7 条自动消失）
- 待修：9 条 C2（B 步完成自动消失）+ 3 条 C5 误判（禁改）+ 2 条其他

## 已提交
- commit 799a6b98: A 步 + C1/C3
