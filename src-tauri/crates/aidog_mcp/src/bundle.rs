use std::collections::BTreeMap;

use aidog_db::{Db, models::SetSettingInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{McpAgent, McpConfigRaw, backend_for};

const SETTING_KEY: &str = "mcp_bundle_own_write";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct BundleWrite {
    bundle: String,
    after: Option<String>,
}

type Ledger = BTreeMap<String, BundleWrite>;

fn key(agent: McpAgent, name: &str) -> String {
    format!("{}:{name}", agent.slug())
}

fn fingerprint(cfg: &McpConfigRaw) -> String {
    let bytes = serde_json::to_vec(cfg).expect("McpConfigRaw serialization is infallible");
    let hash = Sha256::digest(bytes);
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

async fn load(db: &Db) -> Ledger {
    aidog_db::get_setting(db, "global", SETTING_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

async fn store(db: &Db, ledger: &Ledger) -> Result<(), String> {
    aidog_db::set_setting(
        db,
        SetSettingInput {
            scope: "global".into(),
            key: SETTING_KEY.into(),
            value: serde_json::to_value(ledger).map_err(|e| e.to_string())?,
        },
    )
    .await
    .map(|_| ())
}

pub(crate) fn current(agent: McpAgent, name: &str) -> Result<Option<McpConfigRaw>, String> {
    Ok(backend_for(agent)
        .read_all()?
        .into_iter()
        .find_map(|(n, cfg)| (n == name).then_some(cfg)))
}

pub(crate) async fn is_owned(db: &Db, agent: McpAgent, name: &str) -> bool {
    load(db).await.contains_key(&key(agent, name))
}

pub(crate) async fn unchanged(
    db: &Db,
    agent: McpAgent,
    name: &str,
    current: Option<&McpConfigRaw>,
) -> bool {
    let ledger = load(db).await;
    let Some(rec) = ledger.get(&key(agent, name)) else {
        return false;
    };
    current.map(fingerprint) == rec.after
}

pub(crate) async fn record_after(
    db: &Db,
    agent: McpAgent,
    name: &str,
    current: Option<&McpConfigRaw>,
) -> Result<(), String> {
    let mut ledger = load(db).await;
    let Some(rec) = ledger.get_mut(&key(agent, name)) else {
        return Ok(());
    };
    rec.after = current.map(fingerprint);
    store(db, &ledger).await
}

pub(crate) async fn clear(db: &Db, agent: McpAgent, name: &str) -> Result<(), String> {
    let mut ledger = load(db).await;
    ledger.remove(&key(agent, name));
    store(db, &ledger).await
}

/// Record a plugin-owned MCP entry after its first write. Later edits are kept
/// when a plugin update, resync or removal checks this fingerprint.
pub async fn mark_bundle_entry(
    db: &Db,
    name: &str,
    agent: McpAgent,
    bundle: &str,
) -> Result<(), String> {
    let mut ledger = load(db).await;
    let cfg = current(agent, name)?;
    ledger.insert(
        key(agent, name),
        BundleWrite {
            bundle: bundle.to_string(),
            after: cfg.as_ref().map(fingerprint),
        },
    );
    store(db, &ledger).await
}
