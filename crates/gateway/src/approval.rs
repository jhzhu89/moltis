//! Live approval service and broadcaster for the gateway.

use std::sync::Arc;

use {async_trait::async_trait, serde_json::Value, tracing::{info, warn}};

use moltis_tools::{
    approval::{ApprovalDecision, ApprovalManager},
    exec::ApprovalBroadcaster,
};

use moltis_channels::plugin::{
    ButtonConfirm, ButtonStyle, InteractiveButton, InteractiveMessage,
};

use crate::{
    broadcast::{BroadcastOpts, broadcast},
    services::{ExecApprovalService, ServiceResult},
    state::GatewayState,
};

/// Live approval service backed by an `ApprovalManager`.
pub struct LiveExecApprovalService {
    manager: Arc<ApprovalManager>,
}

impl LiveExecApprovalService {
    pub fn new(manager: Arc<ApprovalManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl ExecApprovalService for LiveExecApprovalService {
    async fn get(&self) -> ServiceResult {
        Ok(serde_json::json!({
            "mode": self.manager.mode,
            "securityLevel": self.manager.security_level,
        }))
    }

    async fn set(&self, _params: Value) -> ServiceResult {
        // Config mutation not yet implemented.
        Ok(serde_json::json!({}))
    }

    async fn node_get(&self, _params: Value) -> ServiceResult {
        Ok(serde_json::json!({ "mode": self.manager.mode }))
    }

    async fn node_set(&self, _params: Value) -> ServiceResult {
        Ok(serde_json::json!({}))
    }

    async fn request(&self, _params: Value) -> ServiceResult {
        let ids = self.manager.pending_ids().await;
        Ok(serde_json::json!({ "pending": ids }))
    }

    async fn resolve(&self, params: Value) -> ServiceResult {
        let id = params
            .get("requestId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'requestId'".to_string())?;

        let decision_str = params
            .get("decision")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'decision'".to_string())?;

        let decision = match decision_str {
            "approved" => ApprovalDecision::Approved,
            "denied" => ApprovalDecision::Denied,
            _ => return Err(format!("invalid decision: {decision_str}").into()),
        };

        let command = params.get("command").and_then(|v| v.as_str());

        info!(id, ?decision, "resolving approval request");
        self.manager.resolve(id, decision, command).await;

        Ok(serde_json::json!({ "ok": true }))
    }
}

/// Maximum display length for commands in approval cards.
const APPROVAL_CMD_DISPLAY_LEN: usize = 200;

/// Truncate a command string for display in approval messages.
///
/// Slack block elements have character limits, so we cap the preview and
/// append "…" when truncated.
pub(crate) fn truncate_command(command: &str) -> String {
    if command.len() <= APPROVAL_CMD_DISPLAY_LEN {
        command.to_string()
    } else {
        format!(
            "{}…",
            &command[..command.floor_char_boundary(APPROVAL_CMD_DISPLAY_LEN)]
        )
    }
}

/// Broadcasts approval requests to connected WebSocket clients.
pub struct GatewayApprovalBroadcaster {
    state: Arc<GatewayState>,
}

impl GatewayApprovalBroadcaster {
    pub fn new(state: Arc<GatewayState>) -> Self {
        Self { state }
    }

    /// Send an interactive approval card to the originating channel, if any.
    async fn send_channel_approval_card(
        &self,
        session_key: &str,
        request_id: &str,
        command: &str,
    ) {
        let target = match self.state.peek_channel_replies(session_key).await.into_iter().next() {
            Some(t) => t,
            None => return,
        };
        let outbound = match self.state.services.channel_outbound_arc() {
            Some(o) => o,
            None => return,
        };

        let display_cmd = truncate_command(command);

        // Slack button `value` field is capped at 2000 chars.  The request_id
        // is sufficient for resolution; include a truncated command only for
        // the post-resolution status message.
        let value_cmd = if command.len() > 1800 {
            &command[..command.floor_char_boundary(1800)]
        } else {
            command
        };
        let value_json = serde_json::json!({
            "request_id": request_id,
            "command": value_cmd,
        })
        .to_string();

        let message = InteractiveMessage {
            text: format!("🔐 *Approval required*\n```{display_cmd}```"),
            button_rows: vec![vec![
                InteractiveButton {
                    label: "✅ Allow".to_string(),
                    callback_data: "exec_approve".to_string(),
                    style: ButtonStyle::Primary,
                    value: Some(value_json.clone()),
                    confirm: Some(ButtonConfirm {
                        title: "Allow command?".to_string(),
                        text: format!("Run: {display_cmd}"),
                        confirm_label: "Allow".to_string(),
                        deny_label: "Cancel".to_string(),
                        danger: false,
                    }),
                },
                InteractiveButton {
                    label: "❌ Deny".to_string(),
                    callback_data: "exec_deny".to_string(),
                    style: ButtonStyle::Danger,
                    value: Some(value_json),
                    confirm: None,
                },
            ]],
            replace_message_id: None,
        };

        if let Err(e) = outbound
            .send_interactive(
                &target.account_id,
                &target.chat_id,
                &message,
                target.message_id.as_deref(),
            )
            .await
        {
            warn!(
                session_key,
                error = %e,
                "failed to send approval card to channel"
            );
        }
    }
}

#[async_trait]
impl ApprovalBroadcaster for GatewayApprovalBroadcaster {
    async fn broadcast_request(
        &self,
        request_id: &str,
        command: &str,
        session_key: Option<&str>,
    ) -> moltis_tools::Result<()> {
        // 1. Broadcast to WebSocket clients (existing web UI path).
        broadcast(
            &self.state,
            "exec.approval.requested",
            serde_json::json!({
                "requestId": request_id,
                "command": command,
            }),
            BroadcastOpts::default(),
        )
        .await;

        // 2. Send interactive approval card to originating channel (if any).
        if let Some(sk) = session_key {
            self.send_channel_approval_card(sk, request_id, command)
                .await;
        }

        Ok(())
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_live_service_resolve() {
        let mgr = Arc::new(ApprovalManager::default());
        let svc = LiveExecApprovalService::new(Arc::clone(&mgr));

        // Create a pending request.
        let (id, mut rx) = mgr.create_request("rm -rf /").await;

        // Resolve via the service.
        let result = svc
            .resolve(serde_json::json!({
                "requestId": id,
                "decision": "denied",
            }))
            .await;
        assert!(result.is_ok());

        // The receiver should get Denied.
        let decision = rx.try_recv().unwrap();
        assert_eq!(decision, ApprovalDecision::Denied);
    }

    #[tokio::test]
    async fn test_live_service_get() {
        let mgr = Arc::new(ApprovalManager::default());
        let svc = LiveExecApprovalService::new(mgr);
        let result = svc.get().await.unwrap();
        // Default mode is on-miss.
        assert_eq!(result["mode"], "on-miss");
    }
}
