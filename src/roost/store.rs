//! Durable, bounded message history backed by sled.

use starling::event::ChatMessage;
use std::path::Path;

const MAX_BACKFILL_MESSAGES: usize = 500;
const KEY_VERSION: u8 = 1;

/// A sled-backed message store.
#[derive(Debug)]
pub struct Store {
    db: sled::Db,
}

impl Store {
    /// Open (or create) the database at `path`.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self {
            db: sled::open(path)?,
        })
    }

    /// Persist a message without allowing equal timestamps to overwrite each other.
    pub fn append(&self, channel: &str, message: &ChatMessage) -> anyhow::Result<()> {
        validate_channel(channel)?;
        self.db
            .insert(message_key(channel, message), postcard::to_stdvec(message)?)?;
        Ok(())
    }

    /// Return at most the newest 500 messages strictly newer than `since`.
    ///
    /// Legacy text keys are also scanned so existing databases remain readable.
    pub fn since(&self, channel: &str, since: i64) -> anyhow::Result<Vec<ChatMessage>> {
        validate_channel(channel)?;

        let mut messages = Vec::new();
        let prefix = channel_prefix(channel);
        for item in self.db.scan_prefix(prefix) {
            let (_, value) = item?;
            let message: ChatMessage = postcard::from_bytes(&value)?;
            if message.ts > since {
                messages.push(message);
            }
        }

        let legacy_prefix = format!("{channel}/");
        for item in self.db.scan_prefix(legacy_prefix.as_bytes()) {
            let (_, value) = item?;
            let message: ChatMessage = postcard::from_bytes(&value)?;
            if message.ts > since {
                messages.push(message);
            }
        }

        messages.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.id.cmp(&b.id)));
        messages.dedup_by(|a, b| a.id == b.id);
        if messages.len() > MAX_BACKFILL_MESSAGES {
            messages.drain(..messages.len() - MAX_BACKFILL_MESSAGES);
        }
        Ok(messages)
    }
}

pub(super) fn validate_channel(channel: &str) -> anyhow::Result<()> {
    let valid = !channel.is_empty()
        && channel.len() <= 64
        && channel
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if !valid {
        anyhow::bail!("invalid channel name");
    }
    Ok(())
}

fn channel_prefix(channel: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(channel.len() + 2);
    key.push(KEY_VERSION);
    key.extend_from_slice(channel.as_bytes());
    key.push(0);
    key
}

fn message_key(channel: &str, message: &ChatMessage) -> Vec<u8> {
    let mut key = channel_prefix(channel);
    let ordered_timestamp = (message.ts as u64) ^ (1_u64 << 63);
    key.extend_from_slice(&ordered_timestamp.to_be_bytes());
    key.extend_from_slice(message.id.as_bytes());
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, ts: i64) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            author: "bird".into(),
            body: id.into(),
            ts,
        }
    }

    fn temporary_store() -> Store {
        Store {
            db: sled::Config::new().temporary(true).open().unwrap(),
        }
    }

    #[test]
    fn equal_timestamps_do_not_overwrite_messages() {
        let store = temporary_store();
        store.append("general", &message("a", 10)).unwrap();
        store.append("general", &message("b", 10)).unwrap();

        let result = store.since("general", 0).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "a");
        assert_eq!(result[1].id, "b");
    }

    #[test]
    fn backfill_is_exclusive_and_bounded_to_newest_messages() {
        let store = temporary_store();
        for ts in 0..510 {
            store
                .append("general", &message(&format!("{ts:03}"), ts))
                .unwrap();
        }

        let result = store.since("general", 5).unwrap();
        assert_eq!(result.len(), MAX_BACKFILL_MESSAGES);
        assert_eq!(result.first().unwrap().ts, 10);
        assert_eq!(result.last().unwrap().ts, 509);
    }

    #[test]
    fn rejects_channels_that_can_escape_key_namespaces() {
        for channel in ["", "../general", "general/other", "contains space"] {
            assert!(validate_channel(channel).is_err());
        }
    }
}
