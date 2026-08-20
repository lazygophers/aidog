---
name: delete-platform-no-cascade
title: delete_platform 软删禁连带删组
layer: core
category: domain
keywords: [cascade,lifecycle,platform,group]
created: 1725080438
inclusion: auto
---

`delete_platform` 仅软删平台，禁物理删，且禁连带删关联组。

- `db/platform_lifecycle.rs:29 fn delete_platform`
- `:48 invalidate_groups_cache()` 仅清缓存
- `:47` 注释明确「展示为无成员卡片，与手动空组一致」
