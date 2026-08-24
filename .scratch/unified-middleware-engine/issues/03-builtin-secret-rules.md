# 03: 内置密钥规则集（迁移硬编码检测器）

**What to build:** 四类内置规则随 seed 下发并默认生效：AI token（sk-/ghp_/AKIA/AIza/xox…）、
邮箱、手机号（大陆格式 + 特征明确的国际格式）、DB/Redis 凭据（连接串 URI + 明确 key=value
形式）。引擎内写死的密钥/邮箱检测器删除，检测全部由内置规则表达。只匹配特征明确的模式，
高误伤模式（裸 password=、宽松国际号段）明确排除。

**Blocked by:** 02 CRUD + seed + 前端列表页（最小可用）

**Status:** done

- [x] 四类内置规则 seed，各含条件树 + mask 动作，默认启用
- [x] 引擎内硬编码检测器（BUILTIN_SECRET/EMAIL pattern 与 builtin_detectors_match）删除
- [x] pattern 命中/不命中样本测试（含排除的高风险样本）
- [x] cargo test 全绿
