// types.ts — barrel：统一 re-export 各类型分片。
// 域文件 `import type { X } from "./types"` 保持不变。
// c1b-tsrs：手写 part1~5.ts 已由 ts-rs codegen 取代，Rust struct 变为唯一真值源。
// generated/*（cargo test -p aidog_core 产出，禁手改）+ manual.ts（锁定 enum / camelCase DTO /
// 越界 crate / 无 Rust 背书的前端类型，见该文件顶部 6 节分类）。

export * from "./types/generated";
export * from "./types/manual";
