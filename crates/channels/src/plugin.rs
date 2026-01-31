use {
    anyhow::Result, async_trait::async_trait, moltis_common::types::ReplyPayload, tokio::sync::mpsc,
};

/// Core channel plugin trait. Each messaging platform implements this.
#[async_trait]
pub trait ChannelPlugin: Send + Sync {
    /// Channel identifier (e.g. "telegram", "discord").
    fn id(&self) -> &str;

    /// Human-readable channel name.
    fn name(&self) -> &str;

    /// Start an account connection.
    async fn start_account(&mut self, account_id: &str, config: serde_json::Value) -> Result<()>;

    /// Stop an account connection.
    async fn stop_account(&mut self, account_id: &str) -> Result<()>;

    /// Get outbound adapter for sending messages.
    fn outbound(&self) -> Option<&dyn ChannelOutbound>;

    /// Get status adapter for health checks.
    fn status(&self) -> Option<&dyn ChannelStatus>;
}

/// Send messages to a channel.
#[async_trait]
pub trait ChannelOutbound: Send + Sync {
    async fn send_text(&self, account_id: &str, to: &str, text: &str) -> Result<()>;
    async fn send_media(&self, account_id: &str, to: &str, payload: &ReplyPayload) -> Result<()>;
}

/// Probe channel account health.
#[async_trait]
pub trait ChannelStatus: Send + Sync {
    async fn probe(&self, account_id: &str) -> Result<ChannelHealthSnapshot>;
}

/// Channel health snapshot.
#[derive(Debug, Clone)]
pub struct ChannelHealthSnapshot {
    pub connected: bool,
    pub account_id: String,
    pub details: Option<String>,
}

/// Stream event for edit-in-place streaming.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of text to append.
    Delta(String),
    /// Stream is complete.
    Done,
    /// An error occurred.
    Error(String),
}

/// Receiver end of a stream channel.
pub type StreamReceiver = mpsc::Receiver<StreamEvent>;

/// Sender end of a stream channel.
pub type StreamSender = mpsc::Sender<StreamEvent>;

/// Streaming outbound — send responses via edit-in-place updates.
#[async_trait]
pub trait ChannelStreamOutbound: Send + Sync {
    /// Send a streaming response that updates a message in place.
    async fn send_stream(&self, account_id: &str, to: &str, stream: StreamReceiver) -> Result<()>;
}
