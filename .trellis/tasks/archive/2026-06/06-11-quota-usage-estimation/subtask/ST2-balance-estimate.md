# ST2: 余额预估

- **目标**: 请求后台精确预估扣减余额
- **产出**:
  - db.rs `estimate_balance(db, platform_id, model, platform_type, in_tok, out_tok, cache_tok)`：resolve_price(db.rs:1074) 算 cost = in×in_cost+out×out_cost+cache×cache_cost；**原子自减单 SQL** `UPDATE platform SET est_balance_remaining=est_balance_remaining-?, estimate_count=estimate_count+1 WHERE id=?`（禁持锁跨 await）
  - proxy.rs: upsert_log 后（非流式 :487 / 流式 token 收尾）`tokio::spawn` 调用（不阻塞响应）；coding plan 平台跳过余额
- **验证**: cargo build + 单测（原子自减、cost 计算）
- **资源**: research/estimate-trigger-architecture.md + resolve-price-coding-plan.md、design.md
- **依赖**: ST1
- **失败处理**: 持锁跨 await 编译/死锁 → 锁内只 SQL，resolve_price 锁外或单连接
