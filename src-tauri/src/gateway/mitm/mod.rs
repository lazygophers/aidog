//! P3 MITM 解密隧道子系统入口。
//!
//! 当前仅 ST1（假 CA）+ ST2（白名单）基础设施落地。ST3（TLS 层）/ ST4（CONNECT 升级）
//! / ST5（forward 接入）/ ST6（HTTP/2 ALPN）由后续 subtask 补，本模块暂不接入代理热路径。
//!
//! 子模块:
//! - `ca`: rcgen 生成 Root CA + DB 持久化（明文 + DB 文件权限 0600，D4/D5）
//!   + 装信任库（macOS/Windows/Linux 经 tauri-plugin-shell + sudo，D1/D8）+ 清理（ST9）
//! - `whitelist`: 全局 host suffix 匹配（D6），默认 AI host + 已配平台 host（migration 041 填）
//!
//! 设计依据：`.trellis/tasks/07-03-proxy-relay-mitm/design.md`、
//! `.trellis/spec/backend/proxy-connect-relay.md`（P1 契约，P3 待扩展）。
//!
//! `dead_code` allow: ST1/ST2 是独立基础设施模块，CA 生成 / 白名单匹配 / DB 持久化
//! 等公开 API 留待 ST3（TLS accept）+ ST4（CONNECT 分流）+ ST5（forward 接入）消费。
//! 本 subtask 不接入热路径，故全部公开 API 暂未被业务代码引用；接入后移除 allow。

#![allow(dead_code)]

pub mod ca;
pub mod whitelist;
