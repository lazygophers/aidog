//! 平台出站适配器框架（issue 68）。
//!
//! 转发层按请求选中的平台（`platform_type` 的 registry code）查找其出站整理
//! 适配器，由适配器完成该平台端点的全部出站转换与平台差异钩子。接口默认实现
//! 全部委托协议构件（[`crate::converter`] + [`crate::protocols`]），即家族基座；
//! 所有平台初始映射到默认行为，与历史按 wire 协议分派逐字节等价。平台家族
//! 成员只覆盖差异钩子（示例见 [`registry::KimiFamilyAdapter`]）。
//!
//! 端点来源 = 用户平台配置（`platform.endpoints`，运行时只读 DB）；registry
//! 内置端点仅启动灌种进 DB，出站永远按用户配置的 (protocol, base_url) 出站。

pub mod error;
pub mod hooks;
pub mod registry;
pub mod trait_def;
pub mod usage;

pub use registry::{adapter_for, lookup};
pub use trait_def::{PlatformAdapter, UsageTuple};

#[cfg(test)]
#[path = "test_platform.rs"]
mod test_platform;
