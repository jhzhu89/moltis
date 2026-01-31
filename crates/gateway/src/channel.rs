use std::sync::Arc;

use {
    async_trait::async_trait,
    serde_json::Value,
    tokio::sync::RwLock,
    tracing::{error, info, warn},
};

use {moltis_channels::ChannelPlugin, moltis_telegram::TelegramPlugin};

use moltis_channels::store::{ChannelStore, StoredChannel};

use crate::services::{ChannelService, ServiceResult};

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Live channel service backed by `TelegramPlugin`.
pub struct LiveChannelService {
    telegram: Arc<RwLock<TelegramPlugin>>,
    store: Arc<dyn ChannelStore>,
}

impl LiveChannelService {
    pub fn new(telegram: TelegramPlugin, store: Arc<dyn ChannelStore>) -> Self {
        Self {
            telegram: Arc::new(RwLock::new(telegram)),
            store,
        }
    }
}

#[async_trait]
impl ChannelService for LiveChannelService {
    async fn status(&self) -> ServiceResult {
        let tg = self.telegram.read().await;
        let account_ids = tg.account_ids();
        let mut channels = Vec::new();

        if let Some(status) = tg.status() {
            for aid in &account_ids {
                match status.probe(aid).await {
                    Ok(snap) => {
                        let mut entry = serde_json::json!({
                            "type": "telegram",
                            "name": format!("Telegram ({})", aid),
                            "account_id": aid,
                            "status": if snap.connected { "connected" } else { "disconnected" },
                            "details": snap.details,
                        });
                        if let Some(cfg) = tg.account_config(aid) {
                            entry["config"] = cfg;
                        }
                        channels.push(entry);
                    },
                    Err(e) => {
                        channels.push(serde_json::json!({
                            "type": "telegram",
                            "name": format!("Telegram ({})", aid),
                            "account_id": aid,
                            "status": "error",
                            "details": e.to_string(),
                        }));
                    },
                }
            }
        }

        Ok(serde_json::json!({ "channels": channels }))
    }

    async fn add(&self, params: Value) -> ServiceResult {
        let channel_type = params
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("telegram");

        if channel_type != "telegram" {
            return Err(format!("unsupported channel type: {channel_type}"));
        }

        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;

        let config = params
            .get("config")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        info!(account_id, "adding telegram channel account");

        let mut tg = self.telegram.write().await;
        tg.start_account(account_id, config.clone())
            .await
            .map_err(|e| {
                error!(error = %e, account_id, "failed to start telegram account");
                e.to_string()
            })?;

        let now = unix_now();
        if let Err(e) = self
            .store
            .upsert(StoredChannel {
                account_id: account_id.to_string(),
                channel_type: "telegram".into(),
                config,
                created_at: now,
                updated_at: now,
            })
            .await
        {
            warn!(error = %e, account_id, "failed to persist channel");
        }

        Ok(serde_json::json!({ "added": account_id }))
    }

    async fn remove(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;

        info!(account_id, "removing telegram channel account");

        let mut tg = self.telegram.write().await;
        tg.stop_account(account_id).await.map_err(|e| {
            error!(error = %e, account_id, "failed to stop telegram account");
            e.to_string()
        })?;

        if let Err(e) = self.store.delete(account_id).await {
            warn!(error = %e, account_id, "failed to delete channel from store");
        }

        Ok(serde_json::json!({ "removed": account_id }))
    }

    async fn logout(&self, params: Value) -> ServiceResult {
        self.remove(params).await
    }

    async fn update(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;

        let config = params
            .get("config")
            .cloned()
            .ok_or_else(|| "missing 'config'".to_string())?;

        info!(account_id, "updating telegram channel account");

        let mut tg = self.telegram.write().await;

        // Stop then restart with new config
        tg.stop_account(account_id).await.map_err(|e| {
            error!(error = %e, account_id, "failed to stop telegram account for update");
            e.to_string()
        })?;

        tg.start_account(account_id, config.clone())
            .await
            .map_err(|e| {
                error!(error = %e, account_id, "failed to restart telegram account after update");
                e.to_string()
            })?;

        let now = unix_now();
        if let Err(e) = self
            .store
            .upsert(StoredChannel {
                account_id: account_id.to_string(),
                channel_type: "telegram".into(),
                config,
                created_at: now,
                updated_at: now,
            })
            .await
        {
            warn!(error = %e, account_id, "failed to persist channel update");
        }

        Ok(serde_json::json!({ "updated": account_id }))
    }

    async fn send(&self, _params: Value) -> ServiceResult {
        Err("direct channel send not yet implemented".into())
    }
}
