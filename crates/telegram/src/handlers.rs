use std::sync::Arc;

use {
    teloxide::{
        prelude::*,
        types::{MediaKind, MessageKind},
    },
    tracing::{debug, info, warn},
};

use moltis_channels::message_log::MessageLogEntry;
use moltis_common::types::{ChatType, MsgContext};

use crate::{access, state::AccountStateMap};

/// Shared context injected into teloxide's dispatcher.
#[derive(Clone)]
pub struct HandlerContext {
    pub accounts: AccountStateMap,
    pub account_id: String,
}

/// Build the teloxide update handler.
pub fn build_handler() -> Handler<
    'static,
    DependencyMap,
    Result<(), Box<dyn std::error::Error + Send + Sync>>,
    teloxide::dispatching::DpHandlerDescription,
> {
    Update::filter_message().endpoint(handle_message)
}

/// Handle a single inbound Telegram message (called from manual polling loop).
pub async fn handle_message_direct(
    msg: Message,
    bot: &Bot,
    account_id: &str,
    accounts: &AccountStateMap,
) -> anyhow::Result<()> {
    let text = extract_text(&msg);
    if text.is_none() && !has_media(&msg) {
        debug!(account_id, "ignoring non-text, non-media message");
        return Ok(());
    }

    let (config, bot_username, outbound, message_log) = {
        let accts = accounts.read().unwrap();
        let state = match accts.get(account_id) {
            Some(s) => s,
            None => {
                warn!(account_id, "handler: account not found in state map");
                return Ok(());
            },
        };
        (
            state.config.clone(),
            state.bot_username.clone(),
            Arc::clone(&state.outbound),
            state.message_log.clone(),
        )
    };

    let (chat_type, group_id) = classify_chat(&msg);
    let peer_id = msg
        .from
        .as_ref()
        .map(|u| u.id.0.to_string())
        .unwrap_or_default();
    let sender_name = msg.from.as_ref().and_then(|u| {
        let first = &u.first_name;
        let last = u.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if name.is_empty() {
            u.username.clone()
        } else {
            Some(name)
        }
    });

    let bot_mentioned = check_bot_mentioned(&msg, bot_username.as_deref());

    debug!(
        account_id,
        ?chat_type,
        peer_id,
        ?bot_mentioned,
        "checking access"
    );

    let username = msg.from.as_ref().and_then(|u| u.username.clone());

    // Access control
    let access_result = access::check_access(
        &config,
        &chat_type,
        &peer_id,
        username.as_deref(),
        group_id.as_deref(),
        bot_mentioned,
    );
    let access_granted = access_result.is_ok();

    // Log every inbound message (before returning on denial).
    if let Some(ref log) = message_log {
        let chat_type_str = match chat_type {
            ChatType::Dm => "dm",
            ChatType::Group => "group",
            ChatType::Channel => "channel",
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let entry = MessageLogEntry {
            id: 0,
            account_id: account_id.to_string(),
            channel_type: "telegram".into(),
            peer_id: peer_id.clone(),
            username: username.clone(),
            sender_name: sender_name.clone(),
            chat_id: msg.chat.id.0.to_string(),
            chat_type: chat_type_str.into(),
            body: text.clone().unwrap_or_default(),
            access_granted,
            created_at: now,
        };
        if let Err(e) = log.log(entry).await {
            warn!(account_id, "failed to log message: {e}");
        }
    }

    if let Err(reason) = access_result {
        warn!(account_id, %reason, peer_id, username = ?username, "handler: access denied");
        return Ok(());
    }

    debug!(account_id, "handler: access granted");

    let session_key = build_session_key(account_id, &chat_type, &peer_id, group_id.as_deref());

    let reply_to_id = msg.reply_to_message().map(|r| r.id.0.to_string());

    let body = text.unwrap_or_default();

    let msg_ctx = MsgContext {
        body,
        from: peer_id,
        to: msg.chat.id.0.to_string(),
        channel: "telegram".into(),
        account_id: account_id.to_string(),
        chat_type,
        session_key,
        reply_to_id,
        media_path: None,
        media_url: extract_media_url(&msg),
        group_id,
        guild_id: None,
        team_id: None,
        sender_name,
    };

    // Dispatch to auto-reply pipeline
    match moltis_auto_reply::reply::get_reply(&msg_ctx).await {
        Ok(reply) => {
            info!(account_id, to = %msg_ctx.to, text = %reply.text, "sending reply");
            if let Err(e) = outbound.send_reply(bot, &msg_ctx.to, &reply).await {
                warn!(account_id, "failed to send reply: {e}");
            }
        },
        Err(e) => {
            warn!(account_id, "auto-reply failed: {e}");
        },
    }

    Ok(())

}

/// Handle a single inbound Telegram message (teloxide dispatcher endpoint).
async fn handle_message(
    msg: Message,
    bot: Bot,
    ctx: Arc<HandlerContext>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    handle_message_direct(msg, &bot, &ctx.account_id, &ctx.accounts)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Extract text content from a message.
fn extract_text(msg: &Message) -> Option<String> {
    match &msg.kind {
        MessageKind::Common(common) => match &common.media_kind {
            MediaKind::Text(t) => Some(t.text.clone()),
            MediaKind::Photo(p) => p.caption.clone(),
            MediaKind::Document(d) => d.caption.clone(),
            MediaKind::Audio(a) => a.caption.clone(),
            MediaKind::Voice(v) => v.caption.clone(),
            MediaKind::Video(vid) => vid.caption.clone(),
            MediaKind::Animation(a) => a.caption.clone(),
            _ => None,
        },
        _ => None,
    }
}

/// Check if the message contains media (photo, document, etc.).
fn has_media(msg: &Message) -> bool {
    match &msg.kind {
        MessageKind::Common(common) => !matches!(common.media_kind, MediaKind::Text(_)),
        _ => false,
    }
}

/// Extract a file ID reference from a message for later download.
fn extract_media_url(msg: &Message) -> Option<String> {
    match &msg.kind {
        MessageKind::Common(common) => match &common.media_kind {
            MediaKind::Photo(p) => p.photo.last().map(|ps| format!("tg://file/{}", ps.file.id)),
            MediaKind::Document(d) => Some(format!("tg://file/{}", d.document.file.id)),
            MediaKind::Audio(a) => Some(format!("tg://file/{}", a.audio.file.id)),
            MediaKind::Voice(v) => Some(format!("tg://file/{}", v.voice.file.id)),
            MediaKind::Sticker(s) => Some(format!("tg://file/{}", s.sticker.file.id)),
            _ => None,
        },
        _ => None,
    }
}

/// Classify the chat type.
fn classify_chat(msg: &Message) -> (ChatType, Option<String>) {
    match msg.chat.kind {
        teloxide::types::ChatKind::Private(_) => (ChatType::Dm, None),
        teloxide::types::ChatKind::Public(ref p) => {
            let group_id = msg.chat.id.0.to_string();
            match p.kind {
                teloxide::types::PublicChatKind::Channel(_) => (ChatType::Channel, Some(group_id)),
                _ => (ChatType::Group, Some(group_id)),
            }
        },
    }
}

/// Check if the bot was @mentioned in the message.
fn check_bot_mentioned(msg: &Message, bot_username: Option<&str>) -> bool {
    let text = extract_text(msg).unwrap_or_default();
    if let Some(username) = bot_username {
        text.contains(&format!("@{username}"))
    } else {
        false
    }
}

/// Build a session key.
fn build_session_key(
    account_id: &str,
    chat_type: &ChatType,
    peer_id: &str,
    group_id: Option<&str>,
) -> String {
    match chat_type {
        ChatType::Dm => format!("telegram:{account_id}:dm:{peer_id}"),
        ChatType::Group | ChatType::Channel => {
            let gid = group_id.unwrap_or("unknown");
            format!("telegram:{account_id}:group:{gid}")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_dm() {
        let key = build_session_key("bot1", &ChatType::Dm, "user123", None);
        assert_eq!(key, "telegram:bot1:dm:user123");
    }

    #[test]
    fn session_key_group() {
        let key = build_session_key("bot1", &ChatType::Group, "user123", Some("-100999"));
        assert_eq!(key, "telegram:bot1:group:-100999");
    }
}
