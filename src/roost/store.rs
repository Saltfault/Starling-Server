//! Durable message history backed by sled.
//!
//! Messages are keyed by `"{channel}/{timestamp}"` so we can efficiently
//! range-scan for everything after a given point — exactly what the
//! `RoostSync` protocol needs to backfill late-joining peers.
//!
//! Timestamps are zero-padded to 20 digits so lexicographic ordering
//! matches chronological ordering.

use crate::event::ChatMessage;

/// A sled‑backed message store.
///
/// Each roost gets its own `Store` instance, backed by its own sled
/// database at `~/.config/starling/roosts/<name>/roost.db`.
#[derive(Debug)]
pub struct Store {
    db: sled::Db,
}

impl Store {
    /// Open (or create) the database at `path`.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        Ok(Self {
            db: sled::open(path)?,
        })
    }

    /// Persist a single chat message into the given channel's history.
    ///
    /// The key is `"{channel}/{zero-padded-timestamp}"`, which keeps
    /// messages sorted by time within each channel.
    pub fn append(&self, chan: &str, m: &ChatMessage) -> anyhow::Result<()> {
        self.db.insert(
            format!("{chan}/{:020}", m.ts).as_bytes(),
            postcard::to_stdvec(m)?,
        )?;
        Ok(())
    }

    /// Return every message in `chan` that was sent *at or after*
    /// timestamp `ts`, ordered chronologically.
    ///
    /// This is the query used by `RoostSync` to backfill history for
    /// clients that join after the roost has been running.
    pub fn since(&self, chan: &str, ts: i64) -> Vec<ChatMessage> {
        self.db
            .range(format!("{chan}/{:020}", ts).as_bytes()..format!("{chan}/~").as_bytes())
            .filter_map(|kv| postcard::from_bytes(&kv.ok()?.1).ok())
            .collect()
    }
}
