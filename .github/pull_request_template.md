<!--
  标题用 Conventional Commits：<type>(<scope>): <描述>
  type ∈ feat / fix / chore / style / refactor / docs
  例：feat(router): 分组支持按时段切换默认平台
-->

## 改了什么

<!-- 一两句说清这个 PR 做了啥、解决哪个问题 -->

关联 Issue：Closes #

## 为什么

<!-- 动机 / 背景。reviewer 看这段判断方向对不对 -->

## 改动范围

<!-- 勾选涉及的层，跨 Rust↔TS 边界的改动尤其要标 -->

- [ ] Rust 后端（gateway / proxy / converter / router / db …）
- [ ] React 前端（pages / components / services）
- [ ] 跨 Rust↔TS 边界（字段名 / 类型契约）
- [ ] 文档（docs/ Rspress 站点）
- [ ] 构建 / CI / 脚本

## 自测门禁

<!-- 本地跑过的打勾；前端无 lint/test 脚本，门禁即下列 -->

**前端**
- [ ] `yarn build`（tsc && vite build）无报错
- [ ] `yarn check:i18n` 通过（动了 i18n key 时必跑）
- [ ] `yarn version:check` 通过（动了 .version / 各 manifest 时必跑）
- [ ] `yarn test:statusline-golden`（动了 statusline 脚本时）

**后端**（动了 `src-tauri/` 时必跑）
- [ ] `cargo build` 通过
- [ ] `cargo clippy` 零 warning
- [ ] `cargo test` 通过

## 截图 / 录屏

<!-- 改了 UI 就贴前后对比；纯后端可删本节 -->

## 自查

- [ ] 标题符合 Conventional Commits
- [ ] 无 API Key / token / 私钥等敏感信息混入（含日志、测试数据、截图）
- [ ] 改了跨层契约的，前后端字段名 / 类型已对齐
- [ ] 新增 UI 文案已补全 7 种语言 i18n（漏补会被 check:i18n 拦）
